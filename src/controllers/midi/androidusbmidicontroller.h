#pragma once

#include <QString>
#include <QThread>

#include "controllers/midi/midicontroller.h"

#ifdef Q_OS_ANDROID
#include <QJniObject>
#endif

/// Android USB MIDI controller that reads/writes MIDI data directly via
/// UsbDeviceConnection.bulkTransfer() on the USB bulk endpoints.
///
/// This bypasses Android's MidiManager entirely because the DDJ-FLX4's
/// MIDI interface (class 1 subclass 3) is part of a composite USB audio
/// device. Android's audio subsystem claims the device before MidiManager
/// can see it. Reading directly from the bulk endpoints via UsbManager
/// is the only reliable way to access MIDI on this device.
///
/// The controller is created by the HidEnumerator when it detects a
/// USB audio/MIDI streaming interface during Android device scanning.
class AndroidUsbMidiController : public MidiController {
  public:
    AndroidUsbMidiController(const QString& name,
            int vendorId,
            int productId,
            const QString& vendorString,
            const QString& productString);
    ~AndroidUsbMidiController() override;

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override;
    QString getVendorString() const override;
    QString getProductString() const override;
    std::optional<uint16_t> getVendorId() const override;
    std::optional<uint16_t> getProductId() const override;
    QString getSerialNumber() const override;
    std::optional<uint8_t> getUsbInterfaceNumber() const override;

#ifdef Q_OS_ANDROID
    /// Set the Android USB device, connection, and endpoints from the
    /// enumerator. Called after construction but before open().
    void setAndroidDevice(QJniObject&& usbDevice,
            QJniObject&& usbConnection,
            QJniObject&& bulkInEndpoint,
            QJniObject&& bulkOutEndpoint);
#endif

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

#ifdef Q_OS_ANDROID
    // MIDI I/O thread — reads from bulk IN endpoint
    class MidiIoThread : public QThread {
      public:
        MidiIoThread(AndroidUsbMidiController* parent);
        void stop();
        void setAndroidDevice(QJniObject&& usbDevice,
                QJniObject&& usbConnection,
                QJniObject&& bulkInEndpoint,
                QJniObject&& bulkOutEndpoint);

      protected:
        void run() override;

      private:
        AndroidUsbMidiController* m_parent;
        QJniObject m_usbDevice;
        QJniObject m_usbConnection;
        QJniObject m_bulkInEndpoint;
        QJniObject m_bulkOutEndpoint;
        QAtomicInt m_stopRequested;
    };

    MidiIoThread* m_pIoThread;
#endif
    int m_vendorId;
    int m_productId;
    QString m_vendorString;
    QString m_productString;
#ifdef Q_OS_ANDROID
    friend class MidiIoThread;
#endif
};
