#include "qml/qmlframeclock.h"

#include <QDebug>
#include <QElapsedTimer>
#include <algorithm>
#include <atomic>
#include <cmath>

namespace mixxx {
namespace qml {
namespace frameclock {

namespace {

// A 60 Hz display at 60 frames per second until a waveform tells better
std::atomic<qint64> s_periodMicros{16667};
std::atomic<qint64> s_intervalMicros{16667};
// The time of the last frame shown, in microseconds of epoch()
std::atomic<qint64> s_lastFrameMicros{0};
// The time of the last frame shown with a waveform drawn in it, 0 if none
std::atomic<qint64> s_lastTickMicros{0};

// Adapting to the device, see frameShown()
std::atomic<bool> s_everySecondRefresh{false};
std::atomic<qint64> s_everySecondRefreshSinceMicros{0};
// Frames looked at, and those that took longer than a refresh. Render
// thread only.
int s_frames = 0;
int s_lateFrames = 0;
constexpr int kFramesJudged = 90;
// Every refresh is tried again after this long, three times longer after
// each try that was late again
constexpr qint64 kFirstRetryMicros = 10'000'000;
constexpr qint64 kLastRetryMicros = 300'000'000;
std::atomic<qint64> s_retryMicros{kFirstRetryMicros};

const QElapsedTimer& epoch() {
    static const QElapsedTimer s_epoch = []() {
        QElapsedTimer timer;
        timer.start();
        return timer;
    }();
    return s_epoch;
}

qint64 nowMicros() {
    return epoch().nsecsElapsed() / 1000;
}

} // namespace

void setDisplay(std::chrono::microseconds refreshPeriod, int frameRate) {
    const qint64 period = std::max<qint64>(1000, refreshPeriod.count());
    // From 60 fps on (all the choices of the settings), every refresh: a
    // waveform moved on every second refresh of a 120 Hz display, while
    // the rest of the window was drawn on every one, jumped by two and
    // three refreshes by turns. Drawn by the GPU, it costs little.
    const double wanted = frameRate >= 60
            ? (s_everySecondRefresh.load() ? 2.0 * static_cast<double>(period) : 0.0)
            : 1'000'000.0 / std::max(1, frameRate);
    const qint64 periods = std::max<qint64>(1,
            static_cast<qint64>(std::ceil(wanted / static_cast<double>(period) - 0.01)));
    s_periodMicros = period;
    s_intervalMicros = periods * period;
}

std::chrono::microseconds frameInterval() {
    return std::chrono::microseconds(s_intervalMicros.load());
}

std::chrono::microseconds refreshPeriod() {
    return std::chrono::microseconds(s_periodMicros.load());
}

void frameShown() {
    const qint64 now = nowMicros();
    const qint64 period = s_periodMicros.load();
    const qint64 interval = now - s_lastFrameMicros.exchange(now);
    if (interval < period / 2) {
        // Told by the other waveform already
        return;
    }
    if (s_everySecondRefresh.load()) {
        if (now - s_everySecondRefreshSinceMicros.load() > s_retryMicros.load()) {
            s_everySecondRefresh = false;
            s_frames = 0;
            s_lateFrames = 0;
        }
        return;
    }
    // Pauses (nothing moving) are not late frames
    if (interval > 4 * period) {
        return;
    }
    ++s_frames;
    if (interval > period * 3 / 2) {
        ++s_lateFrames;
    }
    if (s_frames >= kFramesJudged) {
        if (s_lateFrames * 5 > s_frames) {
            const bool retried = s_everySecondRefreshSinceMicros.load() > 0;
            s_everySecondRefresh = true;
            s_everySecondRefreshSinceMicros = now;
            if (retried) {
                s_retryMicros = std::min(kLastRetryMicros, s_retryMicros.load() * 3);
            }
            qInfo() << "Waveforms on every second refresh:" << s_lateFrames << "of"
                    << s_frames << "frames were late; trying every refresh again in"
                    << s_retryMicros.load() / 1'000'000 << "s";
        }
        s_frames = 0;
        s_lateFrames = 0;
    }
}

void tickShown() {
    s_lastTickMicros = nowMicros();
}

int millisToNextTick(std::chrono::microseconds minDelay) {
    const qint64 interval = s_intervalMicros.load();
    const qint64 halfPeriod = s_periodMicros.load() / 2;
    const qint64 lastTick = s_lastTickMicros.load();
    const qint64 lastFrame = lastTick > 0 ? lastTick : s_lastFrameMicros.load();
    const qint64 now = nowMicros();
    const qint64 earliest = now + std::max<qint64>(0, minDelay.count());
    // Ticks are half a period before the refreshes a whole number of
    // intervals after the last frame shown: an update there is drawn on
    // that refresh. Not the frame shown itself (k >= 1).
    const qint64 k = std::max<qint64>(1,
            (earliest - lastFrame + halfPeriod + interval - 1) / interval);
    const qint64 tick = lastFrame + k * interval - halfPeriod;
    const qint64 delayMicros = tick - now;
    return static_cast<int>(std::max<qint64>(1, (delayMicros + 999) / 1000));
}

} // namespace frameclock
} // namespace qml
} // namespace mixxx
