#include "library/usbdrive/usbmassstorage.h"

#include <libusb.h>

#include <QDebug>
#include <QThread>
#include <algorithm>
#include <cstring>

namespace mixxx {
namespace usbdrive {

namespace {

constexpr uint32_t kCbwSignature = 0x43425355; // "USBC"
constexpr uint32_t kCswSignature = 0x53425355; // "USBS"
constexpr int kCbwLength = 31;
constexpr int kCswLength = 13;
constexpr unsigned int kTimeoutMillis = 10000;

constexpr uint8_t kTestUnitReady = 0x00;
constexpr uint8_t kRequestSense = 0x03;
constexpr uint8_t kInquiry = 0x12;
constexpr uint8_t kReadCapacity10 = 0x25;
constexpr uint8_t kRead10 = 0x28;
constexpr uint8_t kReadCapacity16 = 0x9E;
constexpr uint8_t kRead16 = 0x88;

void putLittleEndian32(uint8_t* p, uint32_t value) {
    p[0] = static_cast<uint8_t>(value);
    p[1] = static_cast<uint8_t>(value >> 8);
    p[2] = static_cast<uint8_t>(value >> 16);
    p[3] = static_cast<uint8_t>(value >> 24);
}

uint32_t getLittleEndian32(const uint8_t* p) {
    return p[0] | (p[1] << 8) | (p[2] << 16) | (static_cast<uint32_t>(p[3]) << 24);
}

uint32_t getBigEndian32(const uint8_t* p) {
    return (static_cast<uint32_t>(p[0]) << 24) | (p[1] << 16) | (p[2] << 8) | p[3];
}

void putBigEndian32(uint8_t* p, uint32_t value) {
    p[0] = static_cast<uint8_t>(value >> 24);
    p[1] = static_cast<uint8_t>(value >> 16);
    p[2] = static_cast<uint8_t>(value >> 8);
    p[3] = static_cast<uint8_t>(value);
}

bool isGone(int error) {
    return error == LIBUSB_ERROR_NO_DEVICE || error == LIBUSB_ERROR_IO;
}

} // namespace

UsbMassStorage::UsbMassStorage(libusb_device_handle* pHandle,
        int interfaceNumber,
        uint8_t endpointIn,
        uint8_t endpointOut)
        : m_pHandle(pHandle),
          m_interfaceNumber(interfaceNumber),
          m_endpointIn(endpointIn),
          m_endpointOut(endpointOut),
          m_lun(0),
          m_tag(1),
          m_blockSize(0),
          m_blockCount(0),
          m_disconnected(false) {
}

int UsbMassStorage::maxLun() {
    uint8_t lun = 0;
    // Get Max LUN; devices with a single unit may stall it
    const int result = libusb_control_transfer(m_pHandle,
            static_cast<int>(LIBUSB_ENDPOINT_IN) |
                    static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                    static_cast<int>(LIBUSB_RECIPIENT_INTERFACE),
            0xFE,
            0,
            static_cast<uint16_t>(m_interfaceNumber),
            &lun,
            1,
            1000);
    return result == 1 ? std::min<int>(lun, 15) : 0;
}

bool UsbMassStorage::open() {
    std::lock_guard lock(m_mutex);
    const int lastLun = maxLun();
    for (int lun = 0; lun <= lastLun; ++lun) {
        m_lun = static_cast<uint8_t>(lun);
        uint8_t inquiry[36] = {};
        const uint8_t inquiryCommand[6] = {kInquiry, 0, 0, 0, sizeof(inquiry), 0};
        command(inquiryCommand, sizeof(inquiryCommand), inquiry, sizeof(inquiry));
        // Some sticks need a few tries after being plugged in
        bool ready = false;
        for (int attempt = 0; attempt < 5 && !ready && !m_disconnected; ++attempt) {
            ready = testUnitReady();
            if (!ready) {
                QThread::msleep(200);
            }
        }
        if (ready && readCapacity()) {
            qInfo() << "USB drive LUN" << lun << ":" << m_blockCount << "blocks of"
                    << m_blockSize << "bytes";
            return true;
        }
    }
    return false;
}

bool UsbMassStorage::testUnitReady() {
    const uint8_t testCommand[6] = {kTestUnitReady, 0, 0, 0, 0, 0};
    if (command(testCommand, sizeof(testCommand), nullptr, 0)) {
        return true;
    }
    // Clear the pending sense data (e.g. "medium changed")
    uint8_t sense[18] = {};
    const uint8_t senseCommand[6] = {kRequestSense, 0, 0, 0, sizeof(sense), 0};
    command(senseCommand, sizeof(senseCommand), sense, sizeof(sense));
    return false;
}

bool UsbMassStorage::readCapacity() {
    uint8_t capacity[8] = {};
    const uint8_t command10[10] = {kReadCapacity10, 0, 0, 0, 0, 0, 0, 0, 0, 0};
    if (!command(command10, sizeof(command10), capacity, sizeof(capacity))) {
        return false;
    }
    uint64_t lastBlock = getBigEndian32(capacity);
    m_blockSize = getBigEndian32(capacity + 4);
    if (lastBlock == 0xFFFFFFFF) {
        // Larger than 2 TiB
        uint8_t capacity16[32] = {};
        uint8_t command16[16] = {kReadCapacity16, 0x10};
        putBigEndian32(command16 + 10, sizeof(capacity16));
        if (!command(command16, sizeof(command16), capacity16, sizeof(capacity16))) {
            return false;
        }
        lastBlock = (static_cast<uint64_t>(getBigEndian32(capacity16)) << 32) |
                getBigEndian32(capacity16 + 4);
        m_blockSize = getBigEndian32(capacity16 + 8);
    }
    m_blockCount = lastBlock + 1;
    return m_blockSize >= 512 && m_blockSize <= 4096 && m_blockCount > 0;
}

bool UsbMassStorage::readBlocks(uint64_t lba, uint32_t count, uint8_t* pDestination) {
    std::lock_guard lock(m_mutex);
    if (m_disconnected) {
        return false;
    }
    // Keep each command reasonably small, some devices don't like large ones
    constexpr uint32_t kMaxBlocksPerCommand = 128;
    while (count > 0) {
        const uint32_t blocks = std::min(count, kMaxBlocksPerCommand);
        bool ok;
        if (lba + blocks > 0xFFFFFFFF) {
            uint8_t read16[16] = {kRead16};
            putBigEndian32(read16 + 2, static_cast<uint32_t>(lba >> 32));
            putBigEndian32(read16 + 6, static_cast<uint32_t>(lba));
            putBigEndian32(read16 + 10, blocks);
            ok = command(read16, sizeof(read16), pDestination, blocks * m_blockSize);
        } else {
            uint8_t read10[10] = {kRead10};
            putBigEndian32(read10 + 2, static_cast<uint32_t>(lba));
            read10[7] = static_cast<uint8_t>(blocks >> 8);
            read10[8] = static_cast<uint8_t>(blocks);
            ok = command(read10, sizeof(read10), pDestination, blocks * m_blockSize);
        }
        if (!ok) {
            return false;
        }
        lba += blocks;
        count -= blocks;
        pDestination += blocks * m_blockSize;
    }
    return true;
}

bool UsbMassStorage::command(const uint8_t* pCommand,
        int commandLength,
        uint8_t* pData,
        uint32_t dataLength) {
    if (m_disconnected) {
        return false;
    }
    uint8_t cbw[kCbwLength] = {};
    const uint32_t tag = m_tag++;
    putLittleEndian32(cbw, kCbwSignature);
    putLittleEndian32(cbw + 4, tag);
    putLittleEndian32(cbw + 8, dataLength);
    cbw[12] = 0x80; // Data in (Mixxx never writes)
    cbw[13] = m_lun;
    cbw[14] = static_cast<uint8_t>(commandLength);
    std::memcpy(cbw + 15, pCommand, commandLength);

    int transferred = 0;
    int error = libusb_bulk_transfer(
            m_pHandle, m_endpointOut, cbw, kCbwLength, &transferred, kTimeoutMillis);
    if (error == LIBUSB_ERROR_PIPE) {
        libusb_clear_halt(m_pHandle, m_endpointOut);
        error = libusb_bulk_transfer(
                m_pHandle, m_endpointOut, cbw, kCbwLength, &transferred, kTimeoutMillis);
    }
    if (error < 0 || transferred != kCbwLength) {
        if (isGone(error)) {
            m_disconnected = true;
        }
        qWarning() << "USB drive: sending command" << pCommand[0]
                   << "failed:" << libusb_error_name(error);
        resetRecovery();
        return false;
    }

    if (dataLength > 0) {
        uint32_t received = 0;
        while (received < dataLength) {
            error = libusb_bulk_transfer(m_pHandle,
                    m_endpointIn,
                    pData + received,
                    static_cast<int>(dataLength - received),
                    &transferred,
                    kTimeoutMillis);
            if (error == LIBUSB_ERROR_PIPE) {
                // The device has less data: the status follows
                libusb_clear_halt(m_pHandle, m_endpointIn);
                break;
            }
            if (error < 0) {
                if (isGone(error)) {
                    m_disconnected = true;
                }
                qWarning() << "USB drive: reading data of command" << pCommand[0]
                           << "failed:" << libusb_error_name(error);
                resetRecovery();
                return false;
            }
            received += transferred;
            if (transferred == 0) {
                break;
            }
        }
    }

    uint8_t csw[kCswLength] = {};
    error = libusb_bulk_transfer(
            m_pHandle, m_endpointIn, csw, kCswLength, &transferred, kTimeoutMillis);
    if (error == LIBUSB_ERROR_PIPE) {
        libusb_clear_halt(m_pHandle, m_endpointIn);
        error = libusb_bulk_transfer(
                m_pHandle, m_endpointIn, csw, kCswLength, &transferred, kTimeoutMillis);
    }
    if (error < 0 || transferred != kCswLength || getLittleEndian32(csw) != kCswSignature ||
            getLittleEndian32(csw + 4) != tag) {
        if (isGone(error)) {
            m_disconnected = true;
        }
        qWarning() << "USB drive: invalid status of command" << pCommand[0]
                   << libusb_error_name(error);
        resetRecovery();
        return false;
    }
    // 0 = passed, 1 = failed, 2 = phase error
    if (csw[12] == 2) {
        resetRecovery();
    }
    return csw[12] == 0;
}

void UsbMassStorage::resetRecovery() {
    if (m_disconnected) {
        return;
    }
    // Bulk-Only Mass Storage Reset, then clear both endpoints
    libusb_control_transfer(m_pHandle,
            static_cast<int>(LIBUSB_ENDPOINT_OUT) |
                    static_cast<int>(LIBUSB_REQUEST_TYPE_CLASS) |
                    static_cast<int>(LIBUSB_RECIPIENT_INTERFACE),
            0xFF,
            0,
            static_cast<uint16_t>(m_interfaceNumber),
            nullptr,
            0,
            1000);
    libusb_clear_halt(m_pHandle, m_endpointIn);
    libusb_clear_halt(m_pHandle, m_endpointOut);
}

} // namespace usbdrive
} // namespace mixxx
