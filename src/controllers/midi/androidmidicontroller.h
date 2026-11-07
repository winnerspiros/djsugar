#pragma once

#include <QJniObject>
#include <cstdint>
#include <memory>

#include "controllers/midi/midicontroller.h"
#include "rigtorp/SPSCQueue.h"
#include "util/duration.h"

struct AMidiDevice;
struct AMidiInputPort;
struct AMidiOutputPort;

// Length of SysEx buffer in byte
#define MIXXX_ANDROID_MIDI_SYSEX_BUFFER_LEN 1024

namespace mixxx {
namespace androidusb {
class AndroidUsbAudioDevice;
}
} // namespace mixxx

/// Common base of the Android MIDI controllers: parses a raw MIDI byte stream.
class AndroidMidiControllerBase : public MidiController {
    Q_OBJECT
  public:
    explicit AndroidMidiControllerBase(const QString& deviceName);

  protected:
    void resetParser();
    void parseBytes(const uint8_t* pData, size_t length, mixxx::Duration timestamp);

  private:
    unsigned char m_runningStatus;
    unsigned char m_data[2];
    int m_dataCount;
    bool m_bInSysex;
    QByteArray m_sysexBuffer;
};

/// Android MIDI implementation of MidiController
///
/// Devices are listed and opened through android.media.midi (see
/// org.mixxx.MidiHelper) and the actual I/O uses the NDK AMidi API, which is
/// resolved at runtime because it requires API level 29.
class AndroidMidiController : public AndroidMidiControllerBase {
    Q_OBJECT
  public:
    AndroidMidiController(const QJniObject& deviceInfo, const QString& deviceName);
    ~AndroidMidiController() override;

    /// Returns true if the AMidi NDK library could be loaded.
    static bool isSupported();

    /// Android's MIDI device id, which is unique for as long as the device is
    /// plugged in.
    int androidDeviceId() const {
        return m_androidDeviceId;
    }

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override;

    QString getVendorString() const override {
        return m_manufacturer;
    }
    QString getProductString() const override {
        return m_product;
    }
    std::optional<uint16_t> getVendorId() const override {
        return m_vendorId;
    }
    std::optional<uint16_t> getProductId() const override {
        return m_productId;
    }
    QString getSerialNumber() const override {
        return m_serialNumber;
    }
    std::optional<uint8_t> getUsbInterfaceNumber() const override {
        return std::nullopt;
    }

  private slots:
    bool poll() override;

  protected:
    void sendShortMsg(unsigned char status,
            unsigned char byte1,
            unsigned char byte2) override;

  private:
    int open(const QString& resourcePath) override;
    int close() override;

    bool sendBytes(const QByteArray& data) override;

    bool isPolling() const override {
        return true;
    }

    void closePorts();

    QJniObject m_deviceInfo;
    QJniObject m_midiDevice;
    AMidiDevice* m_pDevice;
    AMidiOutputPort* m_pOutputPort; // messages coming from the controller
    AMidiInputPort* m_pInputPort;   // messages sent to the controller

    int m_androidDeviceId;
    int m_androidDeviceType;
    QString m_manufacturer;
    QString m_product;
    QString m_serialNumber;
    std::optional<uint16_t> m_vendorId;
    std::optional<uint16_t> m_productId;
};

/// USB-MIDI controller driven directly over libusb. Used for USB audio
/// interfaces whose kernel driver was detached by AndroidUsbAudioDevice, which
/// also removes them from the Android MIDI service.
class AndroidUsbMidiController : public AndroidMidiControllerBase {
    Q_OBJECT
  public:
    explicit AndroidUsbMidiController(
            std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> pDevice);
    ~AndroidUsbMidiController() override;

    const std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice>& usbDevice() const {
        return m_pDevice;
    }

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override {
        return PhysicalTransportProtocol::USB;
    }
    QString getVendorString() const override;
    QString getProductString() const override;
    std::optional<uint16_t> getVendorId() const override;
    std::optional<uint16_t> getProductId() const override;
    QString getSerialNumber() const override {
        return QString();
    }
    std::optional<uint8_t> getUsbInterfaceNumber() const override {
        return std::nullopt;
    }

  private slots:
    bool poll() override;

  protected:
    void sendShortMsg(unsigned char status,
            unsigned char byte1,
            unsigned char byte2) override;

  private:
    int open(const QString& resourcePath) override;
    int close() override;

    bool sendBytes(const QByteArray& data) override;

    bool isPolling() const override {
        return true;
    }

    std::shared_ptr<mixxx::androidusb::AndroidUsbAudioDevice> m_pDevice;
    // USB-MIDI event packets received on the USB thread, which may also run
    // the audio engine: no locks or allocations there
    struct MidiPacket {
        uint8_t bytes[4];
        mixxx::Duration timestamp;
    };
    rigtorp::SPSCQueue<MidiPacket> m_packets;
};
