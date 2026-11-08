#pragma once

#include <QFuture>
#include <QFutureWatcher>
#include <QList>
#include <QString>

#include "library/baseexternallibraryfeature.h"
#include "library/baseexternalplaylistmodel.h"
#include "library/treeitemmodel.h"
#include "util/parented_ptr.h"

class TrackCollectionManager;

class EngineDJPlaylistModel : public BaseExternalPlaylistModel {
    Q_OBJECT
  public:
    EngineDJPlaylistModel(QObject* parent,
            TrackCollectionManager* pTrackCollectionManager,
            QSharedPointer<BaseTrackCache> trackSource);

  protected:
    void initSortColumnMapping() override;
};

/// Reads the Engine Library of drives prepared with Engine DJ (Denon / Numark
/// players), so their playlists and crates can be played directly from the
/// drive.
class EngineDJFeature : public BaseExternalLibraryFeature {
    Q_OBJECT
  public:
    /// A playlist or crate of a device. `name` is unique among all devices.
    struct PlaylistNode {
        QString label;
        QString name;
        QList<PlaylistNode> children;
    };
    struct Device {
        QString path;
        QString label;
    };
    struct ParseResult {
        QString devicePath;
        QString error;
        QList<PlaylistNode> playlists;
    };

    EngineDJFeature(Library* pLibrary, UserSettingsPointer pConfig);
    ~EngineDJFeature() override;

    QVariant title() override;
    TreeItemModel* sidebarModel() const override;

  public slots:
    void activate() override;
    void activateChild(const QModelIndex& index) override;

  private slots:
    void onDevicesFound();
    void onDeviceParsed();

  private:
    std::unique_ptr<BaseSqlTableModel> createPlaylistModelForPlaylist(
            const QVariant& data) override;
    void showPlaylist(const QString& name);

    parented_ptr<TreeItemModel> m_pSidebarModel;
    parented_ptr<EngineDJPlaylistModel> m_pPlaylistModel;
    QSharedPointer<BaseTrackCache> m_trackSource;

    QFutureWatcher<QList<Device>> m_devicesFutureWatcher;
    QFuture<QList<Device>> m_devicesFuture;
    QFutureWatcher<ParseResult> m_parseFutureWatcher;
    QFuture<ParseResult> m_parseFuture;
    QString m_title;
};
