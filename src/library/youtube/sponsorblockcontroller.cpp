#include "library/youtube/sponsorblockcontroller.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QRegularExpression>
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

// Titles of music videos, which may carry a non-music lead-in, as opposed
// to audio uploads ("Official Audio", "Topic" channels) that start with the song.
bool isMusicVideoTitle(const QString& title) {
    static const QRegularExpression kMusicVideo(
            QStringLiteral("official\\s+(music\\s+)?video|music\\s+video|"
                           "video\\s*clip|video\\s+oficial|clip\\s+officiel|"
                           "\\bm/?v\\b"),
            QRegularExpression::CaseInsensitiveOption);
    return kMusicVideo.match(title).hasMatch();
}

// Raw YouTube title written by YouTubeFeature at download time.
QString rawVideoTitle(const QString& cacheDir, const QString& videoId) {
    QFile file(QDir(cacheDir).filePath(videoId + QStringLiteral(".meta.json")));
    if (!file.open(QIODevice::ReadOnly)) {
        return {};
    }
    return QJsonDocument::fromJson(file.readAll())
            .object()
            .value(QStringLiteral("title"))
            .toString();
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

SponsorBlockController::SponsorBlockController(YouTubeService* pService,
        const QString& cacheDir,
        bool autoSkipEnabled,
        QObject* parent)
        : QObject(parent),
          m_cacheDir(cacheDir),
          m_autoSkipEnabled(autoSkipEnabled),
          m_pService(pService) {
    connect(m_pService.data(),
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
    it->segmentsKnown = false;
    it->sponsorBlockCovered = false;
    it->isMusicVideo = false;
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
    it->isMusicVideo = isMusicVideoTitle(rawVideoTitle(m_cacheDir, videoId));

    if (!tryLoadSegmentsFromSidecar(group, videoId) && m_pService) {
        // Fall back to the network. The reply is correlated by videoId, and
        // multiple decks can be waiting on the same id concurrently.
        m_pService->fetchSponsorSegments(videoId);
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
    if (doc.isObject() && doc.object().value(QStringLiteral("cut")).toInt() > 0) {
        // Segments were cut from the file at download: nothing to skip,
        // and no analysis skip either (SponsorBlock is preferred)
        auto it = m_decks.find(group);
        if (it != m_decks.end()) {
            it->segmentsKnown = true;
            it->sponsorBlockCovered = true;
        }
        return true;
    }
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
        it->segmentsKnown = true;
        it->sponsorBlockCovered = !segments.isEmpty();
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
            it->segmentsKnown = true;
            it->sponsorBlockCovered = !segments.isEmpty();
            kLogger.info() << "Applied" << segments.size()
                           << "SponsorBlock segments to" << it.key();
        }
    }
}

double SponsorBlockController::musicVideoLeadInEndSec(
        const DeckState& state, const TrackPointer& pTrack) const {
    constexpr double kMinLeadInSec = 8.0;
    constexpr double kMaxLeadInSec = 120.0;
    if (!pTrack || !state.isMusicVideo || !state.segmentsKnown || !state.segments.isEmpty()) {
        return -1.0;
    }
    const CuePointer pIntro = pTrack->findCueByType(mixxx::CueType::Intro);
    const auto sampleRate = pTrack->getSampleRate();
    if (!pIntro || !sampleRate.isValid()) {
        return -1.0;
    }
    const auto introEnd = pIntro->getEndPosition();
    if (!introEnd.isValid()) {
        return -1.0;
    }
    const double introEndSec = introEnd.value() / sampleRate.value();
    if (introEndSec < kMinLeadInSec || introEndSec > kMaxLeadInSec) {
        return -1.0;
    }
    return introEndSec;
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
    // A video SponsorBlock knows is skipped by its segments only. Until its
    // reply is in (normally well under a second), the analysis skip waits
    // for the first seconds, so it doesn't jump ahead of SponsorBlock.
    constexpr double kWaitForSponsorBlockSec = 5.0;
    const bool analysisSkipAllowed = !it->sponsorBlockCovered &&
            (it->segmentsKnown || currentSec >= kWaitForSponsorBlockSec);
    if (skipTo < 0.0 && analysisSkipAllowed) {
        // No SponsorBlock data covers this spot (the database has nothing for
        // most music videos). Fall back to the analyzer's audible range:
        // skip the silence/talk-free lead-in before the music starts and the
        // silent tail after it ends. The range only exists once the analyzer
        // has run, which can be a few seconds after the track starts.
        const TrackPointer pTrack = it->pTrack.lock();
        const CuePointer pSound = pTrack
                ? pTrack->findCueByType(mixxx::CueType::N60dBSound)
                : CuePointer();
        const auto sampleRate = pTrack ? pTrack->getSampleRate() : mixxx::audio::SampleRate();
        if (pSound && sampleRate.isValid() && pSound->getPosition().isValid()) {
            constexpr double kMinSilenceSec = 1.0;
            const double soundStartSec =
                    pSound->getPosition().value() / sampleRate.value();
            if (!it->leadSilenceSkipped) {
                double leadInEndSec = soundStartSec >= kMinSilenceSec ? soundStartSec : -1.0;
                // Music videos often open with a skit, talking or ambience
                // that SponsorBlock has no entry for. The energy analyzer's
                // Intro End marker is where the bass and the level of the
                // song come in; before it, there is no music to mix.
                const double introEndSec = musicVideoLeadInEndSec(*it, pTrack);
                if (introEndSec > leadInEndSec) {
                    leadInEndSec = introEndSec;
                }
                if (leadInEndSec > 0.0 && currentSec < leadInEndSec - 0.25) {
                    skipTo = leadInEndSec;
                }
                // Decided for this load once skipped, or once the music plays
                // past the lead-in (the user may seek back into it on
                // purpose). The analysis and SponsorBlock reply can land a
                // few seconds after the start, so an unknown lead-in stays open.
                constexpr double kLeadInDecidedSec = 120.0;
                if (skipTo >= 0.0 || (leadInEndSec > 0.0 && currentSec >= leadInEndSec) ||
                        currentSec > kLeadInDecidedSec) {
                    it->leadSilenceSkipped = true;
                }
            }
            const auto endPos = pSound->getEndPosition();
            if (skipTo < 0.0 && endPos.isValid()) {
                const double soundEndSec = endPos.value() / sampleRate.value();
                if (it->durationSec - soundEndSec >= kMinSilenceSec &&
                        currentSec > soundEndSec + 0.25 &&
                        currentSec < it->durationSec - 0.5) {
                    skipTo = it->durationSec;
                }
            }
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
