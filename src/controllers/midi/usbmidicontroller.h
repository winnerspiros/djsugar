#pragma once

#include <QString>

#include "controllers/midi/midicontroller.h"

/// Android USB MIDI controller implementation.
///
/// Uses Android's android.media.midi API via JNI to communicate with USB MIDI
/// devices. This is the standard Android API for USB MIDI and is how
/// Rekordbox and other Android DJ apps access the DDJ-FLX4 on Android.
///
/// The DDJ-FLX4 main chip (08E4:017B) exposes a MIDI interface (interface 3)
/// that Android's MIDI subsystem claims. This controller reads/writes via
/// MidiInputPort/MidiOutputPort through the UsbMidiDevice Java helper.
class UsbMidiController : public MidiController {
    Q_OBJECT
  public:
    UsbMidiController(const QString& deviceName,
            int vendorId,
            int productId,
            const QString& vendorString,
            const QString& productString);
    ~UsbMidiController() override;

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override;
    QString getVendorString() const override;
    QString getProductString() const override;
    std::optional<uint16_t> getVendorId() const override;
    std::optional<uint16_t> getProductId() const override;
    QString getSerialNumber() const override;
    std::optional<uint8_t> getUsbInterfaceNumber() const override;

    /// Called from Java when MIDI data arrives
    static void onMidiDataReceived(int deviceId, const QByteArray& data);
    /// Called from Java when a USB MIDI device is discovered
    static void onDeviceConnected(int vendorId,
            int productId,
            const QString& manufacturer,
            const QString& product,
            int interfaceNumber);

  protected:
    void sendShortMsg(unsigned char status,
            unsigned char byte1,
            unsigned char byte2) override;

  private:
    int open(const QString& resourcePath) override;
    int close() override;
    bool sendBytes(const QByteArray& data) override;
    bool poll() override;
    bool isPolling() const override;

    static UsbMidiController* s_pInstance;

    int m_vendorId;
    int m_productId;
    int m_deviceId;
    QString m_vendorString;
    QString m_productString;
    bool m_connected;
};
