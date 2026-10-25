#pragma once

#ifdef __ANDROID__

#include <QSet>

#include "controllers/midi/midienumerator.h"

class AndroidMidiController;
class AndroidUsbMidiController;

/// Enumerates MIDI devices on Android via android.media.midi.MidiManager,
/// plus a UsbManager fallback for devices the audio HAL hides. Device entries
/// persist across scans (removed ones are deferred-deleted so the GUI can
/// still hold pointers until it processes devicesChanged()).
class AndroidMidiEnumerator : public MidiEnumerator {
    Q_OBJECT
  public:
    AndroidMidiEnumerator();
    ~AndroidMidiEnumerator() override;

    QList<Controller*> queryDevices() override;

  private:
    QList<AndroidMidiController*> m_devices;
    QList<Controller*> m_removedDevices;
};

#endif // __ANDROID__
