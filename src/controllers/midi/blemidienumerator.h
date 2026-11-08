#pragma once

#include <QHash>
#include <QList>
#include <QMutex>
#include <QObject>
#include <QTimer>

#include "controllers/midi/midienumerator.h"
#include "preferences/usersettings.h"

class BleMidiController;
class Controller;

class BleMidiEnumerator : public MidiEnumerator {
    Q_OBJECT

  public:
    explicit BleMidiEnumerator(UserSettingsPointer pConfig);
    ~BleMidiEnumerator() override;

    QList<Controller*> queryDevices() override;
    void startScan();
    bool isConnected() const;
    void rescan();

    static void notifyBleDeviceConnected(const QString& address, const QString& name);
    static void notifyBleDeviceDisconnected(const QString& address);

  private slots:
    void slotOnScanTimeout();

  private:
    UserSettingsPointer m_pConfig;
    QList<Controller*> m_devices;
    QList<Controller*> m_removedDevices;
    QHash<QString, BleMidiController*> m_devicesByAddress;
    QTimer* m_pScanTimer;
    bool m_scanning;
    mutable QMutex m_mutex;

    static QMutex s_connectedMutex;
    static QHash<QString, QString> s_connectedGattDevices;
};
