#pragma once

#include <QJniObject>
#include <QMutex>
#include <QThread>

#include "controllers/midi/midicontroller.h"

/// Android USB-MIDI controller.
/// Handles USB devices with Audio/MIDI interface (class=1, subclass=3)
/// using Android's UsbDeviceConnection for raw USB-MIDI packets.
class AndroidUsbMidiController : public MidiController {
    Q_OBJECT

  public:
    explicit AndroidUsbMidiController(const QString& name,
            int vendorId,
            int productId,
            QObject* parent = nullptr);
    ~AndroidUsbMidiController() override;

    int open(const QString& resourcePath) override;

    /// Set the JNI UsbDevice and UsbManager from the enumerator
    void setUsbDevice(QJniObject usbDevice, QJniObject usbManager);

  private:
    class IoThread : public QThread {
      public:
        explicit IoThread(AndroidUsbMidiController* pController);
        void stop();

      protected:
        void run() override;

      private:
        AndroidUsbMidiController* m_pController;
        volatile bool m_stop;
    };

    int m_vendorId;
    int m_productId;
    QJniObject m_usbDevice;
    QJniObject m_usbManager;
    QJniObject m_usbConnection;
    QJniObject m_usbInterface;
    QJniObject m_endpointIn;
    QJniObject m_endpointOut;
    IoThread* m_pIoThread;
    QMutex m_mutex;

    void parseUsbMidiPacket(const QByteArray& packet);
    void sendMidiMessage(uint8_t status, uint8_t data1, uint8_t data2);
};
