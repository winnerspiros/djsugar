#pragma once

#include <QElapsedTimer>
#include <QList>
#include <QStringList>
#include <QVector>
#include <optional>
#include <utility>

#include "library/autodj/smart/mixscorer.h"

/// A clash in the sorted order and the tracks that could bridge it.
struct BridgeGap {
    int k = 0;        ///< bridge goes right after the k-th track (from 1)
    QString fromText; ///< the two tracks either side, for the DJ
    QString toText;
    /// Best first; empty = nothing bridges. Each option is one or two tracks.
    QList<QList<TrackId>> options;
    QStringList optionTexts;
};

/// The outcome of SmartSequencer::solve().
struct SequenceResult {
    QList<TrackId> order;
    double totalCost = 0.0;
    /// Transitions the solver could not make smooth.
    int clashCount = 0;
    /// One line per clash, so the DJ knows where a bridge is needed.
    QStringList warnings;
    /// The running order with clash notes and bridge suggestions.
    QStringList orderLines;
    /// Best bridge per clash, as (k, tracks): play right after the k-th track.
    QList<std::pair<int, QList<TrackId>>> bestBridges;
    /// Every clash with ALL bridge options (up to kMaxBridgeOptions).
    /// Options only exclude tracks already queued.
    QList<BridgeGap> gaps;
    static constexpr int kMaxBridgeOptions = 6;
};

/// Sorts tracks into the lowest-cost mixing order (open TSP).
///
/// Beside the pair cost, the first and last track carry a "set shape" cost
/// (MixScorer::startCost/endCost), e.g. so an energy-building set starts
/// calm and ends at the peak.
/// - Up to kExactLimit tracks: exact answer (Held-Karp DP).
/// - Larger sets: multi-start greedy, then 2-opt / Or-opt polishing and
///   random kick-and-re-polish until the time budget runs out.
/// Pure logic: no Track objects, no database, safe on a worker thread.
class SmartSequencer {
  public:
    /// Largest set solved exactly. 2^16 * 16 * 16 = ~17M steps, ~12 MB.
    static constexpr int kExactLimit = 16;

    explicit SmartSequencer(const MixScorer& scorer);

    /// @param startId optional track that must stay first (e.g. playing now)
    /// @param timeBudgetMs stop improving after this long
    /// @param bridgeCandidates tracks to suggest as bridges for remaining clashes
    SequenceResult solve(const QVector<TrackFeatures>& tracks,
            std::optional<TrackId> startId = std::nullopt,
            int timeBudgetMs = 2000,
            const QVector<TrackFeatures>& bridgeCandidates = {}) const;

  private:
    struct Costs {
        QVector<QVector<double>> pair; ///< pair[a][b]: play b right after a
        QVector<double> start;         ///< extra cost if a track opens the set
        QVector<double> end;           ///< extra cost if a track closes the set
    };

    Costs buildCosts(const QVector<TrackFeatures>& tracks) const;
    static QVector<int> exactPath(const Costs& c, int fixedStart);
    static QVector<int> heuristicPath(const Costs& c, int fixedStart, int timeBudgetMs);
    static void localSearch(const Costs& c,
            QVector<int>* pPath,
            bool lockFirst,
            const QElapsedTimer& timer,
            int timeBudgetMs);
    static QVector<int> greedyPath(const Costs& c, int start);
    static bool improveTwoOpt(const Costs& c, QVector<int>* pPath, bool lockFirst);
    static bool improveOrOpt(const Costs& c, QVector<int>* pPath, bool lockFirst);
    static double pathCost(const Costs& c, const QVector<int>& path);

    MixScorer m_scorer;
};
