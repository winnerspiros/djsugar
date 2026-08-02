#pragma once

#ifdef __ANDROID__

#include <QAtomicInt>
#include <QJniObject>
#include <QMutex>
#include <QString>
#include <QThread>

#include "controllers/midi/midicontroller.h"

class AndroidMidiController : public MidiController {
    Q_OBJECT
  public:
    AndroidMidiController(const QString& name,
            const QJniObject& midiDeviceInfo,
            int inputPortIndex,
            int outputPortIndex);
    ~AndroidMidiController() override;

    PhysicalTransportProtocol getPhysicalTransportProtocol() const override {
        return PhysicalTransportProtocol::USB;
    }
    QString getVendorString() const override { return m_vendor; }
    QString getProductString() const override { return m_product; }
    std::optional<uint16_t> getVendorId() const override {
        return m_vendorId ? std::optional<uint16_t>(m_vendorId) : std::nullopt;
    }
    std::optional<uint16_t> getProductId() const override {
        return m_productId ? std::optional<uint16_t>(m_productId) : std::nullopt;
    }

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

    class IoThread : public QThread {
      public:
        explicit IoThread(AndroidMidiController* parent);
        void stop();
        void send(const QByteArray& data);

      protected:
        void run() override;

      private:
        AndroidMidiController* m_parent;
        QJniObject m_outputPort;
        QJniObject m_inputPort;
        QAtomicInt m_stop{0};
        QMutex m_sendMutex;
        QByteArray m_pendingSend;
        bool m_hasPending{false};
    };

    IoThread* m_pIoThread{nullptr};
    QJniObject m_deviceInfo;
    int m_inputPortIndex;
    int m_outputPortIndex;
    uint16_t m_vendorId{0};
    uint16_t m_productId{0};
    QString m_vendor;
    QString m_product;
};

#endif // __ANDROID__
