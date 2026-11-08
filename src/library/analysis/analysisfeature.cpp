#include "library/analysis/analysisfeature.h"

#include <QList>
#include <QSet>
#include <QtDebug>

#include "analyzer/analyzerscheduledtrack.h"
#include "controllers/keyboard/keyboardeventfilter.h"
#include "library/analysis/dlganalysis.h"
#include "library/library.h"
#include "library/trackcollectionmanager.h"
#include "mixer/playerinfo.h"
#include "moc_analysisfeature.cpp"
#include "sources/soundsourceproxy.h"
#include "track/track.h"
#include "util/dnd.h"
#include "util/logger.h"
#include "util/math.h"
#include "widget/wlibrary.h"

namespace {

const mixxx::Logger kLogger("AnalysisFeature");

const QString kViewName = QStringLiteral("Analysis");

#ifdef Q_OS_ANDROID
// Phones have few big cores: a full set of batch workers starves the audio
// and UI threads (audible as crackle). Keep cores free for playback.
const int kNumberOfAnalyzerThreads = math_max(1, math_min(3, QThread::idealThreadCount() / 2));
#else
// Utilize all available cores for batch analysis of tracks
const int kNumberOfAnalyzerThreads = math_max(1, QThread::idealThreadCount());
#endif

inline int numberOfAnalyzerThreads() {
    return kNumberOfAnalyzerThreads;
}

inline AnalyzerModeFlags getAnalyzerModeFlags(
        const UserSettingsPointer& pConfig) {
    // Always enable at least BPM detection for batch analysis, even if disabled
    // in the config for ad-hoc analysis of tracks.
    // NOTE(uklotzde, 2018-12-26): The previous comment just states the status-quo
    // of the existing code. We should rethink the configuration of analyzers when
    // refactoring/redesigning the analyzer framework.
    int modeFlags = AnalyzerModeFlags::WithBeats | AnalyzerModeFlags::LowPriority;
    if (pConfig->getValue<bool>(ConfigKey("[Library]", "EnableWaveformGenerationWithAnalysis"), true)) {
        modeFlags |= AnalyzerModeFlags::WithWaveform;
    }
    return static_cast<AnalyzerModeFlags>(modeFlags);
}

} // anonymous namespace

AnalysisFeature::AnalysisFeature(
        Library* pLibrary,
        UserSettingsPointer pConfig)
        : LibraryFeature(pLibrary, pConfig, QStringLiteral("prepare")),
          m_baseTitle(tr("Analyze")),
          m_pTrackAnalysisScheduler(TrackAnalysisScheduler::NullPointer()),
          m_pSidebarModel(make_parented<TreeItemModel>(this)),
          m_pAnalysisView(nullptr),
          m_title(m_baseTitle) {
}

AnalysisFeature::~AnalysisFeature() {
    // We need to delete m_pTrackAnalysisScheduler here immediately synchronously,
    // waiting for pending threads to have finished, to not kill them during exit
    delete m_pTrackAnalysisScheduler.release();
}

void AnalysisFeature::resetTitle() {
    m_title = m_baseTitle;
    emit featureIsLoading(this, false);
}

void AnalysisFeature::setTitleProgress(int currentTrackNumber, int totalTracksCount) {
    m_title = QString("%1 (%2 / %3)")
                      .arg(m_baseTitle,
                              QString::number(currentTrackNumber),
                              QString::number(totalTracksCount));
    emit featureIsLoading(this, false);
}

void AnalysisFeature::bindLibraryWidget(WLibrary* libraryWidget,
        KeyboardEventFilter* keyboard) {
    m_pAnalysisView = new DlgAnalysis(libraryWidget,
            m_pConfig,
            m_pLibrary);
    connect(m_pAnalysisView,
            &DlgAnalysis::loadTrack,
            this,
            &AnalysisFeature::loadTrack);
    connect(m_pAnalysisView,
            &DlgAnalysis::loadTrackToPlayer,
            this,
            [=, this](TrackPointer track, const QString& group) {
                emit loadTrackToPlayer(track, group,
#ifdef __STEM__
                        mixxx::StemChannelSelection(),
#endif
                        false);
            });
    connect(m_pAnalysisView,
            &DlgAnalysis::analyzeTracks,
            this,
            &AnalysisFeature::analyzeTracks);
    connect(m_pAnalysisView,
            &DlgAnalysis::stopAnalysis,
            this,
            &AnalysisFeature::stopAnalysis);

    connect(m_pAnalysisView,
            &DlgAnalysis::trackSelected,
            this,
            &AnalysisFeature::trackSelected);

    connect(this,
            &AnalysisFeature::analysisActive,
            m_pAnalysisView,
            &DlgAnalysis::slotAnalysisActive);

    m_pAnalysisView->installEventFilter(keyboard);

    // Let the DlgAnalysis know whether or not analysis is active.
    emit analysisActive(static_cast<bool>(m_pTrackAnalysisScheduler));

    libraryWidget->registerView(kViewName, m_pAnalysisView);
}

TreeItemModel* AnalysisFeature::sidebarModel() const {
    return m_pSidebarModel;
}

void AnalysisFeature::refreshLibraryModels() {
    if (m_pAnalysisView) {
        m_pAnalysisView->onShow();
    }
}

void AnalysisFeature::activate() {
    // qDebug() << "AnalysisFeature::activate()";
    emit switchToView(kViewName);
    if (m_pAnalysisView) {
        emit restoreSearch(m_pAnalysisView->currentSearch());
    }
    emit enableCoverArtDisplay(true);
}

void AnalysisFeature::analyzeTracks(const QList<AnalyzerScheduledTrack>& tracks) {
    if (!m_pTrackAnalysisScheduler) {
        const int numAnalyzerThreads = numberOfAnalyzerThreads();
        kLogger.info()
                << "Starting analysis using"
                << numAnalyzerThreads
                << "analyzer threads";
        m_pTrackAnalysisScheduler = m_pLibrary->createTrackAnalysisScheduler(
                numAnalyzerThreads,
                getAnalyzerModeFlags(m_pConfig));

        // The QML UI has no analysis view (it is a widget skin dialog)
        if (m_pAnalysisView) {
            connect(m_pTrackAnalysisScheduler.get(),
                    &TrackAnalysisScheduler::progress,
                    m_pAnalysisView,
                    &DlgAnalysis::onTrackAnalysisSchedulerProgress);
            connect(m_pTrackAnalysisScheduler.get(),
                    &TrackAnalysisScheduler::finished,
                    m_pAnalysisView,
                    &DlgAnalysis::onTrackAnalysisSchedulerFinished);
        }
        connect(m_pTrackAnalysisScheduler.get(),
                &TrackAnalysisScheduler::progress,
                this,
                &AnalysisFeature::onTrackAnalysisSchedulerProgress);
        connect(m_pTrackAnalysisScheduler.get(),
                &TrackAnalysisScheduler::finished,
                this,
                &AnalysisFeature::onTrackAnalysisSchedulerFinished);
        // Forward the signal to be picked up by Library.
        // Used by WOverview to render the re-analysis progress of loaded tracks.
        // This is the equivalent to PlayerManager's progress signal fired for
        // analysis triggered by loading a track.
        connect(m_pTrackAnalysisScheduler.get(),
                &TrackAnalysisScheduler::trackProgress,
                this,
                &AnalysisFeature::trackProgress);

        emit analysisActive(true);
    }

    // A track on a deck is analyzed by the deck's own analyzer when it is
    // loaded. Scheduling it here as well ran the same 5 s analysis twice at
    // once on the phone (seen in the logs for every YouTube load). Explicit
    // re-analysis requests (with options) still go through.
    QSet<TrackId> loadedTrackIds;
    const auto loadedTracks = PlayerInfo::instance().getLoadedTracks();
    for (const TrackPointer& pTrack : loadedTracks) {
        if (pTrack) {
            loadedTrackIds.insert(pTrack->getId());
        }
    }
    QList<AnalyzerScheduledTrack> toSchedule;
    toSchedule.reserve(tracks.size());
    for (const auto& track : tracks) {
        const auto& options = track.getOptions();
        if (loadedTrackIds.contains(track.getTrackId()) && !options.useFixedTempo &&
                !options.redoBeats) {
            continue;
        }
        toSchedule.append(track);
    }
    if (toSchedule.isEmpty()) {
        return;
    }
    if (m_pTrackAnalysisScheduler->scheduleTracks(toSchedule) > 0) {
        resumeAnalysis();
    }
}

void AnalysisFeature::suspendAnalysis() {
    if (!m_pTrackAnalysisScheduler) {
        return; // inactive
    }
    // Called on every progress step of a deck's analysis: logged and
    // forwarded once per suspension (it filled the log with ~40 lines a track)
    if (m_analysisSuspended) {
        return;
    }
    m_analysisSuspended = true;
    kLogger.info() << "Suspending analysis";
    m_pTrackAnalysisScheduler->suspend();
}

void AnalysisFeature::resumeAnalysis() {
    if (!m_pTrackAnalysisScheduler) {
        return; // inactive
    }
    m_analysisSuspended = false;
    kLogger.info() << "Resuming analysis";
    m_pTrackAnalysisScheduler->resume();
}

void AnalysisFeature::stopAnalysis() {
    if (!m_pTrackAnalysisScheduler) {
        return; // inactive
    }
    kLogger.info() << "Stopping analysis";
    m_pTrackAnalysisScheduler->stop();
}

void AnalysisFeature::onTrackAnalysisSchedulerProgress(
        AnalyzerProgress /*currentTrackProgress*/,
        int currentTrackNumber,
        int totalTracksCount) {
    // Ignore any delayed progress updates after the analysis
    // has already been stopped.
    if (!m_pTrackAnalysisScheduler) {
        return; // inactive
    }
    if (totalTracksCount > 0) {
        setTitleProgress(currentTrackNumber, totalTracksCount);
    } else {
        resetTitle();
    }
}

void AnalysisFeature::onTrackAnalysisSchedulerFinished() {
    if (!m_pTrackAnalysisScheduler) {
        return; // already inactive
    }
    kLogger.info() << "Finishing analysis";
    if (m_pTrackAnalysisScheduler) {
        // Free resources by abandoning the queue after the batch analysis
        // has completed. Batch analysis are not started very frequently
        // during a session and should be avoided while performing live.
        // If the user decides to start a new batch analysis the setup costs
        // for creating the queue with its worker threads are acceptable.
        m_pTrackAnalysisScheduler.reset();
    }
    resetTitle();
    emit analysisActive(false);
}

bool AnalysisFeature::dropAccept(const QList<QUrl>& urls, QObject* pSource) {
    const QList<mixxx::FileInfo> fileInfos =
            // collect all tracks, accept playlist files
            DragAndDropHelper::supportedTracksFromUrls(urls, false, true);
    const QList<TrackId> trackIds =
            m_pLibrary->trackCollectionManager()->resolveTrackIds(fileInfos, pSource);
    QList<AnalyzerScheduledTrack> tracks;
    for (auto trackId : trackIds) {
        tracks.append(trackId);
    }
    analyzeTracks(tracks);
    return tracks.size() > 0;
}

bool AnalysisFeature::dragMoveAccept(const QList<QUrl>& urls) {
    return DragAndDropHelper::urlsContainSupportedTrackFiles(urls, false);
}
