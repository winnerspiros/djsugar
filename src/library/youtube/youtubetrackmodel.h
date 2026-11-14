#pragma once

#include <QObject>
#include <QSharedPointer>
#include <QString>
#include <QUrl>
#include <QVariant>

#include "library/baseexternaltrackmodel.h"
#include "library/basetrackcache.h"
#include "library/coverart.h"
#include "library/trackcollectionmanager.h"

namespace mixxx {
struct YouTubeVideoInfo;
}

/// Track-table model backed by the `youtube_library` SQL table.
/// Downloaded rows have a real location path; placeholder rows use
/// `youtube://VIDEOID` sentinels to avoid downloading on metadata reads.
class YouTubeTrackModel : public BaseExternalTrackModel {
    Q_OBJECT
  public:
    /// Sentinel `youtube://VIDEOID` for placeholder rows (unique via videoId).
    static const QString kPlaceholderScheme;
    QString thumbnailDir() const {
        return m_thumbnailDir;
    }

    YouTubeTrackModel(QObject* parent,
            TrackCollectionManager* pTrackCollectionManager,
            QSharedPointer<BaseTrackCache> trackSource);

    TrackPointer getTrack(const QModelIndex& index) const override;
    QUrl getTrackUrl(const QModelIndex& index) const override;
    TrackId getTrackId(const QModelIndex& index) const override;
    TrackModel::Capabilities getCapabilities() const override;
    /// Override — does NOT dispatch a YouTube search. Only searchNow() (Enter
    /// key) triggers a search, avoiding network requests per keystroke.
    void search(const QString& searchText) override;

    /// Emit searchRequested with the given query (Enter key presses).
    void searchNow(const QString& query);

    /// Serves YouTube thumbnail (hqdefault.jpg) per row instead of coverart columns.
    CoverInfo getCoverInfo(const QModelIndex& index) const override;

    /// Set the thumbnail image directory (must be set before rendering).
    void setThumbnailDir(const QString& dir);

    /// Notify whether more search results are available.
    void setHasMore(bool hasMore);

    /// Emit fetchMoreRequested() when the view scrolls to the bottom.
    bool canFetchMore(const QModelIndex& parent) const override;
    void fetchMore(const QModelIndex& parent) override;

  signals:
    /// Emitted from `search()` to dispatch `YouTubeService::searchVideos(query)`.
    void searchRequested(const QString& query);

    /// Emitted from fetchMore() to load the next search results page.
    void fetchMoreRequested();

  protected:
    QString resolveLocation(const QString& nativeLocation) const override;

  private:
    QString m_thumbnailDir;
    bool m_hasMore = false;
};
