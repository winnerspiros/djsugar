#pragma once

#include <chrono>

namespace mixxx {
namespace qml {

/// A common grid of frame times for periodic redraws (scrolling waveforms,
/// controls with updateInterval, see QmlControlProxy). Updates on the same
/// tick are drawn in one frame. The grid follows the display refresh:
/// its interval is a whole number of refresh periods, ticks half a period
/// before a refresh. A rate not dividing the refresh rate causes judder.
namespace frameclock {

/// Sets the display refresh period and the frame rate. From 60 fps on the
/// grid interval is one period, or two while the device can't keep up
/// (frameShown()); below, the whole number of periods closest to the rate.
void setDisplay(std::chrono::microseconds refreshPeriod, int frameRate);
/// The interval of the grid
std::chrono::microseconds frameInterval();
/// The refresh period of the display
std::chrono::microseconds refreshPeriod();
/// A frame was shown on the render thread. If more than a fifth of frames
/// take longer than a refresh, the grid drops to every second refresh.
/// Retries every refresh after a while (3x longer each failure).
void frameShown();

/// A frame with a waveform was shown on the render thread: ticks counted
/// from it so waveforms move steadily whatever else draws frames.
void tickShown();

/// Milliseconds until the first grid tick >= minDelay after the last frame.
int millisToNextTick(std::chrono::microseconds minDelay = std::chrono::microseconds(0));

} // namespace frameclock
} // namespace qml
} // namespace mixxx
