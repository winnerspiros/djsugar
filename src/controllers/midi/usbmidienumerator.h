#pragma once

#include <QList>
#include <QObject>

#include "controllers/midi/midienumerator.h"

class Controller;

/// Enumerates USB MIDI devices on Android using android.media.midi API.
///
/// Android's MidiManager provides a synchronous device list (no scanning needed).
/// The enumerator queries available USB MIDI devices and creates a UsbMidiController
/// for each one.
class UsbMidiEnumerator : public MidiEnumerator {
    Q_OBJECT

  public:
    explicit UsbMidiEnumerator();
    ~UsbMidiEnumerator() override;

    QList<Controller*> queryDevices() override;

  private:
    QList<Controller*> m_devices;
};
