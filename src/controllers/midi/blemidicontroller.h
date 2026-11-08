#pragma once

#include <QMap>
#include <QMutex>
#include <QString>

#include "controllers/midi/androidmidicontroller.h"

/// BLE MIDI controller implementation for Android.
///
/// Communicates with BLE MIDI devices using the standard BLE MIDI service
/// (UUID 03B80E5A-EDE8-4B33-A751-6CE34EC4C700) via Android BluetoothGatt APIs
/// through JNI, and parses incoming MIDI byte streams via AndroidMidiControllerBase.
class BleMidiController : public AndroidMidiControllerBase {
    Q_OBJECT
  public:
    explicit BleMidiController(const QString& deviceName,
            const QString& deviceAddress);
    ~BleMidiController() override;

    const QString& deviceAddress() const {
        return m_deviceAddress;
    }

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override;
    QString getVendorString() const override;
    QString getProductString() const override;
    std::optional<uint16_t> getVendorId() const override;
    std::optional<uint16_t> getProductId() const override;
    QString getSerialNumber() const override;
    std::optional<uint8_t> getUsbInterfaceNumber() const override;

    bool connectDevice();
    void disconnectDevice();

    static void dispatchMidiData(const QString& address, const QByteArray& data);

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
    bool isPolling() const override;

    static QMutex s_instancesMutex;
    static QMap<QString, BleMidiController*> s_instances;

    QString m_deviceAddress;
    QString m_vendorString;
    QString m_productString;
    QString m_serialNumber;
    bool m_connected;
};
