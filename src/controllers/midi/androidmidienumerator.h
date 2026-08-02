#pragma once

#ifdef __ANDROID__

#include "controllers/midi/midienumerator.h"

/// Enumerates MIDI devices on Android via android.media.midi.MidiManager.
class AndroidMidiEnumerator : public MidiEnumerator {
  public:
    AndroidMidiEnumerator();
    ~AndroidMidiEnumerator() override;

    QList<Controller*> queryDevices() override;
};

#endif // __ANDROID__
