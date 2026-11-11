#pragma once

#include <vector>

#include "effects/backends/builtin/djfxbase.h"

// Roll: beat-quantized loop roll. Waits for the next grid boundary,
// captures one slice and repeats it while the effect is on. Shrinking the
// division mid-roll builds the classic 1/4 to 1/8 to 1/16 riser. Each
// repeat can fade and drop in pitch, and a gate shortens the slices into
// a stutter. Slice edges are windowed so it never clicks.
class BeatRepeatGroupState : public DjFxState {
  public:
    explicit BeatRepeatGroupState(const mixxx::EngineParameters& engineParameters);
    void restart();

    enum class Phase {
        Armed,
        Capturing,
        Playing,
    };

    std::vector<float> buffer; // interleaved stereo
    int capacityFrames = 0;
    int captured = 0;
    int loopFrames = 0;
    double readPos = 0.0;
    int playFrame = 0;
    int repeat = 0;
    float lastPhase = 0.0f;
    Phase phase = Phase::Armed;
    fxdsp::Smoother mix;
};

class BeatRepeatEffect : public DjFxEffect<BeatRepeatGroupState, BeatRepeatEffect> {
    DJFX_EFFECT_INTERFACE(BeatRepeatGroupState)

  private:
    enum Param {
        kDivision,
        kMix,
        kDecay,
        kPitch,
        kGate,
    };
};
