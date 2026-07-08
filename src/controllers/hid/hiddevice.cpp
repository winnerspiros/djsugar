#include "controllers/hid/hiddevice.h"

#include <hidapi.h>

#include <QDebugStateSaver>
#if defined(Q_OS_ANDROID)
#include <android/log.h>
#endif

#include "controllers/controllermappinginfo.h"
#include "util/string.h"

#ifndef Q_OS_ANDROID
namespace {

constexpr std::size_t kDeviceInfoStringMaxLength = 512;

PhysicalTransportProtocol hidapiBusType2PhysicalTransportProtocol(hid_bus_type busType) {
    switch (busType) {
    case HID_API_BUS_USB:
        return PhysicalTransportProtocol::USB;
    case HID_API_BUS_BLUETOOTH:
        return PhysicalTransportProtocol::BlueTooth;
    case HID_API_BUS_I2C:
        return PhysicalTransportProtocol::I2C;
    case HID_API_BUS_SPI:
        return PhysicalTransportProtocol::SPI;
    default:
        return PhysicalTransportProtocol::UNKNOWN;
    }
}

} // namespace
#endif

namespace mixxx {

namespace hid {

#ifndef Q_OS_ANDROID
DeviceInfo::DeviceInfo(const hid_device_info& device_info)
        : vendor_id(device_info.vendor_id),
          product_id(device_info.product_id),
          release_number(device_info.release_number),
          usage_page(device_info.usage_page),
          usage(device_info.usage),
          m_physicalTransportProtocol(hidapiBusType2PhysicalTransportProtocol(
                  device_info.bus_type)),
          m_usbInterfaceNumber(device_info.interface_number),
          m_pathRaw(device_info.path,
                  mixxx::strnlen_s(device_info.path, PATH_MAX)),
          m_serialNumberRaw(device_info.serial_number,
                  mixxx::wcsnlen_s(device_info.serial_number,
                          kDeviceInfoStringMaxLength)),
          m_manufacturerString(mixxx::convertWCStringToQString(
                  device_info.manufacturer_string, kDeviceInfoStringMaxLength)),
          m_productString(mixxx::convertWCStringToQString(
                  device_info.product_string, kDeviceInfoStringMaxLength)),
          m_serialNumber(mixxx::convertWCStringToQString(
                  m_serialNumberRaw.data(), m_serialNumberRaw.size())) {
}
#else
DeviceInfo::DeviceInfo(
        const QJniObject& usbDevice, const QJniObject& usbInterface)
        : m_androidUsbDevice(usbDevice),
          m_physicalTransportProtocol(PhysicalTransportProtocol::USB) {
    vendor_id = static_cast<unsigned short>(usbDevice.callMethod<jint>("getVendorId"));
    product_id = static_cast<unsigned short>(usbDevice.callMethod<jint>("getProductId"));
    m_manufacturerString = usbDevice.callMethod<jstring>("getManufacturerName").toString();
    m_productString = usbDevice.callMethod<jstring>("getProductName").toString();
    m_serialNumber = usbDevice.callMethod<jstring>("getSerialNumber").toString();

    if (m_serialNumber.isEmpty()) {
        // Android won't allow reading serial number if permission wasn't
        // granted previously. Is this an issue?
        m_serialNumber = "N/A";
    }

    m_usbInterfaceNumber = usbInterface.callMethod<jint>("getId");

    // Discover the interrupt IN endpoint for direct bulkTransfer reads.
    // hidapi/libusb doesn't reliably deliver interrupt transfers on Android.
    jint epCount = usbInterface.callMethod<jint>("getEndpointCount");
    for (jint i = 0; i < epCount; i++) {
        auto ep = usbInterface.callMethod<jobject>(
                "getEndpoint", "(I)Landroid/hardware/usb/UsbEndpoint;", i);
        jint epAddr = ep.callMethod<jint>("getAddress");
        jint epType = ep.callMethod<jint>("getType");
        // USB_ENDPOINT_XFER_INT = 3, DIR_IN = 0x80
        if ((epAddr & 0x80) && epType == 3) {
            __android_log_print(ANDROID_LOG_INFO,
                    "mixxx",
                    "Found HID interrupt IN endpoint 0x%02x at index %d",
                    epAddr,
                    i);
            m_androidInterruptEndpoint = ep;
            break;
        }
    }
}
#endif

const std::vector<uint8_t>& DeviceInfo::fetchRawReportDescriptor(hid_device* pHidDevice) {
    if (!pHidDevice) {
        static const std::vector<uint8_t> emptyDescriptor;
        return emptyDescriptor;
    }
    if (!m_reportDescriptor.empty()) {
        //
        return m_reportDescriptor;
    }

    uint8_t tempReportDescriptor[HID_API_MAX_REPORT_DESCRIPTOR_SIZE];
    int descriptorSize = hid_get_report_descriptor(pHidDevice,
            tempReportDescriptor,
            HID_API_MAX_REPORT_DESCRIPTOR_SIZE);
    if (descriptorSize <= 0) {
        static const std::vector<uint8_t> emptyDescriptor;
        return emptyDescriptor;
    }
    m_reportDescriptor = std::vector<uint8_t>(tempReportDescriptor,
            tempReportDescriptor + descriptorSize);

    return m_reportDescriptor;
}

QString DeviceInfo::formatName() const {
    // We include the last 4 digits of the serial number and the
    // interface number to allow the user (and Mixxx!) to keep
    // track of which is which
    const auto serialSuffix = getSerialNumber().right(4);
    if (m_usbInterfaceNumber >= 0) {
        return getProductString() +
                QChar(' ') +
                serialSuffix +
                QChar('_') +
                QString::number(m_usbInterfaceNumber);
    } else {
        return getProductString() +
                QChar(' ') +
                serialSuffix;
    }
}

QDebug operator<<(QDebug dbg, const DeviceInfo& deviceInfo) {
    const auto dbgState = QDebugStateSaver(dbg);
    dbg.nospace().noquote() << "{ ";

    auto formatHex = [](unsigned short value) {
        return QString::number(value, 16).toUpper().rightJustified(4, '0');
    };

    dbg << "VID:" << formatHex(deviceInfo.getVendorId())
        << " PID:" << formatHex(deviceInfo.getProductId()) << " | ";

    dbg << "Physical: "
        << Controller::physicalTransport2String(deviceInfo.getPhysicalTransportProtocol())
        << " | ";

    dbg << "Usage-Page: " << formatHex(deviceInfo.getUsagePage())
        << ' ' << deviceInfo.getUsagePageDescription() << " | ";

    dbg << "Usage: " << formatHex(deviceInfo.getUsage())
        << ' ' << deviceInfo.getUsageDescription() << " | ";

    dbg << "Interface: #" << deviceInfo.getUsbInterfaceNumber().value_or(-1)
        << " | ";

    dbg << "Manufacturer: " << deviceInfo.getVendorString()
        << " | Product: " << deviceInfo.getProductString()
        << " | S/N: " << deviceInfo.getSerialNumber();

    dbg << " }";
    return dbg;
}

} // namespace hid
} // namespace mixxx
