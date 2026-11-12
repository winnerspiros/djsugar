#pragma once

#include <QObject>
#include <QTimer>
#include <array>
#include <atomic>
#include <memory>
#include <random>

#include "effects/backends/builtin/dsp/fdnreverb.h"
#include "effects/backends/builtin/dsp/vinylmotion.h"
#include "engine/fxstation/fxstationdsp.h"
#include "engine/fxstation/fxstationinstruments.h"
#include "engine/fxstation/fxstationscenes.h"
#include "util/types.h"

class ControlObject;

namespace mixxx {

/// The FX Station: a remix effector on the main mix, in the style of the
/// remix stations of DJ booths. Four sections, all locked to the beat grid
/// of the deck that leads (or to a tapped tempo):
///
/// - Scene: build ups (BPF Echo, Echo, Noise, Spiral Up, Reverb Up) and
///   break downs (HPF Echo, LPF Echo, Crush Echo, Spiral Down, Reverb Down)
///   on a big dial, with two sub parameters.
/// - Isolate: three bands (low, mid, high) with Isolator, Cut/Add,
///   Trans/Roll and Gate/Drive modes.
/// - X-Pad: kick, snare, clap and hi-hat rolls from 1/1 to 1/16 of a beat,
///   with pitch, an overdub loop of a bar, mutes and four banks.
/// - Release: Vinyl Brake, Echo or Back Spin, turning everything else off.
///
/// Controls are in the [FxStation] group. Processed on the engine thread,
/// without allocations or locks; it costs almost nothing while idle.
class FxStation : public QObject {
    Q_OBJECT
  public:
    explicit FxStation(UserSettingsPointer pConfig);
    ~FxStation() override;

    /// Processes the main mix in place (interleaved stereo), on the engine
    /// thread
    void process(CSAMPLE* pBuffer, int frames, float sampleRate);

    // The scene effects, in the order of scene_type
    enum class Scene {
        BpfEcho,
        Echo,
        Noise,
        SpiralUp,
        ReverbUp,
        HpfEcho,
        LpfEcho,
        CrushEcho,
        SpiralDown,
        ReverbDown,
        Flanger,
        Phaser,
        Filter,
        Roll,
        Gater,
        Pitch,
        Drive,
        Count,
    };
    enum class IsolateMode {
        Isolator,
        CutAdd,
        TransRoll,
        GateDrive,
        Count,
    };
    enum class Release {
        VinylBrake,
        Echo,
        BackSpin,
    };

  private slots:
    void slotFindDecks();

  private:
    static constexpr int kDecks = 4;
    static constexpr int kBands = 3;
    static constexpr int kVoices = 16;
    // Steps of the overdub loop: a bar of four beats in 1/16 beats
    static constexpr int kLoopSteps = 64;

    struct Deck {
        std::atomic<ControlObject*> pPlay{nullptr};
        std::atomic<ControlObject*> pBpm{nullptr};
        std::atomic<ControlObject*> pBeatDistance{nullptr};
        std::atomic<ControlObject*> pVolume{nullptr};
        std::atomic<ControlObject*> pSyncLeader{nullptr};
    };

    struct Voice {
        const fxstation::Instrument* pInstrument = nullptr;
        int instrument = 0;
        double position = 0.0;
        double rate = 1.0;
        float gain = 0.f;
        // Fading out (choked by a new hit of its instrument), per sample
        float fade = 0.f;
        bool active = false;
    };

    struct BandRoll {
        int length = 0;
        int recorded = 0;
        int playPosition = 0;
        bool active = false;
    };

    void prepare(float sampleRate);
    void setReverb(float size, float tone);
    void updateClock(int frames);
    void handleButtons();
    void startRelease();

    bool processScene(float* pLeft, float* pRight);
    bool processAddedScene(float* pLeft, float* pRight);
    void processIsolate(float* pLeft, float* pRight, double beat);
    void triggerInstrument(int instrument, float velocity);
    /// The division of the ribbon, or of the finest roll button held
    int xpadDivision() const;
    void processXPad(float* pLeft, float* pRight, double beat, double previousBeat);
    void processRelease(float* pLeft, float* pRight, double beat, double previousBeat);

    double beatFrames() const;

    // Controls
    std::unique_ptr<ControlObject> m_pSceneType;
    std::unique_ptr<ControlObject> m_pSceneOn;
    std::unique_ptr<ControlObject> m_pSceneDial;
    std::unique_ptr<ControlObject> m_pSceneParam1;
    std::unique_ptr<ControlObject> m_pSceneParam2;
    std::unique_ptr<ControlObject> m_pIsolateMode;
    std::array<std::unique_ptr<ControlObject>, kBands> m_pIsolateBands;
    std::unique_ptr<ControlObject> m_pXPadInstrument;
    // FX SOURCE: what Scene and Isolate process, the music, the X-Pad or both
    std::unique_ptr<ControlObject> m_pSourceInput;
    std::unique_ptr<ControlObject> m_pSourceXPad;
    std::unique_ptr<ControlObject> m_pXPadDivision;
    // One button per division (1/1 to 1/16), held, for controllers
    std::array<std::unique_ptr<ControlObject>, 5> m_pXPadRolls;
    // One shot of each instrument (value: the velocity), for pads
    std::array<std::unique_ptr<ControlObject>, fxstation::kInstruments> m_pXPadHits;
    std::array<bool, fxstation::kInstruments> m_previousHits{};
    std::unique_ptr<ControlObject> m_pXPadPitch;
    std::unique_ptr<ControlObject> m_pXPadVolume;
    std::unique_ptr<ControlObject> m_pXPadOverdub;
    std::array<std::unique_ptr<ControlObject>, fxstation::kInstruments> m_pXPadMutes;
    std::unique_ptr<ControlObject> m_pXPadClear;
    std::unique_ptr<ControlObject> m_pXPadBank;
    std::unique_ptr<ControlObject> m_pXPadReload;
    std::array<std::unique_ptr<ControlObject>, fxstation::kBanks> m_pXPadBankSounds;
    std::unique_ptr<ControlObject> m_pXPadLoopSteps;
    std::unique_ptr<ControlObject> m_pReleaseType;
    std::unique_ptr<ControlObject> m_pRelease;
    std::unique_ptr<ControlObject> m_pReleaseActive;
    std::unique_ptr<ControlObject> m_pBpm;
    std::unique_ptr<ControlObject> m_pBpmAuto;
    std::unique_ptr<ControlObject> m_pTap;
    std::unique_ptr<ControlObject> m_pNudge;
    std::unique_ptr<ControlObject> m_pQuantize;
    std::unique_ptr<ControlObject> m_pBeat;
    std::unique_ptr<ControlObject> m_pSource;

    std::array<Deck, kDecks> m_decks;
    QTimer m_findDecksTimer;
    std::unique_ptr<fxstation::Instruments> m_pInstruments;

    float m_sampleRate;
    std::minstd_rand m_random;

    // The clock: beats since the start, the tempo, the nudge of the phase
    double m_beat;
    double m_bpm;
    double m_nudge;
    qint64 m_frameCount;
    std::array<qint64, 8> m_taps;
    int m_tapCount;
    bool m_previousTap;
    bool m_previousRelease;
    bool m_previousClear;

    // Scene
    fxstation::DelayLine m_echo;
    fxstation::PitchShifter m_spiral;
    fxdsp::FdnReverb m_reverb;
    fxstation::Svf m_sceneFilter;
    fxstation::Svf m_wetFilter;
    fxstation::Smoother m_sceneSend;
    fxstation::Smoother m_sceneAmount;
    fxstation::Smoother m_echoDelay;
    /// The level of the echoes and reverb: held when the scene goes off, so
    /// that the tail rings out at the level it had
    float m_wetLevel;
    float m_crushHold[2];
    int m_crushCounter;
    int m_sceneSilentFrames;
    bool m_sceneActive;
    int m_lastSceneType;
    // The added scenes: their own wet mix (they have no send or tail) and
    // the dial, smoothed per sample, for the ones it sweeps from the center
    fxstation::Smoother m_sceneMix;
    fxstation::Smoother m_sceneDialValue;
    float m_sceneMixTarget;
    float m_sceneDialTarget;
    float m_sceneCoefficient;
    fxstation::scenes::Flanger m_flanger;
    fxstation::scenes::Phaser m_phaser;
    fxstation::scenes::SweepFilter m_sweepFilter;
    fxstation::scenes::Roll m_roll;
    fxstation::scenes::GrainPitch m_grainPitch;
    fxstation::scenes::Drive m_drive;

    // Isolate
    fxstation::Crossover m_lowMid;
    fxstation::Crossover m_midHigh;
    fxstation::Crossover m_lowAllPass;
    std::array<fxstation::Smoother, kBands> m_bandGain;
    std::array<float, kBands> m_bandEnvelope;
    std::array<fxstation::DelayLine, kBands> m_rollBuffers;
    std::array<BandRoll, kBands> m_rolls;
    fxstation::Smoother m_isolateMix;
    bool m_isolateActive;

    // X-Pad
    std::array<Voice, kVoices> m_voices;
    std::array<std::array<bool, kLoopSteps>, fxstation::kInstruments> m_loop;
    bool m_loopHasSteps;
    double m_rollOrigin;
    int m_previousDivision;
    int m_rollInstrument;

    // Release
    fxstation::DelayLine m_history;
    fxstation::DelayLine m_releaseEcho;
    bool m_releaseActive;
    Release m_releaseType;
    double m_releaseElapsed;
    double m_releaseRead;
    // Top speed of the Back Spin, in times the normal speed
    static constexpr double kBackSpinPeak = 4.0;
    fxdsp::NeedleShaper m_backSpinNeedle;
    bool m_releaseWaiting;
    fxstation::Smoother m_liveGain;
};

} // namespace mixxx
