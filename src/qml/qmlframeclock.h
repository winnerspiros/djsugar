#pragma once

#include <chrono>

namespace mixxx {
namespace qml {

/// A common grid of frame times for everything that redraws periodically
/// (the scrolling waveforms, and the controls with an updateInterval, e.g.
/// playposition, see QmlControlProxy). Each update of an item redraws the
/// whole window: when every source picks its own moments, the window is
/// redrawn as often as the display allows (e.g. 90 Hz) whatever the frame
/// rate of the waveforms is. Updates on the same tick of the grid are drawn
/// in one frame.
///
/// The grid follows the refresh of the display: its interval is a whole
/// number of refresh periods, and its ticks are half a period before a
/// refresh, counted from the last frame shown. A frame rate that doesn't
/// divide the refresh rate (60 fps on a 90 Hz display) shows frames for one
/// and two periods by turns, and the waveforms judder.
namespace frameclock {

/// Sets the refresh period of the display and the frame rate wanted. From 60
/// fps on, the interval of the grid is one period, or two while the device
/// can't draw a frame on every refresh (see frameShown()); below, the whole
/// number of periods closest to the frame rate, not faster.
void setDisplay(std::chrono::microseconds refreshPeriod, int frameRate);
/// The interval of the grid
std::chrono::microseconds frameInterval();
/// The refresh period of the display
std::chrono::microseconds refreshPeriod();
/// Tells that a frame was shown now, on the render thread. While frames
/// should come on every refresh, if more than a fifth of them take longer,
/// the grid goes to every second refresh: steady frames look smoother than
/// frames of one and two refreshes by turns. It tries every refresh again
/// after a while, three times longer after each try that failed.
void frameShown();

/// Tells that a frame with a waveform drawn in it was shown now, on the
/// render thread: the ticks are counted from it, so that the waveforms move
/// at a steady pace whatever else draws frames in between
void tickShown();

/// Milliseconds (at least 1) until the first tick of the grid that is at
/// least minDelay away and after the last frame shown
int millisToNextTick(std::chrono::microseconds minDelay = std::chrono::microseconds(0));

} // namespace frameclock
} // namespace qml
} // namespace mixxx
