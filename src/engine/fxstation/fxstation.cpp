#include "engine/fxstation/fxstation.h"

#include <algorithm>
#include <cmath>

#include "control/controlobject.h"
#include "control/controlpotmeter.h"
#include "control/controlpushbutton.h"
#include "moc_fxstation.cpp"

namespace mixxx {

using namespace fxstation;

namespace {

const QString kGroup = QStringLiteral("[FxStation]");

// Memory is reserved for this sample rate (higher rates get shorter
// delays)
constexpr float kMaxSampleRate = 96000.f;
// The longest echo (two beats at 60 BPM) and the history Release plays from
constexpr float kMaxEchoSeconds = 2.2f;
constexpr float kHistorySeconds = 4.f;
// The longest roll of a band (a quarter of a beat at 40 BPM, and some)
constexpr float kMaxRollSeconds = 0.5f;
constexpr int kPitchWindowFrames = 2048;

// The choices of the multi-state buttons
constexpr int kSceneTypes = 10;
constexpr int kIsolateModes = 4;
constexpr int kReleaseTypes = 3;

// Crossovers of Isolate
constexpr float kLowMidFrequency = 300.f;
constexpr float kMidHighFrequency = 3000.f;

// Echo times of the first sub parameter, in beats
constexpr double kEchoBeats[] = {0.125, 0.25, 0.375, 0.5, 0.75, 1.0};
constexpr int kEchoBeatCount = 6;

// The scene is idle when its tail stays below this for a second
constexpr float kSilentLevel = 1e-5f;

std::unique_ptr<ControlObject> makeControl(const char* key, double defaultValue = 0.0) {
    auto pControl = std::make_unique<ControlObject>(ConfigKey(kGroup, QString::fromLatin1(key)));
    pControl->set(defaultValue);
    pControl->setDefaultValue(defaultValue);
    return pControl;
}

// Knobs and buttons with a range, so that MIDI controllers can be mapped to
// them (0-127 spans the range) and learned
std::unique_ptr<ControlObject> makeKnob(
        const char* key, double minimum, double maximum, double defaultValue) {
    auto pControl = std::make_unique<ControlPotmeter>(
            ConfigKey(kGroup, QString::fromLatin1(key)), minimum, maximum);
    pControl->set(defaultValue);
    pControl->setDefaultValue(defaultValue);
    return pControl;
}

/// A button: held (push), on and off (toggle, 2 states) or stepping through
/// several states on each press (toggle, e.g. the next scene)
std::unique_ptr<ControlObject> makeButton(
        const char* key, bool toggle, int states = 2, double defaultValue = 0.0) {
    auto pControl = std::make_unique<ControlPushButton>(
            ConfigKey(kGroup, QString::fromLatin1(key)), false, defaultValue);
    if (toggle) {
        pControl->setButtonMode(mixxx::control::ButtonMode::Toggle);
        pControl->setStates(states);
    }
    pControl->set(defaultValue);
    return pControl;
}

double fraction(double value) {
    return value - std::floor(value);
}

/// Linear interpolation of an instrument's sample at a position
float sampleAt(const Instrument& instrument, int channel, double position) {
    const int index = static_cast<int>(position);
    const float frac = static_cast<float>(position - index);
    const int frames = instrument.frames();
    const float a = instrument.samples[2 * index + channel];
    const float b = index + 1 < frames ? instrument.samples[2 * (index + 1) + channel] : 0.f;
    return a + (b - a) * frac;
}

} // namespace

FxStation::FxStation()
        : m_sampleRate(0.f),
          m_random(1234),
          m_beat(0.0),
          m_bpm(120.0),
          m_nudge(0.0),
          m_frameCount(0),
          m_taps{},
          m_tapCount(0),
          m_previousTap(false),
          m_previousRelease(false),
          m_previousClear(false),
          m_wetLevel(0.f),
          m_crushHold{0.f, 0.f},
          m_crushCounter(0),
          m_sceneSilentFrames(0),
          m_sceneActive(false),
          m_lastSceneType(-1),
          m_bandEnvelope{},
          m_isolateActive(false),
          m_loop{},
          m_loopHasSteps(false),
          m_rollOrigin(0.0),
          m_previousDivision(0),
          m_rollInstrument(0),
          m_releaseActive(false),
          m_releaseType(Release::VinylBrake),
          m_releaseElapsed(0.0),
          m_releaseRead(0.0),
          m_releaseWaiting(false) {
    m_pSceneType = makeButton("scene_type", true, kSceneTypes);
    m_pSceneOn = makeButton("scene_on", true);
    m_pSceneDial = makeKnob("scene_dial", 0.0, 1.0, 0.5);
    m_pSceneParam1 = makeKnob("scene_param1", 0.0, 1.0, 0.5);
    m_pSceneParam2 = makeKnob("scene_param2", 0.0, 1.0, 0.5);
    m_pIsolateMode = makeButton("isolate_mode", true, kIsolateModes);
    m_pIsolateBands[0] = makeKnob("isolate_low", -1.0, 1.0, 0.0);
    m_pIsolateBands[1] = makeKnob("isolate_mid", -1.0, 1.0, 0.0);
    m_pIsolateBands[2] = makeKnob("isolate_high", -1.0, 1.0, 0.0);
    m_pXPadInstrument = makeButton("xpad_instrument", true, kInstruments);
    m_pSourceInput = makeButton("fx_source_input", true, 2, 1.0);
    m_pSourceXPad = makeButton("fx_source_xpad", true, 2, 0.0);
    m_pXPadDivision = makeControl("xpad_division");
    for (int i = 0; i < kInstruments; ++i) {
        m_pXPadHits[i] = makeButton(
                QStringLiteral("xpad_hit_%1").arg(i + 1).toLatin1().constData(), false);
    }
    for (size_t i = 0; i < m_pXPadRolls.size(); ++i) {
        m_pXPadRolls[i] = makeButton(
                QStringLiteral("xpad_roll_%1").arg(i + 1).toLatin1().constData(), false);
    }
    m_pXPadPitch = makeKnob("xpad_pitch", -1.0, 1.0, 0.0);
    m_pXPadVolume = makeKnob("xpad_volume", 0.0, 1.0, 0.8);
    m_pXPadOverdub = makeButton("xpad_overdub", true);
    for (int i = 0; i < kInstruments; ++i) {
        m_pXPadMutes[i] = makeButton(
                QStringLiteral("xpad_mute_%1").arg(i + 1).toLatin1().constData(), true);
    }
    m_pXPadClear = makeButton("xpad_clear", false);
    m_pXPadBank = makeButton("xpad_bank", true, kBanks);
    m_pXPadReload = makeButton("xpad_reload", false);
    for (int i = 0; i < kBanks; ++i) {
        m_pXPadBankSounds[i] = makeControl(
                QStringLiteral("xpad_bank_sounds_%1").arg(i + 1).toLatin1().constData(),
                i == 0 ? kInstruments : 0);
    }
    m_pXPadLoopSteps = makeControl("xpad_loop_steps");
    m_pReleaseType = makeButton("release_type", true, kReleaseTypes);
    m_pRelease = makeButton("release", false);
    m_pReleaseActive = makeControl("release_active");
    m_pBpm = makeControl("bpm", 120.0);
    m_pBpmAuto = makeButton("bpm_auto", true, 2, 1.0);
    m_pTap = makeButton("tap", false);
    m_pNudge = makeControl("nudge");
    m_pQuantize = makeButton("quantize", true, 2, 1.0);
    m_pBeat = makeControl("beat");
    m_pSource = makeControl("source");

    // Everything the engine needs, reserved now
    const int maxRate = static_cast<int>(kMaxSampleRate);
    m_echo.allocate(static_cast<int>(kMaxEchoSeconds * maxRate));
    m_releaseEcho.allocate(static_cast<int>(kMaxEchoSeconds * maxRate));
    m_history.allocate(static_cast<int>(kHistorySeconds * maxRate));
    m_spiral.allocate(kPitchWindowFrames);
    m_reverb.allocate(kMaxSampleRate);
    for (auto& buffer : m_rollBuffers) {
        buffer.allocate(static_cast<int>(kMaxRollSeconds * maxRate));
    }
    m_liveGain.reset(1.f);

    m_pInstruments = std::make_unique<Instruments>();
    connect(m_pInstruments.get(),
            &Instruments::bankLoaded,
            this,
            [this](int index, int sounds) {
                m_pXPadBankSounds[index]->set(sounds);
            });
    connect(m_pXPadReload.get(),
            &ControlObject::valueChanged,
            this,
            [this](double value) {
                if (value > 0) {
                    m_pInstruments->reloadUserBanks();
                }
            });

    // The decks are created after the engine
    m_findDecksTimer.setInterval(1000);
    connect(&m_findDecksTimer, &QTimer::timeout, this, &FxStation::slotFindDecks);
    m_findDecksTimer.start();
}

FxStation::~FxStation() = default;

void FxStation::slotFindDecks() {
    bool allFound = true;
    for (int i = 0; i < kDecks; ++i) {
        const QString group = QStringLiteral("[Channel%1]").arg(i + 1);
        Deck& deck = m_decks[i];
        const auto find = [&](std::atomic<ControlObject*>& pControl, const char* key) {
            if (!pControl.load()) {
                pControl.store(ControlObject::getControl(
                        ConfigKey(group, QString::fromLatin1(key)), ControlFlag::NoWarnIfMissing));
            }
            allFound = allFound && pControl.load();
        };
        find(deck.pPlay, "play");
        find(deck.pBpm, "bpm");
        find(deck.pBeatDistance, "beat_distance");
        find(deck.pVolume, "volume");
        find(deck.pSyncLeader, "sync_leader");
    }
    if (allFound) {
        m_findDecksTimer.stop();
    }
}

void FxStation::prepare(float sampleRate) {
    m_sampleRate = sampleRate;
    m_reverb.setSampleRate(std::min(sampleRate, kMaxSampleRate));
    m_lowMid.set(kLowMidFrequency, sampleRate);
    m_midHigh.set(kMidHighFrequency, sampleRate);
    m_lowAllPass.set(kMidHighFrequency, sampleRate);
    m_echo.clear();
    m_releaseEcho.clear();
    m_spiral.clear();
    m_history.clear();
    m_sceneFilter.reset();
    m_wetFilter.reset();
    m_lowMid.reset();
    m_midHigh.reset();
    m_lowAllPass.reset();
}

double FxStation::beatFrames() const {
    return 60.0 / std::max(20.0, m_bpm) * m_sampleRate;
}

void FxStation::updateClock(int frames) {
    // The leading deck: the sync leader if it plays, else the loudest one
    // playing
    int source = -1;
    if (m_pBpmAuto->get() > 0) {
        float loudest = -1.f;
        for (int i = 0; i < kDecks; ++i) {
            const Deck& deck = m_decks[i];
            ControlObject* pPlay = deck.pPlay.load(std::memory_order_relaxed);
            ControlObject* pBpm = deck.pBpm.load(std::memory_order_relaxed);
            if (!pPlay || !pBpm || pPlay->get() <= 0 || pBpm->get() <= 0) {
                continue;
            }
            ControlObject* pLeader = deck.pSyncLeader.load(std::memory_order_relaxed);
            ControlObject* pVolume = deck.pVolume.load(std::memory_order_relaxed);
            const float loudness = (pLeader && pLeader->get() > 0 ? 10.f : 0.f) +
                    (pVolume ? static_cast<float>(pVolume->get()) : 1.f);
            if (loudness > loudest) {
                loudest = loudness;
                source = i;
            }
        }
    }
    if (source >= 0) {
        const Deck& deck = m_decks[source];
        m_bpm = deck.pBpm.load()->get();
        ControlObject* pDistance = deck.pBeatDistance.load(std::memory_order_relaxed);
        if (pDistance) {
            // The phase of the deck (and the nudge), keeping the count of
            // beats
            double difference = fraction(pDistance->get() + m_nudge) - fraction(m_beat);
            if (difference > 0.5) {
                difference -= 1.0;
            } else if (difference < -0.5) {
                difference += 1.0;
            }
            m_beat += difference;
        }
    }
    m_pSource->set(source + 1);

    // Nudge: a hundredth of a beat per tenth of a second while held
    const double nudge = m_pNudge->get();
    if (nudge != 0.0) {
        const double step = 0.1 * nudge * frames / m_sampleRate;
        if (source >= 0) {
            m_nudge += step;
        } else {
            m_beat += step;
        }
    }

    // Tap: the tempo of the last taps, the beat on the last one
    const bool tap = m_pTap->get() > 0;
    if (tap && !m_previousTap) {
        if (m_tapCount > 0 &&
                m_frameCount - m_taps[(m_tapCount - 1) % m_taps.size()] >
                        2 * static_cast<qint64>(m_sampleRate)) {
            m_tapCount = 0;
        }
        m_taps[m_tapCount % m_taps.size()] = m_frameCount;
        ++m_tapCount;
        const int taps = std::min<int>(m_tapCount, static_cast<int>(m_taps.size()));
        if (taps >= 2) {
            const qint64 first = m_taps[(m_tapCount - taps) % m_taps.size()];
            const double interval = static_cast<double>(m_frameCount - first) / (taps - 1);
            m_bpm = std::clamp(60.0 * m_sampleRate / interval, 40.0, 240.0);
            m_pBpmAuto->set(0);
        }
        m_beat = std::round(m_beat);
    }
    m_previousTap = tap;

    if (std::fabs(m_pBpm->get() - m_bpm) > 0.005) {
        m_pBpm->set(m_bpm);
    }
}

void FxStation::handleButtons() {
    const bool clear = m_pXPadClear->get() > 0;
    if (clear && !m_previousClear) {
        m_loop = {};
        m_loopHasSteps = false;
        m_pXPadLoopSteps->set(0);
    }
    m_previousClear = clear;

    // Hits of the pads, recorded into the loop while overdubbing
    for (int i = 0; i < kInstruments; ++i) {
        const double velocity = m_pXPadHits[i]->get();
        const bool hit = velocity > 0;
        if (hit && !m_previousHits[i]) {
            triggerInstrument(i,
                    static_cast<float>(m_pXPadVolume->get() * std::min(1.0, velocity)));
            if (m_pXPadOverdub->get() > 0) {
                const int step = ((static_cast<int>(std::lround(m_beat * 16.0)) % kLoopSteps) +
                                         kLoopSteps) %
                        kLoopSteps;
                m_loop[i][step] = true;
                m_loopHasSteps = true;
            }
        }
        m_previousHits[i] = hit;
    }

    const bool release = m_pRelease->get() > 0;
    if (release && !m_previousRelease) {
        startRelease();
    }
    m_previousRelease = release;
}

void FxStation::startRelease() {
    m_releaseActive = true;
    m_releaseWaiting = false;
    m_releaseType = static_cast<Release>(std::clamp(static_cast<int>(m_pReleaseType->get()), 0, 2));
    m_releaseElapsed = 0.0;
    // From the last frame of the history, see processRelease()
    m_releaseRead = static_cast<double>(m_history.writeIndex()) - 1.0;
    m_releaseEcho.clear();
    m_pReleaseActive->set(1);

    // Everything else off at once
    m_pSceneOn->set(0);
    m_sceneSend.reset(0.f);
    m_sceneAmount.reset(0.f);
    m_wetLevel = 0.f;
    m_echo.clear();
    m_spiral.clear();
    m_reverb.clear();
    m_sceneActive = false;
    for (auto& pBand : m_pIsolateBands) {
        pBand->set(0);
    }
    m_pXPadDivision->set(0);
    for (auto& pRoll : m_pXPadRolls) {
        pRoll->set(0);
    }
    m_pXPadOverdub->set(0);
    m_loop = {};
    m_loopHasSteps = false;
    m_pXPadLoopSteps->set(0);
    for (auto& voice : m_voices) {
        voice.active = false;
    }
}

bool FxStation::processScene(float* pLeft, float* pRight) {
    const auto type = static_cast<Scene>(m_lastSceneType);
    const float send = m_sceneSend.value;
    const float amount = m_sceneAmount.value;
    const float wetLevel = m_wetLevel;
    const float param2 = static_cast<float>(m_pSceneParam2->get());
    const float dryLeft = *pLeft;
    const float dryRight = *pRight;
    float left = dryLeft;
    float right = dryRight;
    float wetLeft = 0.f;
    float wetRight = 0.f;

    switch (type) {
    case Scene::BpfEcho:
    case Scene::Echo:
    case Scene::HpfEcho:
    case Scene::LpfEcho:
    case Scene::CrushEcho:
    case Scene::SpiralUp:
    case Scene::SpiralDown: {
        const float delay = m_echoDelay.value;
        wetLeft = m_echo.read(0, delay);
        wetRight = m_echo.read(1, delay);
        float feedbackLeft = wetLeft;
        float feedbackRight = wetRight;
        float inLeft = dryLeft * send;
        float inRight = dryRight * send;
        float feedback = 0.25f + 0.6f * param2;
        if (type == Scene::BpfEcho) {
            // The repeats through a band pass that opens with the dial
            float low, band, high;
            m_wetFilter.process(0, inLeft + feedbackLeft * feedback, &low, &band, &high);
            const float bandLeft = band;
            m_wetFilter.process(1, inRight + feedbackRight * feedback, &low, &band, &high);
            m_echo.write(softClip(bandLeft), softClip(band));
        } else if (type == Scene::SpiralUp || type == Scene::SpiralDown) {
            // Each repeat a step higher or lower
            const float semitones = 1.f + 6.f * param2;
            const float ratio = std::pow(2.f,
                    (type == Scene::SpiralUp ? semitones : -semitones) / 12.f);
            m_spiral.process(&feedbackLeft, &feedbackRight, ratio);
            feedback = 0.55f + 0.35f * wetLevel;
            m_echo.write(softClip(inLeft + feedbackLeft * feedback),
                    softClip(inRight + feedbackRight * feedback));
        } else {
            m_echo.write(softClip(inLeft + feedbackLeft * feedback),
                    softClip(inRight + feedbackRight * feedback));
        }
        left = dryLeft + wetLeft * wetLevel;
        right = dryRight + wetRight * wetLevel;
        if (type == Scene::HpfEcho || type == Scene::LpfEcho) {
            float low, band, high;
            m_sceneFilter.process(0, left, &low, &band, &high);
            left = type == Scene::HpfEcho ? high : low;
            m_sceneFilter.process(1, right, &low, &band, &high);
            right = type == Scene::HpfEcho ? high : low;
        } else if (type == Scene::CrushEcho) {
            // Fewer bits and a lower rate with the dial, and some drive
            const float bits = 16.f - 12.f * amount;
            const float steps = std::pow(2.f, bits);
            const int hold = 1 + static_cast<int>(7.f * amount);
            if (++m_crushCounter >= hold) {
                m_crushCounter = 0;
                m_crushHold[0] = std::round(softClip(left * (1.f + 2.f * amount)) * steps) / steps;
                m_crushHold[1] = std::round(softClip(right * (1.f + 2.f * amount)) * steps) / steps;
            }
            left = left + (m_crushHold[0] - left) * amount;
            right = right + (m_crushHold[1] - right) * amount;
        }
        break;
    }
    case Scene::Noise: {
        std::uniform_real_distribution<float> noise(-1.f, 1.f);
        const float level = 0.4f * amount * amount;
        float low, band, high;
        m_sceneFilter.process(0, noise(m_random), &low, &band, &high);
        wetLeft = (high + band) * level;
        m_sceneFilter.process(1, noise(m_random), &low, &band, &high);
        wetRight = (high + band) * level;
        left = dryLeft + wetLeft;
        right = dryRight + wetRight;
        break;
    }
    case Scene::ReverbUp:
    case Scene::ReverbDown: {
        float inLeft = dryLeft * send;
        float inRight = dryRight * send;
        if (type == Scene::ReverbUp) {
            // Thinner and thinner into the reverb
            float low, band, high;
            m_wetFilter.process(0, inLeft, &low, &band, &high);
            inLeft = high;
            m_wetFilter.process(1, inRight, &low, &band, &high);
            inRight = high;
        }
        m_reverb.process(inLeft, inRight, &wetLeft, &wetRight);
        if (type == Scene::ReverbUp) {
            left = dryLeft + wetLeft * 1.5f * wetLevel;
            right = dryRight + wetRight * 1.5f * wetLevel;
        } else {
            // The room swallows the track
            const float dryGain = 1.f - 0.85f * amount;
            left = dryLeft * dryGain + wetLeft * 1.5f * wetLevel;
            right = dryRight * dryGain + wetRight * 1.5f * wetLevel;
            float low, band, high;
            m_sceneFilter.process(0, left, &low, &band, &high);
            left = low;
            m_sceneFilter.process(1, right, &low, &band, &high);
            right = low;
        }
        break;
    }
    default:
        break;
    }
    *pLeft = left;
    *pRight = right;
    return std::fabs(wetLeft) + std::fabs(wetRight) > kSilentLevel;
}

void FxStation::processIsolate(float* pLeft, float* pRight, double beat) {
    const auto mode = static_cast<IsolateMode>(
            std::clamp(static_cast<int>(m_pIsolateMode->get()), 0, 3));
    float input[2] = {*pLeft, *pRight};
    float output[2] = {0.f, 0.f};
    const double rollFrames = beatFrames() / 8.0;
    const float gateCoefficient = 1.f - std::exp(-1.f / (0.002f * m_sampleRate));
    for (int band = 0; band < kBands; ++band) {
        const float value = static_cast<float>(m_pIsolateBands[band]->get());
        float target = 1.f;
        bool roll = false;
        switch (mode) {
        case IsolateMode::Isolator:
            // Down to silence, or up to twice as loud
            target = 1.f + value;
            break;
        case IsolateMode::CutAdd: {
            // Cut: off on the second half of every eighth; Add: accents on
            // the sixteenths
            if (value < 0.f) {
                target = fraction(beat * 2.0) < 0.5 ? 1.f : 1.f + value;
            } else {
                const float pulse = std::exp(-static_cast<float>(fraction(beat * 4.0)) * 6.f);
                target = 1.f + 1.5f * value * pulse;
            }
            break;
        }
        case IsolateMode::TransRoll:
            if (value < 0.f) {
                target = fraction(beat * 4.0) < 0.5 ? 1.f : 1.f + value;
            } else if (value > 0.05f) {
                roll = true;
            }
            break;
        default:
            break;
        }
        m_bandGain[band].step(target, gateCoefficient);

        BandRoll& bandRoll = m_rolls[band];
        if (roll && !bandRoll.active) {
            bandRoll.active = true;
            bandRoll.length = std::clamp(static_cast<int>(rollFrames),
                    64,
                    m_rollBuffers[band].maxFrames() - 2);
            bandRoll.recorded = 0;
            bandRoll.playPosition = 0;
        } else if (!roll) {
            bandRoll.active = false;
        }
    }

    // Split both channels into the three bands
    float bands[kBands][2];
    for (int channel = 0; channel < 2; ++channel) {
        float low, rest, mid, high;
        m_lowMid.split(channel, input[channel], &low, &rest);
        m_midHigh.split(channel, rest, &mid, &high);
        // The low band through the same phase as the others
        float lowLow, lowHigh;
        m_lowAllPass.split(channel, low, &lowLow, &lowHigh);
        bands[0][channel] = lowLow + lowHigh;
        bands[1][channel] = mid;
        bands[2][channel] = high;
    }

    for (int band = 0; band < kBands; ++band) {
        const float value = static_cast<float>(m_pIsolateBands[band]->get());
        float gain = m_bandGain[band].value;
        BandRoll& bandRoll = m_rolls[band];
        // Gate/Drive
        if (mode == IsolateMode::GateDrive && value < 0.f) {
            const float level = 0.5f * (std::fabs(bands[band][0]) + std::fabs(bands[band][1]));
            float& envelope = m_bandEnvelope[band];
            envelope += (level - envelope) * (level > envelope ? 0.3f : 0.0015f);
            const float threshold = 0.25f * -value * -value;
            gain *= envelope > threshold ? 1.f : std::max(0.f, envelope / threshold);
        }
        for (int channel = 0; channel < 2; ++channel) {
            float sample = bands[band][channel];
            if (mode == IsolateMode::GateDrive && value > 0.f) {
                const float amount = 1.f + 8.f * value;
                sample = softClip(sample * amount) / (1.f + 2.f * value);
            }
            output[channel] += sample * gain;
        }
        // Trans/Roll: the band repeats the last eighth of a beat
        if (bandRoll.active) {
            DelayLine& buffer = m_rollBuffers[band];
            if (bandRoll.recorded < bandRoll.length) {
                buffer.write(bands[band][0], bands[band][1]);
                ++bandRoll.recorded;
            } else {
                const float mix = std::clamp(value, 0.f, 1.f);
                const double position = static_cast<double>(buffer.writeIndex()) -
                        bandRoll.length + bandRoll.playPosition;
                for (int channel = 0; channel < 2; ++channel) {
                    const float repeated = buffer.at(channel, position);
                    output[channel] += (repeated - bands[band][channel]) * mix * gain;
                }
                if (++bandRoll.playPosition >= bandRoll.length) {
                    bandRoll.playPosition = 0;
                }
            }
        }
    }
    *pLeft = output[0];
    *pRight = output[1];
}

void FxStation::triggerInstrument(int instrument, float velocity) {
    const int bankIndex = std::clamp(static_cast<int>(m_pXPadBank->get()), 0, kBanks - 1);
    const Bank* pBank = m_pInstruments->bank(bankIndex);
    const Bank* pFactory = m_pInstruments->bank(0);
    const Instrument* pInstrument = nullptr;
    if (pBank && pBank->instruments[instrument].frames() > 0) {
        pInstrument = &pBank->instruments[instrument];
    } else if (pFactory) {
        pInstrument = &pFactory->instruments[instrument];
    }
    if (!pInstrument || pInstrument->frames() == 0) {
        return;
    }
    // A new hit chokes the last one of the instrument (rolls stay clean)
    Voice* pFree = nullptr;
    for (auto& voice : m_voices) {
        if (voice.active && voice.instrument == instrument && voice.fade == 0.f) {
            voice.fade = 1.f / (0.003f * m_sampleRate);
        }
        if (!voice.active && !pFree) {
            pFree = &voice;
        }
    }
    if (!pFree) {
        // The oldest voice
        pFree = &*std::max_element(m_voices.begin(), m_voices.end(), [](const Voice& a, const Voice& b) {
            return a.position < b.position;
        });
    }
    const double semitones = 12.0 * m_pXPadPitch->get();
    pFree->pInstrument = pInstrument;
    pFree->instrument = instrument;
    pFree->position = 0.0;
    pFree->rate = pInstrument->sampleRate / m_sampleRate * std::pow(2.0, semitones / 12.0);
    pFree->gain = velocity;
    pFree->fade = 0.f;
    pFree->active = true;
}

int FxStation::xpadDivision() const {
    const int ribbon = std::clamp(static_cast<int>(m_pXPadDivision->get()), 0, 5);
    if (ribbon > 0) {
        return ribbon;
    }
    for (int i = static_cast<int>(m_pXPadRolls.size()) - 1; i >= 0; --i) {
        if (m_pXPadRolls[i]->get() > 0) {
            return i + 1;
        }
    }
    return 0;
}

void FxStation::processXPad(float* pLeft, float* pRight, double beat, double previousBeat) {
    const int division = xpadDivision();
    const int instrument = std::clamp(static_cast<int>(m_pXPadInstrument->get()), 0, kInstruments - 1);
    const bool quantize = m_pQuantize->get() > 0;
    const float volume = static_cast<float>(m_pXPadVolume->get());

    // The loop of the last bar: a step every sixteenth of a beat
    const auto stepAt = [](double atBeat) {
        return static_cast<int>(std::floor(atBeat * 16.0));
    };
    if (m_loopHasSteps && stepAt(beat) != stepAt(previousBeat)) {
        const int step = ((stepAt(beat) % kLoopSteps) + kLoopSteps) % kLoopSteps;
        for (int i = 0; i < kInstruments; ++i) {
            const bool rolling = division > 0 && i == instrument;
            if (m_loop[i][step] && !rolling && m_pXPadMutes[i]->get() <= 0) {
                triggerInstrument(i, volume);
            }
        }
    }

    // The roll of the ribbon: 1/1 to 1/16 of a beat
    if (division > 0) {
        const double stepBeats = 1.0 / std::pow(2.0, division - 1);
        bool hit = false;
        if (division != m_previousDivision && m_previousDivision == 0) {
            m_rollOrigin = quantize ? 0.0 : beat;
            m_rollInstrument = instrument;
            // Unquantized: at once
            hit = !quantize;
        } else {
            const double origin = quantize ? 0.0 : m_rollOrigin;
            hit = std::floor((beat - origin) / stepBeats) !=
                    std::floor((previousBeat - origin) / stepBeats);
        }
        if (hit) {
            triggerInstrument(instrument, volume);
            if (m_pXPadOverdub->get() > 0) {
                const int step = ((static_cast<int>(std::lround(beat * 16.0)) % kLoopSteps) +
                                         kLoopSteps) %
                        kLoopSteps;
                m_loop[instrument][step] = true;
                if (!m_loopHasSteps) {
                    m_loopHasSteps = true;
                }
            }
        }
    }
    m_previousDivision = division;

    // The voices
    float left = 0.f;
    float right = 0.f;
    for (auto& voice : m_voices) {
        if (!voice.active) {
            continue;
        }
        const Instrument& sound = *voice.pInstrument;
        if (voice.position >= sound.frames() - 1) {
            voice.active = false;
            continue;
        }
        if (voice.fade > 0.f) {
            voice.gain -= voice.fade;
            if (voice.gain <= 0.f) {
                voice.active = false;
                continue;
            }
        }
        const float gain = voice.gain;
        left += sampleAt(sound, 0, voice.position) * gain;
        right += sampleAt(sound, 1, voice.position) * gain;
        voice.position += voice.rate;
    }
    *pLeft += left;
    *pRight += right;
}

void FxStation::processRelease(float* pLeft, float* pRight, double beat, double previousBeat) {
    const double beatLength = beatFrames();
    const bool quantize = m_pQuantize->get() > 0;
    float outLeft = 0.f;
    float outRight = 0.f;
    double duration = 0.0;

    if (!m_releaseWaiting) {
        const double t = m_releaseElapsed / beatLength;
        switch (m_releaseType) {
        case Release::VinylBrake: {
            // The platter slows down to a stop over two beats
            duration = 2.0;
            const double rate = std::max(0.0, 1.0 - t / duration);
            outLeft = m_history.at(0, m_releaseRead);
            outRight = m_history.at(1, m_releaseRead);
            m_releaseRead += rate * rate;
            break;
        }
        case Release::BackSpin: {
            // Pulled back fast, slowing down over a beat and a half
            duration = 1.5;
            const double rate = -4.0 * std::pow(std::max(0.0, 1.0 - t / duration), 2.0);
            const float level = static_cast<float>(std::min(1.0, 4.0 * (1.0 - t / duration)));
            outLeft = m_history.at(0, m_releaseRead) * level;
            outRight = m_history.at(1, m_releaseRead) * level;
            m_releaseRead += rate;
            break;
        }
        case Release::Echo: {
            // The last quarter of a beat into a fading echo of half beats
            duration = 4.0;
            const float delay = static_cast<float>(
                    std::min(0.5 * beatLength, m_releaseEcho.maxFrames() - 4.0));
            const float echoLeft = m_releaseEcho.read(0, delay);
            const float echoRight = m_releaseEcho.read(1, delay);
            const float input = t < 0.25 ? 1.f : 0.f;
            m_releaseEcho.write(*pLeft * input + echoLeft * 0.65f,
                    *pRight * input + echoRight * 0.65f);
            const float level = static_cast<float>(std::max(0.0, 1.0 - t / duration));
            outLeft = (t < 0.25 ? *pLeft : echoLeft) * level;
            outRight = (t < 0.25 ? *pRight : echoRight) * level;
            break;
        }
        }
        m_releaseElapsed += 1.0;
        if (m_releaseElapsed / beatLength >= duration) {
            m_releaseWaiting = true;
        }
    }
    // Back on the next beat (or at once without quantize)
    if (m_releaseWaiting &&
            (!quantize || std::floor(beat) != std::floor(previousBeat))) {
        m_releaseActive = false;
        m_releaseWaiting = false;
        m_liveGain.reset(0.f);
        m_pReleaseActive->set(0);
        return;
    }
    *pLeft = outLeft;
    *pRight = outRight;
}

void FxStation::process(CSAMPLE* pBuffer, int frames, float sampleRate) {
    if (sampleRate <= 0.f || frames <= 0) {
        return;
    }
    if (sampleRate != m_sampleRate) {
        prepare(sampleRate);
    }
    updateClock(frames);
    handleButtons();

    // Scene parameters, smoothed per buffer
    const int sceneType = std::clamp(static_cast<int>(m_pSceneType->get()),
            0,
            static_cast<int>(Scene::Count) - 1);
    if (sceneType != m_lastSceneType) {
        // Another effect: no tail of the last one
        m_echo.clear();
        m_spiral.clear();
        m_reverb.clear();
        m_sceneFilter.reset();
        m_wetFilter.reset();
        m_lastSceneType = sceneType;
    }
    const bool sceneOn = m_pSceneOn->get() > 0;
    const float dial = static_cast<float>(m_pSceneDial->get());
    const float param1 = static_cast<float>(m_pSceneParam1->get());
    const float param2 = static_cast<float>(m_pSceneParam2->get());
    const float bufferCoefficient = 1.f - std::exp(-static_cast<float>(frames) / (0.03f * m_sampleRate));
    m_sceneAmount.step(sceneOn ? dial : 0.f, bufferCoefficient);
    if (sceneOn) {
        m_wetLevel = m_sceneAmount.value;
    }
    const float sendTarget = sceneOn ? dial : 0.f;
    if (sceneOn) {
        m_sceneActive = true;
        m_sceneSilentFrames = 0;
    }
    const auto scene = static_cast<Scene>(sceneType);
    const float amount = m_sceneAmount.value;
    switch (scene) {
    case Scene::BpfEcho:
        m_wetFilter.set(250.f * std::pow(2.f, 5.f * amount), 1.5f + 4.f * param2, m_sampleRate);
        break;
    case Scene::HpfEcho:
        m_sceneFilter.set(20.f * std::pow(2.f, 7.f * amount), 0.9f, m_sampleRate);
        break;
    case Scene::LpfEcho:
        m_sceneFilter.set(20000.f * std::pow(2.f, -6.f * amount), 0.9f, m_sampleRate);
        break;
    case Scene::Noise:
        m_sceneFilter.set(200.f * std::pow(2.f, 5.5f * amount) * std::pow(2.f, 3.f * (param1 - 0.5f)),
                0.7f + 6.f * param2,
                m_sampleRate);
        break;
    case Scene::ReverbUp:
        m_wetFilter.set(100.f * std::pow(2.f, 5.f * amount), 0.7f, m_sampleRate);
        m_reverb.set(0.5f + 0.5f * param1, param2);
        break;
    case Scene::ReverbDown:
        m_sceneFilter.set(20000.f * std::pow(2.f, -4.3f * amount), 0.7f, m_sampleRate);
        m_reverb.set(0.5f + 0.5f * param1, param2);
        break;
    default:
        break;
    }
    // The echo time of the first sub parameter, in beats
    const int echoIndex = std::clamp(static_cast<int>(param1 * kEchoBeatCount), 0, kEchoBeatCount - 1);
    const float echoTarget = static_cast<float>(std::min(kEchoBeats[echoIndex] * beatFrames(),
            static_cast<double>(m_echo.maxFrames() - 4)));
    if (m_echoDelay.value <= 0.f) {
        m_echoDelay.reset(echoTarget);
    }

    // Isolate is on while a band is turned (or a roll plays)
    bool isolateWanted = false;
    for (const auto& pBand : m_pIsolateBands) {
        isolateWanted = isolateWanted || std::fabs(pBand->get()) > 0.01;
    }
    if (isolateWanted && !m_isolateActive) {
        m_isolateActive = true;
        m_isolateMix.reset(0.f);
        m_lowMid.reset();
        m_midHigh.reset();
        m_lowAllPass.reset();
        for (auto& gain : m_bandGain) {
            gain.reset(1.f);
        }
    }

    const bool xpadActive = xpadDivision() > 0 || m_loopHasSteps ||
            std::any_of(m_voices.begin(), m_voices.end(), [](const Voice& voice) {
                return voice.active;
            });

    const double beatIncrement = m_bpm / 60.0 / m_sampleRate;
    const float sampleCoefficient = 1.f - std::exp(-1.f / (0.005f * m_sampleRate));
    const float delayCoefficient = 1.f - std::exp(-1.f / (0.05f * m_sampleRate));
    bool sceneAudible = false;
    // FX SOURCE, like the unit: Scene and Isolate process the music, the
    // X-Pad, or both; the rest passes by them
    const bool sourceInput = m_pSourceInput->get() > 0;
    const bool sourceXPad = m_pSourceXPad->get() > 0;
    for (int frame = 0; frame < frames; ++frame) {
        const double previousBeat = m_beat;
        m_beat += beatIncrement;
        const float inputLeft = pBuffer[2 * frame];
        const float inputRight = pBuffer[2 * frame + 1];
        float xpadLeft = 0.f;
        float xpadRight = 0.f;
        if (xpadActive) {
            processXPad(&xpadLeft, &xpadRight, m_beat, previousBeat);
        }
        float left = (sourceInput ? inputLeft : 0.f) + (sourceXPad ? xpadLeft : 0.f);
        float right = (sourceInput ? inputRight : 0.f) + (sourceXPad ? xpadRight : 0.f);
        const float bypassLeft = (sourceInput ? 0.f : inputLeft) + (sourceXPad ? 0.f : xpadLeft);
        const float bypassRight = (sourceInput ? 0.f : inputRight) + (sourceXPad ? 0.f : xpadRight);

        if (m_isolateActive) {
            const float dryLeft = left;
            const float dryRight = right;
            processIsolate(&left, &right, m_beat);
            const float mix = m_isolateMix.step(isolateWanted ? 1.f : 0.f, sampleCoefficient);
            left = dryLeft + (left - dryLeft) * mix;
            right = dryRight + (right - dryRight) * mix;
        }
        if (m_sceneActive) {
            m_sceneSend.step(sendTarget, sampleCoefficient);
            m_echoDelay.step(echoTarget, delayCoefficient);
            sceneAudible = processScene(&left, &right) || sceneAudible;
        }
        left += bypassLeft;
        right += bypassRight;
        m_history.write(left, right);
        if (m_releaseActive) {
            processRelease(&left, &right, m_beat, previousBeat);
        } else if (m_liveGain.value < 1.f) {
            const float gain = m_liveGain.step(1.f, 0.01f);
            left *= gain;
            right *= gain;
            if (gain > 0.999f) {
                m_liveGain.reset(1.f);
            }
        }
        // Never beyond full scale
        pBuffer[2 * frame] = std::clamp(left, -1.f, 1.f);
        pBuffer[2 * frame + 1] = std::clamp(right, -1.f, 1.f);
    }
    m_frameCount += frames;

    // Idle sections cost nothing
    if (m_sceneActive && !sceneOn && !sceneAudible) {
        m_sceneSilentFrames += frames;
        if (m_sceneSilentFrames > static_cast<int>(m_sampleRate)) {
            m_sceneActive = false;
            m_echo.clear();
            m_spiral.clear();
            m_reverb.clear();
        }
    }
    if (m_isolateActive && !isolateWanted && m_isolateMix.value < 0.001f) {
        m_isolateActive = false;
        for (auto& roll : m_rolls) {
            roll.active = false;
        }
    }
    if (m_loopHasSteps) {
        int steps = 0;
        for (const auto& instrument : m_loop) {
            steps += static_cast<int>(std::count(instrument.begin(), instrument.end(), true));
        }
        m_pXPadLoopSteps->set(steps);
    }
    m_pBeat->set(std::fmod(std::fmod(m_beat, 4.0) + 4.0, 4.0));
}

} // namespace mixxx
