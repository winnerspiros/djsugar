#pragma once

#include <cstdint>
#include <mutex>

struct libusb_device_handle;

namespace mixxx {
namespace usbdrive {

/// Read-only access to a USB mass storage device (USB sticks, card readers)
/// with the Bulk-Only Transport and SCSI commands. The interface must have
/// been claimed by the caller.
class UsbMassStorage {
  public:
    UsbMassStorage(libusb_device_handle* pHandle,
            int interfaceNumber,
            uint8_t endpointIn,
            uint8_t endpointOut);

    /// Looks for a logical unit with a medium and reads its capacity
    bool open();

    uint32_t blockSize() const {
        return m_blockSize;
    }
    uint64_t blockCount() const {
        return m_blockCount;
    }

    /// Reads `count` blocks starting at `lba`. Thread-safe.
    bool readBlocks(uint64_t lba, uint32_t count, uint8_t* pDestination);

    /// True once a transfer failed because the device is gone
    bool isDisconnected() const {
        return m_disconnected;
    }

  private:
    bool command(const uint8_t* pCommand,
            int commandLength,
            uint8_t* pData,
            uint32_t dataLength);
    bool testUnitReady();
    bool readCapacity();
    void resetRecovery();
    int maxLun();

    libusb_device_handle* m_pHandle;
    const int m_interfaceNumber;
    const uint8_t m_endpointIn;
    const uint8_t m_endpointOut;
    uint8_t m_lun;
    uint32_t m_tag;
    uint32_t m_blockSize;
    uint64_t m_blockCount;
    bool m_disconnected;
    std::mutex m_mutex;
};

} // namespace usbdrive
} // namespace mixxx
