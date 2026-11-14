#pragma once

#include <QHash>
#include <QList>
#include <QSqlDatabase>
#include <optional>

#include "analyzer/energycalculator.h"
#include "track/trackid.h"

/// Auto DJ 2.0: saves energy scores, auto-markers and grid-check results.
/// Uses separate tables (autodj_energy, autodj_energy_manual,
/// autodj_auto_markers, autodj_grid_check) not in Mixxx's versioned schema.
class EnergyStore {
  public:
    static constexpr int kMinRating = 1;
    static constexpr int kMaxRating = 10;

    struct Value {
        double energy = 0.0; ///< 1..10
        bool manual = false; ///< true = rated by the DJ
    };

    /// Creates the table if needed. Safe to call often.
    static bool ensureTable(const QSqlDatabase& db);

    /// The formula version stored for this track, if any.
    static std::optional<int> storedVersion(const QSqlDatabase& db, TrackId trackId);

    static bool save(const QSqlDatabase& db,
            TrackId trackId,
            const EnergyCalculator::Result& result,
            int version);

    /// Where the track's body starts and ends (seconds at its own speed),
    /// from analysis v2+. nullopt if not analysed yet.
    struct Body {
        double startSec = 0.0;
        double endSec = 0.0;
    };
    static std::optional<Body> loadBody(const QSqlDatabase& db, TrackId trackId);

    /// Marker positions (seconds) the analyzer set itself; -1 = none.
    struct AutoMarkers {
        double introEndSec = -1.0;
        double outroStartSec = -1.0;
    };
    static AutoMarkers loadAutoMarkers(const QSqlDatabase& db, TrackId trackId);
    static bool saveAutoMarkers(
            const QSqlDatabase& db, TrackId trackId, const AutoMarkers& markers);

    /// Beat grid check result. bpm 0 = the track had no grid.
    struct GridCheck {
        double bpm = 0.0;
        double firstBeatSec = 0.0;
        double driftBeats = -1.0; ///< -1 = could not tell
        int version = 0;          ///< EnergyCalculator::kGridCheckVersion (or ...MapVersion)
        /// True if this check was made on that grid (the DJ has not
        /// changed it since), with the current or the previous version of
        /// the check (a previous result counts until the track is analysed
        /// again). `beatMap`: the grid is a beat map now.
        bool isFor(double gridBpm, double gridFirstBeatSec, bool beatMap) const;
        /// Made with the current version of the check.
        bool isCurrent(bool beatMap) const;
    };
    static std::optional<GridCheck> loadGridCheck(const QSqlDatabase& db, TrackId trackId);
    static bool saveGridCheck(const QSqlDatabase& db, TrackId trackId, const GridCheck& check);

    /// Sets the DJ's rating (1..10) for the tracks. rating 0 clears it, so
    /// the measured value is used again.
    static bool setManualRating(
            const QSqlDatabase& db, const QList<TrackId>& trackIds, int rating);

    /// Energy for the given tracks: the rating if set, else the measured
    /// value. Tracks with neither are left out.
    static QHash<TrackId, Value> loadEnergies(
            const QSqlDatabase& db, const QList<TrackId>& trackIds);
};
