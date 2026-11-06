#pragma once

#include "controllers/midi/midienumerator.h"

class AndroidMidiController;

/// Lists the MIDI devices known to Android's MidiManager (USB, Bluetooth and
/// virtual devices). The list is kept up to date on every call so devices can
/// be plugged in and out while Mixxx is running.
class AndroidMidiEnumerator : public MidiEnumerator {
    Q_OBJECT
  public:
    AndroidMidiEnumerator();
    ~AndroidMidiEnumerator() override;

    QList<Controller*> queryDevices() override;

  private:
    QList<AndroidMidiController*> m_devices;
    // Unplugged devices are only deleted on shutdown, because the GUI may
    // still hold pointers to them until it processed devicesChanged().
    QList<Controller*> m_removedDevices;
};
