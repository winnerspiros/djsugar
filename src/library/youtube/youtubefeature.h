#pragma once

#include <QHash>
#include <QNetworkAccessManager>
#include <QPointer>
#include <QQueue>
#include <QSet>
#include <QSharedPointer>
#include <QTimer>

#include "analyzer/analyzerprogress.h"
#include "library/baseexternallibraryfeature.h"
#include "library/dao/playlistdao.h"
#include "library/youtube/youtubeservice.h"
#include "track/trackid.h"
#include "util/parented_ptr.h"

class BaseTrackCache;
namespace mixxx {
class YouTubeGenreLookup;
} // namespace mixxx
class KeyboardEventFilter;
class QSqlDatabase;
class TreeItem;
class WLibrary;
class WLibraryTextBrowser;
class WSearchLineEdit;
class YouTubeTrackModel;

class YouTubeFeature : public BaseExternalLibraryFeature {
    Q_OBJECT
  public:
    YouTubeFeature(Library* pLibrary, UserSettingsPointer pConfig);
    void prefetchThumbnails(const QList<mixxx::YouTubeVideoInfo>& videos) {
        fetchThumbnails(videos);
    }
    ~YouTubeFeature() override = default;

    QVariant title() override {
        return tr("YouTube");
    }
    void activate() override;
    void activateChild(const QModelIndex& index) override;
    TreeItemModel* sidebarModel() const override;
    void bindLibraryWidget(WLibrary* pLibraryWidget,
            KeyboardEventFilter* pKeyboard) override;

    /// Connect the search box's returnPressed signal so YouTube searches fire on Enter only.
    void bindSearchboxWidget(WSearchLineEdit* pSearchboxWidget);

    void searchAndActivate(const QString& query);

    /// Fetch the next batch of results for the current search (infinite scroll).
    void loadMoreResults();

    const QString& lastQuery() const {
        return m_lastQuery;
    }

    void requestDownloadToPlayer(
            const QString& videoId, const QString& group, bool play);
    void requestDownload(const QString& videoId);
    /// Results shown outside YouTube search (e.g. SoundCloud); download gets title/artist like a YouTube result.
    void rememberExternalResults(const QList<mixxx::YouTubeVideoInfo>& results);
    void requestDownloadToAutoDJ(
            const QString& videoId, PlaylistDAO::AutoDJSendLoc loc);

    /// Expose the underlying service so other features can connect to its signals (e.g. AIBroFeature).
    mixxx::YouTubeService* service() {
        return &m_service;
    }

    /// Absolute path to the per-user yt-dlp cache directory. Created on demand.
    QString cacheDir() const;
    /// Per video ID: "timesPlayed", "bpm", "key" from library, and "sameAs" (ID of earlier same-song result).
    QHash<QString, QVariantMap> libraryInfoForResults(
            const QList<mixxx::YouTubeVideoInfo>& results) const;

    /// Resolve the ISO 3166-1 alpha-2 region for YouTube top-songs feed.
    /// Order: 1) [YouTube]/trending_region override, 2) "GR" (Greece) default.
    /// Stale geo-IP cache/en_US locale must not switch to US. Returns non-empty 2-letter code.
    QString resolvedTrendingRegion() const;

    /// Whether SponsorBlock auto-skip is enabled (default: true).
    bool sponsorBlockAutoSkip() const;
    void setSponsorBlockAutoSkip(bool enabled);

  signals:
    void downloadToPlayerRequested(
            const QString& videoId,
            const QString& group,
            const QString& title);
    /// Every deck waiting on `videoId` has its track, or the load was abandoned. Fires for fresh downloads and cache hits (never reach service's downloadFinished).
    void playerLoadsSettled(const QString& videoId);
    /// The file of `videoId` is on disk: downloaded or from cache.
    void fileReady(const QString& videoId, const QString& localPath);
    void sponsorBlockAutoSkipChanged(bool enabled);

  protected:
    void appendTrackIdsFromRightClickIndex(
            QList<TrackId>* trackIds, QString* pPlaylist) override;
    void onRightClick(const QPoint& globalPos) override;

  private slots:
    void onSearchResultsReady(
            const QString& query, const QList<mixxx::YouTubeVideoInfo>& results);
    void onSearchFailed(const QString& query, const QString& error);
    void onDownloadFinished(const QString& videoId, const QString& localPath);
    void onDownloadFailed(const QString& videoId, const QString& error);
    void onTrackAnalysisProgress(TrackId trackId, AnalyzerProgress analyzerProgress);
    void slotCleanCache();
    /// Dispatch clicks on links rendered in the home pane HTML.
    /// `ytplay:VIDEOID`     → download/cache/analyze the track.
    /// `ytcached:LOCALPATH` → refresh the already-downloaded row.
    void onHomeAnchorClicked(const QUrl& url);
    /// Append additional results to the track table (used when the user
    /// scrolls to the bottom and fetchMore() fetches the next InnerTube page).
    void onSearchMoreReady(
            const QString& query, const QList<mixxx::YouTubeVideoInfo>& results);

  private:
    /// Rebuild the sidebar tree from current search-result and downloaded-track caches.
    void rebuildSidebar();
    /// Rebuild HTML for the main YOUTUBE_HOME pane (right-hand area). Mirrors sidebar so user sees results before unfolding tree node.
    void rebuildHomeHtml();
    void requestDownloadFile(const QString& videoId);
    /// Background repair of missing AutoDJ-queued tracks.
    void requestPrefetch(const QString& videoId);
    bool autoAnalyzeResultsEnabled() const;
    void setAutoAnalyzeResultsEnabled(bool enabled);
    void autoAnalyzeCurrentResults();
    /// If `pTrack` was downloaded by us and is no longer on any deck, delete its cached audio file and purge from library DB. Tracks referenced by any playlist/crate (incl. AutoDJ queue) are preserved.
    void maybeReleaseCachedTrack(const TrackPointer& pTrack);
    /// If `pTrack` is a YouTube-cache track whose file is missing, re-download in background so next play succeeds. No-op for non-YouTube tracks or present files.
    void ensureDownloaded(const TrackPointer& pTrack);
    /// Walk AutoDJ queue at startup; ensure every YouTube-cache track is on disk, re-downloading swept ones.
    void prefetchAutoDjQueue();
    void syncAnalyzedTrackMetadata(const TrackPointer& pTrack);
    void fetchGenre(const TrackPointer& pTrack,
            const QString& videoId,
            const QString& artist,
            const QString& title,
            int durationSec);
    /// Persistent track table backing the right-hand pane. See
    /// YouTubeTrackModel for the placeholder/downloaded row model.
    QSharedPointer<BaseTrackCache> m_pTrackCache;
    YouTubeTrackModel* m_pTrackModel = nullptr;
    /// Replace all rows in `youtube_library` with `videos`, then refresh model. `videos` may include downloaded and not-yet-downloaded entries; we resolve per row. `attempt` bounds deferred retries when write loses SQLite lock to library scanner.
    void replaceTrackTable(
            const QList<mixxx::YouTubeVideoInfo>& videos, int attempt = 0);
    /// Append rows for `videos` to `youtube_library` without wiping table. Used when InnerTube returns continuation page so new batch appears below existing results.
    void appendToTrackTable(const QList<mixxx::YouTubeVideoInfo>& videos);
    /// Append/update a single downloaded entry so "Downloaded" column reflects new file without full table rebuild.
    void upsertDownloadedRow(const QString& videoId,
            const QString& localPath,
            const QString& title,
            const QString& uploader,
            int durationSec);
    /// Push newly-arrived thumbnail onto live Track when audio file was already downloaded (deck loaded before thumb).
    void backfillCoverArt(const QString& videoId, const QString& thumbPath);

    // ----- Cover art (YouTube thumbnails) -----

    /// Subdirectory of cacheDir() for per-video thumbnail images.
    QString thumbnailDir() const;
    /// For each video in `videos`, download `hqdefault.jpg` from YouTube's image CDN to `thumbnailDir()/<videoId>.jpg` if not present. On completion, fires dataChanged() on model so cover-art delegate repaints.
    void fetchThumbnails(const QList<mixxx::YouTubeVideoInfo>& videos);
    /// Set of videoIds whose thumbnails are currently being downloaded.
    QSet<QString> m_thumbnailsDownloading;
    /// Pending thumbnail videoIds waiting for a download slot. Caps parallel thumbnail TCP+TLS handshakes (mobile data) — see kMaxParallelThumbs.
    QQueue<QString> m_thumbnailQueue;
    void pumpThumbnailQueue();

    parented_ptr<TreeItemModel> m_pSidebarModel;
    QPointer<WLibraryTextBrowser> m_pHomeView;
    QPointer<WSearchLineEdit> m_pSearchbox;
    mixxx::YouTubeService m_service;
    /// Debounce timer for rebuildSidebar() + rebuildHomeHtml() — coalesces rapid calls (e.g. during batch auto-analyze downloads) so UI thread doesn't churn HTML on every download completion.
    QTimer* m_rebuildTimer = nullptr;
    /// When true, next rebuildTimer fire also runs buildIndex()+select() to pick up rows updated by upsertDownloadedRow() or analysis completion. Set by those paths instead of calling inline so N rapid completions share one rebuild.
    bool m_pendingModelUpdate = false;
    /// Schedule a deferred rebuildSidebar() + rebuildHomeHtml(). Multiple calls within debounce window trigger only one rebuild.
    void scheduleRebuild();
    /// Debounce timer for thumbnail dataChanged() notifications. Coalesces N simultaneous thumbnail arrivals into a single model repaint.
    QTimer* m_thumbnailTimer = nullptr;
    /// Set when at least one thumbnail arrived since last repaint.
    bool m_thumbnailsDirty = false;
    QString m_lastQuery;
    QList<mixxx::YouTubeVideoInfo> m_lastResults;
    QHash<QString, mixxx::YouTubeVideoInfo> m_externalResults;
    const mixxx::YouTubeVideoInfo* resultInfo(const QString& videoId) const;
    /// Guards the deferred one-shot genre fetch in activate().
    bool m_genresFetchScheduled = false;
    // videoId -> human-readable label (used for the "Downloaded" branch).
    QHash<QString, QString> m_downloadedTracks;
    struct PendingPlayerLoad {
        QString group;
        bool play = false;
    };
    QHash<QString, QList<PendingPlayerLoad>> m_pendingPlayerLoads;
    /// Drops the deck loads waiting on `videoId` after a failed hand-over.
    void abandonPendingLoads(const QString& videoId);
    QHash<QString, QList<PlaylistDAO::AutoDJSendLoc>> m_pendingAutoDjLoads;
    QSet<QString> m_videoIdsDownloading;
    /// Track download retry attempts per video id. After kMaxDownloadRetries
    /// we give up and surface the error to the user.
    QHash<QString, int> m_downloadRetryCount;
    static constexpr int kMaxDownloadRetries = 2;
#ifdef Q_OS_ANDROID
    static constexpr int kMaxParallelDownloads = 3;
#else
    static constexpr int kMaxParallelDownloads = 4;
#endif
    /// FIFO of videoIds waiting for a download slot. Parallel yt-dlp downloads share one radio; 3-at-a-time on Android fills pipe while user browses (5-14s/track) and leaves CPU for analyzer/library writes.
    QList<QString> m_downloadQueue;
    /// Start queued downloads while slots are free.
    void pumpDownloadQueue();
    /// Last error from YouTubeService for current `m_lastQuery`. Cleared on new search or successful results. When non-empty, home pane shows error instead of "Searching..." placeholder.
    QString m_lastSearchError;
    /// Auto-discovered music genres from YouTube Music's browse API. Populated async on first activate(); falls back to kDefaultGenres when empty (fetch failed or not completed).
    QStringList m_discoveredGenres;
    /// True while a trending fetch from activate() is in flight; repeated activations don't fire duplicate requests. Cleared when results arrive, fetch fails, or user search supersedes.
    bool m_trendingFetchInFlight = false;
    /// True when recent searchAndActivate() came from sidebar Samples section (kSampleQueryPrefix). First result auto-downloaded to next sampler slot instead of displayed in table.
    bool m_samplerTargetSearch = false;

    // ----- Samples section (DJ Tools + Greek Memes from myinstants.com) -----

    /// A sound scraped from myinstants.com (name + CDN MP3 URL).
    struct MyInstantSound {
        QString name;
        QString mp3Url; // full CDN URL: https://www.myinstants.com/media/sounds/…
    };

    /// Fetched list of Greek meme sounds. Populated async on first expand.
    QList<MyInstantSound> m_myInstantSounds;
    /// True while a myinstants.com fetch is in-flight.
    bool m_myInstantsFetchInFlight = false;
    /// QNetworkAccessManager for Samples section (myinstants fetches + MP3 downloads). Separate from YouTubeService so cookie jars and rate-limiting aren't shared.
    QNetworkAccessManager* m_pSamplesNam = nullptr;
    /// Genre of downloaded songs from iTunes/Deezer catalogues.
    mixxx::YouTubeGenreLookup* m_pGenreLookup = nullptr;
    /// Set of myinstants CDN URLs currently being downloaded (prevent dupes).
    QSet<QString> m_myInstantsDownloading;

    /// Fetch the Greek myinstants.com page and populate m_myInstantSounds.
    void fetchMyInstantsSounds();
    /// Download a single myinstants MP3 to myInstantsCacheDir() and load it.
    void downloadMyInstant(const QString& mp3Url, const QString& displayName);
    /// Subdirectory of cacheDir() for myinstants MP3 files.
    QString myInstantsCacheDir() const;
    /// Find next empty sampler slot and emit loadTrackToPlayer(). Falls back to loadTrack() (next deck) when all 32 slots full.
    void loadTrackToNextSampler(const TrackPointer& pTrack);
};
