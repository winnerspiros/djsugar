#include "library/youtube/sponsorblockcontroller.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <algorithm>

#include "control/controlobject.h"
#include "mixer/playerinfo.h"
#include "track/track.h"
#include "util/logger.h"

namespace mixxx {

namespace {
const Logger kLogger("SponsorBlockController");

constexpr int kMaxDecks = 8;

QString sidecarPathFor(const QString& cacheDir, const QString& videoId) {
    return QDir(cacheDir).filePath(videoId + QStringLiteral(".sponsorblock.json"));
}

QString deriveVideoIdFromTrackPath(
        const QString& cacheDir, const QString& trackLocation) {
    if (trackLocation.isEmpty() || cacheDir.isEmpty()) {
        return {};
    }
    const QFileInfo fi(trackLocation);
    // Cached files are named <videoId>.<ext> directly inside cacheDir.
    if (fi.absoluteDir().absolutePath() != QDir(cacheDir).absolutePath()) {
        return {};
    }
    // YouTube IDs are 11 chars; we are deliberately permissive and just trust
    // the cache directory to only contain our own files.
    return fi.completeBaseName();
}
} // namespace

SponsorBlockController::SponsorBlockController(
        const QString& cacheDir, bool autoSkipEnabled, QObject* parent)
        : QObject(parent),
          m_cacheDir(cacheDir),
          m_autoSkipEnabled(autoSkipEnabled),
          m_service(this) {
    connect(&m_service,
            &YouTubeService::sponsorSegmentsFetched,
            this,
            &SponsorBlockController::onSponsorSegmentsFetched);
    connect(&PlayerInfo::instance(),
            &PlayerInfo::trackChanged,
            this,
            &SponsorBlockController::onTrackChanged);
}

SponsorBlockController::~SponsorBlockController() {
    for (auto& state : m_decks) {
        delete state.pPlayPosition;
    }
}

void SponsorBlockController::ensureDeckTracked(const QString& group) {
    if (m_decks.contains(group)) {
        return;
    }
    DeckState state;
    state.pPlayPosition = new ControlProxy(group, QStringLiteral("playposition"), this);
    state.pPlayPosition->connectValueChanged(this,
            [this, group](double v) { onPlayPositionChanged(group, v); });
    m_decks.insert(group, state);
}

void SponsorBlockController::clearDeck(const QString& group) {
    auto it = m_decks.find(group);
    if (it == m_decks.end()) {
        return;
    }
    it->videoId.clear();
    it->segments.clear();
    it->durationSec = 0.0;
    it->pTrack.reset();
    it->leadSilenceSkipped = false;
}

void SponsorBlockController::onTrackChanged(
        const QString& group,
        TrackPointer pNewTrack,
        TrackPointer /*pOldTrack*/) {
    // Only deck channels (`[Channel1]`, `[Channel2]`, ...) are interesting; we
    // don't bother with sampler/preview groups.
    if (!group.startsWith(QStringLiteral("[Channel"))) {
        return;
    }
    // Cap to a sensible number of decks regardless of what Mixxx reports.
    if (m_decks.size() >= kMaxDecks && !m_decks.contains(group)) {
        return;
    }
    ensureDeckTracked(group);
    clearDeck(group);
    if (!pNewTrack) {
        return;
    }
    const QString location = pNewTrack->getLocation();
    const QString videoId = deriveVideoIdFromTrackPath(m_cacheDir, location);
    if (videoId.isEmpty()) {
        return; // not one of our cached YouTube tracks
    }
    auto it = m_decks.find(group);
    it->videoId = videoId;
    it->durationSec = pNewTrack->getDuration();
    it->pTrack = pNewTrack;

    if (!tryLoadSegmentsFromSidecar(group, videoId)) {
        // Fall back to the network. The reply is correlated by videoId, and
        // multiple decks can be waiting on the same id concurrently.
        m_service.fetchSponsorSegments(videoId);
    }
}

bool SponsorBlockController::tryLoadSegmentsFromSidecar(
        const QString& group, const QString& videoId) {
    QFile f(sidecarPathFor(m_cacheDir, videoId));
    if (!f.exists() || !f.open(QIODevice::ReadOnly)) {
        return false;
    }
    const QJsonDocument doc = QJsonDocument::fromJson(f.readAll());
    f.close();
    if (!doc.isArray()) {
        return false;
    }
    QList<SponsorSegment> segments;
    const QJsonArray arr = doc.array();
    for (const auto& v : arr) {
        const QJsonObject obj = v.toObject();
        const QJsonArray seg = obj.value(QStringLiteral("segment")).toArray();
        if (seg.size() < 2) {
            continue;
        }
        segments.append({seg[0].toDouble(),
                seg[1].toDouble(),
                obj.value(QStringLiteral("category")).toString()});
    }
    auto it = m_decks.find(group);
    if (it != m_decks.end()) {
        it->segments = segments;
    }
    if (!segments.isEmpty()) {
        kLogger.info() << "Loaded" << segments.size()
                       << "SponsorBlock segments for" << videoId
                       << "from sidecar";
    }
    return true;
}

void SponsorBlockController::onSponsorSegmentsFetched(
        const QString& videoId, const QList<SponsorSegment>& segments) {
    // Persist a sidecar so subsequent loads do not hit the network.
    if (!segments.isEmpty()) {
        QFile f(sidecarPathFor(m_cacheDir, videoId));
        if (f.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
            QJsonArray arr;
            for (const auto& s : segments) {
                QJsonObject obj;
                QJsonArray seg;
                seg.append(s.start);
                seg.append(s.end);
                obj.insert(QStringLiteral("segment"), seg);
                obj.insert(QStringLiteral("category"), s.category);
                arr.append(obj);
            }
            f.write(QJsonDocument(arr).toJson(QJsonDocument::Compact));
            f.close();
        }
    }
    // Apply to any deck currently waiting on this id.
    for (auto it = m_decks.begin(); it != m_decks.end(); ++it) {
        if (it->videoId == videoId) {
            it->segments = segments;
            kLogger.info() << "Applied" << segments.size()
                           << "SponsorBlock segments to" << it.key();
        }
    }
}

void SponsorBlockController::onPlayPositionChanged(
        const QString& group, double pos) {
    if (!m_autoSkipEnabled) {
        return;
    }
    auto it = m_decks.find(group);
    if (it == m_decks.end() || it->videoId.isEmpty() || it->durationSec <= 0.0) {
        return;
    }
    // Only a playing deck skips: seeking around a paused deck (cueing, setting
    // loops) must not be fought.
    if (ControlObject::get(ConfigKey(group, QStringLiteral("play"))) <= 0.0) {
        return;
    }
    const double currentSec = pos * it->durationSec;
    // Skip whenever the playhead is INSIDE a segment, not only when it crosses
    // the start: the segments arrive over the network a moment after the load
    // and the intro (a segment from 0:00) is already playing by then, and a
    // seek or a coarse position update can jump straight past a start edge.
    double skipTo = -1.0;
    for (const auto& seg : std::as_const(it->segments)) {
        if (seg.end > seg.start && currentSec >= seg.start && currentSec < seg.end) {
            skipTo = std::max(skipTo, seg.end);
        }
    }
    if (skipTo < 0.0 && !it->leadSilenceSkipped) {
        // No SponsorBlock data covers this spot (the database has nothing for
        // most music videos). Fall back to the analyzer's audible range and
        // skip silence or near-silence before the music starts.
        const TrackPointer pTrack = it->pTrack.lock();
        const CuePointer pSound = pTrack
                ? pTrack->findCueByType(mixxx::CueType::N60dBSound)
                : CuePointer();
        const auto sampleRate = pTrack ? pTrack->getSampleRate() : mixxx::audio::SampleRate();
        if (pSound && sampleRate.isValid() && pSound->getPosition().isValid()) {
            const double soundStartSec =
                    pSound->getPosition().value() / sampleRate.value();
            constexpr double kMinLeadSilenceSec = 1.0;
            if (soundStartSec >= kMinLeadSilenceSec &&
                    currentSec < soundStartSec - 0.25) {
                skipTo = soundStartSec;
            }
        }
        if (currentSec > 0.0) {
            // Decided for this load: either skipped now or the music has
            // already started.
            it->leadSilenceSkipped = true;
        }
    }
    if (skipTo < 0.0) {
        return;
    }
    const double newPos = qBound(0.0, skipTo / it->durationSec, 1.0);
    kLogger.info() << "[" << group << "] Skipping non-music segment"
                   << currentSec << "→" << skipTo;
    // set() is async-safe; the engine will pick it up on the next pass.
    it->pPlayPosition->set(newPos);
}

} // namespace mixxx

#include "moc_sponsorblockcontroller.cpp"
