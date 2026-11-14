#pragma once

#include <vector>

/// Auto DJ 2.0 plus Video Mixing: singing detection.
/// Vocal level smoothed; moments above the quiet background count as singing.
/// Pure maths, unit tested (vocalmap_test.cpp).
namespace vocalmap {

struct Section {
    double startSec = 0.0;
    double endSec = 0.0;
};

struct Settings {
    double smoothSec = 0.5;     ///< averaging window
    double joinGapSec = 1.5;    ///< shorter rests (breaths) stay one section
    double minSectionSec = 1.0; ///< shorter bits are not singing
    /// Singing threshold: floor + level x (peak - floor) of vocal level.
    double level = 0.3;     ///< threshold relative to vocal range
    double minPeak = 0.06;  ///< vocal never louder: no singing
    double leakRatio = 0.2; ///< vocal must reach this of the band's level
};

/// @param vocal level of the vocal part (0..1), `rate` values per second
/// @param rest level of the other parts together (same length) or empty
/// @return the singing sections in seconds, in order
std::vector<Section> find(const std::vector<float>& vocal,
        const std::vector<float>& rest,
        double rate,
        const Settings& settings = Settings());

/// Does anyone sing somewhere between `fromSec` and `toSec`?
bool singsBetween(const std::vector<Section>& sections, double fromSec, double toSec);

/// The first moment when nobody sings (end of a line), or -1.
double quietMoment(const std::vector<Section>& sections, double fromSec, double toSec);

/// When singing starts after `fromSec` (-1 if not).
double nextSinging(const std::vector<Section>& sections, double fromSec);

} // namespace vocalmap
