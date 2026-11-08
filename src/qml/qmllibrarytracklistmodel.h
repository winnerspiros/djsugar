#pragma once

#include <QIdentityProxyModel>
#include <QObject>
#include <QQmlEngine>
#include <QQmlListProperty>
#include <QUrl>
#include <memory>

#include "library/trackmodel.h"
#include "qml/qmllibrarytracklistcolumn.h"
#include "qml/qmltrackproxy.h"

namespace mixxx {
namespace qml {

class QmlLibraryTrackListModel : public QIdentityProxyModel {
    Q_OBJECT
    Q_PROPERTY(int rowCount READ rowCount NOTIFY rowCountChanged)
    Q_PROPERTY(QQmlListProperty<QmlLibraryTrackListColumn> columns READ columns CONSTANT)
    QML_NAMED_ELEMENT(LibraryTrackListModel)
    QML_UNCREATABLE("Only accessible via Mixxx.Library")
  public:
    enum Role {
        DelegateRole = Qt::UserRole + 1,
        TrackProxyRole,
        FileURLRole,
        CoverArtRole,
    };
    Q_ENUM(Role)

    // Mirrors TrackModel::Capability for QML: Track.qml and TrackList.qml
    // check Mixxx.LibraryTrackListModel.Capability.* (upstream, merged), and
    // without it every menu check threw "Cannot read property 'Capability'".
    enum class Capability {
        None = 0u,
        Reorder = 1u << 0u,
        ReceiveDrops = 1u << 1u,
        AddToTrackSet = 1u << 2u,
        AddToAutoDJ = 1u << 3u,
        Locked = 1u << 4u,
        EditMetadata = 1u << 5u,
        LoadToDeck = 1u << 6u,
        LoadToSampler = 1u << 7u,
        LoadToPreviewDeck = 1u << 8u,
        Remove = 1u << 9u,
        ResetPlayed = 1u << 10u,
        Hide = 1u << 11u,
        Unhide = 1u << 12u,
        Purge = 1u << 13u,
        RemovePlaylist = 1u << 14u,
        RemoveCrate = 1u << 15u,
        RemoveFromDisk = 1u << 16u,
        Analyze = 1u << 17u,
        Properties = 1u << 18u,
        Sorting = 1u << 19u,
    };
    Q_ENUM(Capability)

    QmlLibraryTrackListModel(
            const QList<QmlLibraryTrackListColumn*>& librarySource,
            QAbstractItemModel* pModel,
            QObject* pParent = nullptr);
    ~QmlLibraryTrackListModel() override = default;

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& proxyIndex, int role = Qt::DisplayRole) const override;
    int columnCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant headerData(int section, Qt::Orientation orientation, int role = Qt::DisplayRole) const override;
    QHash<int, QByteArray> roleNames() const override;

    QQmlListProperty<QmlLibraryTrackListColumn> columns() {
        return {this, &m_columns};
    }

    Q_INVOKABLE QUrl getUrl(int row) const;
    Q_INVOKABLE mixxx::qml::QmlTrackProxy* getTrack(int row) const;

    Q_INVOKABLE TrackModel::Capabilities getCapabilities() const;
    Q_INVOKABLE bool hasCapabilities(TrackModel::Capabilities caps) const;

    Q_INVOKABLE void sort(int column, Qt::SortOrder order = Qt::AscendingOrder) override;
    Q_INVOKABLE void search(const QString& searchText);

    int sourceColumn(int column) const;

  signals:
    void rowCountChanged();

  private:
    QList<QmlLibraryTrackListColumn*> m_columns;
};

} // namespace qml
} // namespace mixxx
