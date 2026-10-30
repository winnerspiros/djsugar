#include "analyzer/androidkickgrid.h"

#include <QDebug>
#include <algorithm>
#include <cmath>

namespace mixxx {
namespace android {

namespace {

constexpr double kLowPassHz = 120;
// About 23 ms at 44.1 kHz, longer than a period of the low frequencies
constexpr std::size_t kSmoothingHops = 4;
// Onsets weaker than this part of the strongest one of the first seconds
// after the track starts are not kicks
constexpr double kStrongOnset = 0.5;
constexpr double kTrackStartOnset = 0.1;
constexpr double kStrongOnsetWindowSeconds = 4;
constexpr std::size_t kMaxKicks = 8;
constexpr std::size_t kMinKicks = 4;
// How far from the beat of a kick its low frequencies rise the most, in
// beats (about 120 ms at 128 BPM)
constexpr double kLagBefore = 0.05;
constexpr double kLagAfter = 0.25;
// Doubled house and techno; drum & bass (~175 BPM, kicks often on every
// second beat) is left alone
constexpr double kMinBpmToHalve = 200;
constexpr double kHalveGain = 1.5;
// The best position must have this much more kick than half a beat away
constexpr double kMinGain = 1.2;

// The phase of a position on the grid, between -0.5 and 0.5 beats
double phaseOf(double position, double firstBeat, double beatLength) {
    const double beats = (position - firstBeat) / beatLength + 0.5;
    return beats - std::floor(beats) - 0.5;
}

} // namespace

void KickGrid::initialize(audio::SampleRate sampleRate) {
    m_sampleRate = sampleRate;
    m_lowPassCoefficient = std::exp(-2 * M_PI * kLowPassHz / sampleRate.value());
    m_lowPass1 = 0;
    m_lowPass2 = 0;
    m_hopEnergy = 0;
    m_hopFrames = 0;
    m_maxHops = static_cast<SINT>(kSecondsKept * sampleRate.value() / kHopFrames);
    m_envelope.clear();
    m_envelope.reserve(m_maxHops);
}

void KickGrid::process(const CSAMPLE* pSamples, SINT frames, int channels) {
    if (channels < 1 || static_cast<SINT>(m_envelope.size()) >= m_maxHops) {
        return;
    }
    const double a = m_lowPassCoefficient;
    for (SINT frame = 0; frame < frames; ++frame) {
        double mono = 0;
        for (int channel = 0; channel < channels; ++channel) {
            mono += pSamples[frame * channels + channel];
        }
        mono /= channels;
        // Two one-pole low-passes
        m_lowPass1 = (1 - a) * mono + a * m_lowPass1;
        m_lowPass2 = (1 - a) * m_lowPass1 + a * m_lowPass2;
        m_hopEnergy += m_lowPass2 * m_lowPass2;
        if (++m_hopFrames == kHopFrames) {
            m_envelope.push_back(static_cast<float>(std::sqrt(m_hopEnergy / kHopFrames)));
            m_hopEnergy = 0;
            m_hopFrames = 0;
            if (static_cast<SINT>(m_envelope.size()) >= m_maxHops) {
                return;
            }
        }
    }
}

BeatsPointer KickGrid::correct(const BeatsPointer& pBeats) const {
    if (!pBeats || m_envelope.size() < 3 || !m_sampleRate.isValid()) {
        return pBeats;
    }
    // The level over the last few hops: a hop is shorter than a period of a
    // kick's low frequencies, so the level of single hops jumps up and down
    // while a kick sounds, which would look like onsets after the kick
    std::vector<double> level(m_envelope.size(), 0);
    double sum = 0;
    for (std::size_t i = 0; i < m_envelope.size(); ++i) {
        sum += static_cast<double>(m_envelope[i]) * m_envelope[i];
        if (i >= kSmoothingHops) {
            sum -= static_cast<double>(m_envelope[i - kSmoothingHops]) *
                    m_envelope[i - kSmoothingHops];
        }
        level[i] = std::sqrt(std::max(0.0, sum) / kSmoothingHops);
    }
    // Onsets: the rise of the low frequencies from one hop to the next
    std::vector<double> onset(m_envelope.size(), 0);
    double strongest = 0;
    for (std::size_t i = 1; i < level.size(); ++i) {
        onset[i] = std::max(0.0, level[i] - level[i - 1]);
        strongest = std::max(strongest, onset[i]);
    }
    if (strongest <= 0) {
        return pBeats;
    }
    // Where the track starts: the first onset of some strength, at its peak
    std::size_t start = 1;
    while (start < onset.size() && onset[start] <= kTrackStartOnset * strongest) {
        ++start;
    }
    while (start + 1 < onset.size() && onset[start + 1] >= onset[start]) {
        ++start;
    }
    if (start + 1 >= onset.size()) {
        return pBeats;
    }
    // The first strong onsets from there: without a few, there's no kick to
    // follow
    const std::size_t windowEnd = std::min(onset.size(),
            start +
                    static_cast<std::size_t>(kStrongOnsetWindowSeconds *
                            m_sampleRate.value() / kHopFrames));
    const double threshold = kStrongOnset *
            *std::max_element(onset.begin() + start, onset.begin() + windowEnd);
    std::vector<double> kicks;
    for (std::size_t i = start; i + 1 < onset.size() && kicks.size() < kMaxKicks; ++i) {
        if (onset[i] >= threshold && onset[i] >= onset[i - 1] && onset[i] > onset[i + 1]) {
            kicks.push_back(i * kHopFrames + kHopFrames / 2.0);
        }
    }
    if (kicks.size() < kMinKicks) {
        return pBeats;
    }
    const double trackStart = start * kHopFrames + kHopFrames / 2.0;

    BeatsPointer pResult = pBeats;
    const Bpm bpm = pBeats->getBpmInRange(audio::kStartFramePos,
            audio::FramePos(m_envelope.size() * static_cast<double>(kHopFrames)));
    if (!bpm.isValid()) {
        return pBeats;
    }
    double beatLength = 60.0 * m_sampleRate.value() / bpm.value();

    // Score a position of the grid by the low-frequency onsets on its beats
    if (!pResult->firstBeat().isValid()) {
        return pResult;
    }
    double firstBeat = pResult->firstBeat().value();
    const auto scoreAt = [&](double phase, double length) {
        double score = 0;
        for (double position = firstBeat + phase * length;
                position < onset.size() * static_cast<double>(kHopFrames);
                position += length) {
            const auto hop = static_cast<std::size_t>(std::max(0.0, position) / kHopFrames);
            double best = 0;
            for (std::size_t i = hop > 0 ? hop - 1 : 0; i <= hop + 1 && i < onset.size(); ++i) {
                best = std::max(best, onset[i]);
            }
            score += best;
        }
        return score;
    };
    // A doubled BPM: the kicks are on every second beat only, so one half of
    // the beats has much more kick than the other
    if (bpm.value() >= kMinBpmToHalve) {
        const double even = scoreAt(0, 2 * beatLength);
        const double odd = scoreAt(0.5, 2 * beatLength);
        if (std::max(even, odd) > kHalveGain * std::min(even, odd)) {
            const auto halved = pResult->tryScale(Beats::BpmScale::Halve);
            if (halved) {
                pResult = *halved;
                beatLength *= 2;
                firstBeat = pResult->firstBeat().value();
                qDebug() << "KickGrid: halved the BPM of" << bpm.value();
            }
        }
    }

    // Where in the beat the low frequencies rise the most
    const double step = static_cast<double>(kHopFrames) / beatLength;
    double bestPhase = 0;
    double bestScore = scoreAt(0, beatLength);
    for (double phase = -0.5; phase < 0.5; phase += step) {
        const double score = scoreAt(phase, beatLength);
        if (score > bestScore) {
            bestScore = score;
            bestPhase = phase;
        }
    }
    // The beat tracker is precise, but may follow the off-beats. The low
    // frequencies of a kick rise some time after its attack, how long
    // depends on the kick, so only the side of the beat is taken from them:
    // the grid is moved by exactly half a beat if they rise just after the
    // off-beats, and the track starts there too (tracks start on a beat).
    const auto afterOffBeat = [&](double position) {
        const double phase = phaseOf(position - beatLength / 2, firstBeat, beatLength);
        return phase >= -kLagBefore && phase <= kLagAfter;
    };
    const double bestPosition = firstBeat + bestPhase * beatLength;
    const bool kicksOffBeat = afterOffBeat(bestPosition);
    const bool startOffBeat = afterOffBeat(trackStart);
    qDebug() << "KickGrid: low frequencies rise at" << bestPhase
             << "beats; after the off-beats:" << kicksOffBeat
             << "; track starts after an off-beat:" << startOffBeat;
    if (kicksOffBeat && startOffBeat &&
            bestScore > kMinGain * scoreAt(bestPhase + 0.5, beatLength)) {
        const auto moved = pResult->tryTranslateBeats(0.5);
        if (moved) {
            pResult = *moved;
            qDebug() << "KickGrid: moved the beatgrid by half a beat onto the kicks";
        }
    }
    return pResult;
}

} // namespace android
} // namespace mixxx
