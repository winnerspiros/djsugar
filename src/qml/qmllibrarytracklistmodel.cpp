#include "qml/qmllibrarytracklistmodel.h"

#include <QAbstractProxyModel>
#include <QColor>
#include <QObject>
#include <QQmlEngine>
#include <QSortFilterProxyModel>
#include <QVariant>

#include "library/basesqltablemodel.h"
#include "library/basetracktablemodel.h"
#include "library/browse/browsetablemodel.h"
#include "library/columncache.h"
#include "qml/asyncimageprovider.h"
#include "qml/librarycoverloader.h"
#include "qml/qmllibrarytracklistcolumn.h"
#include "qml/qmltrackproxy.h"
#include "qml_owned_ptr.h"
#include "track/track.h"
#include "util/assert.h"
#include "util/parented_ptr.h"

namespace mixxx {
namespace qml {
namespace {
const QHash<int, QByteArray> kRoleNames = {
        {Qt::DisplayRole, "display"},
        {Qt::DecorationRole, "decoration"},
        {QmlLibraryTrackListModel::DelegateRole, "delegate"},
        {QmlLibraryTrackListModel::TrackProxyRole, "track"},
        {QmlLibraryTrackListModel::FileURLRole, "file_url"},
        {QmlLibraryTrackListModel::CoverArtRole, "cover_art"},
};

QColor colorFromRgbCode(double colorValue) {
    if (colorValue < 0 || colorValue > 0xFFFFFF) {
        return {};
    }

    QRgb rgbValue = static_cast<QRgb>(colorValue) | 0xFF000000;
    return QColor(rgbValue);
}

/// The folder browser has its own fixed columns
int browseColumn(int column) {
    switch (static_cast<ColumnCache::Column>(column)) {
    case ColumnCache::COLUMN_LIBRARYTABLE_ALBUM:
        return COLUMN_ALBUM;
    case ColumnCache::COLUMN_LIBRARYTABLE_ARTIST:
        return COLUMN_ARTIST;
    case ColumnCache::COLUMN_LIBRARYTABLE_TITLE:
        return COLUMN_TITLE;
    case ColumnCache::COLUMN_LIBRARYTABLE_YEAR:
        return COLUMN_YEAR;
    case ColumnCache::COLUMN_LIBRARYTABLE_BPM:
        return COLUMN_BPM;
    case ColumnCache::COLUMN_LIBRARYTABLE_KEY:
        return COLUMN_KEY;
    case ColumnCache::COLUMN_LIBRARYTABLE_FILETYPE:
        return COLUMN_TYPE;
    case ColumnCache::COLUMN_LIBRARYTABLE_BITRATE:
        return COLUMN_BITRATE;
    // Folders have no "date added", the time the file was last modified
    // takes its place
    case ColumnCache::COLUMN_LIBRARYTABLE_DATETIMEADDED:
        return COLUMN_FILE_MODIFIED_TIME;
    default:
        return column;
    }
}
} // namespace

int QmlLibraryTrackListModel::sourceColumn(int column) const {
    if (column < 0) {
        return column;
    }
    if (auto* const pTrackTableModel = qobject_cast<BaseTrackTableModel*>(sourceModel())) {
        return pTrackTableModel->fieldIndex(static_cast<ColumnCache::Column>(column));
    }
    // The folder browser is wrapped in a ProxyTrackModel, which keeps the
    // columns of its source
    QAbstractItemModel* pModel = sourceModel();
    while (auto* const pProxyModel = qobject_cast<QAbstractProxyModel*>(pModel)) {
        pModel = pProxyModel->sourceModel();
    }
    if (qobject_cast<BrowseTableModel*>(pModel)) {
        return browseColumn(column);
    }
    return column;
}

QmlLibraryTrackListModel::QmlLibraryTrackListModel(
        const QList<QmlLibraryTrackListColumn*>& librarySource,
        QAbstractItemModel* pModel,
        QObject* pParent)
        : QIdentityProxyModel(pParent),
          m_columns() {
    m_columns.reserve(librarySource.size());
    for (const auto* pColumn : librarySource) {
        m_columns.append(make_parented<QmlLibraryTrackListColumn>(this,
                pColumn->label(),
                pColumn->fillSpan(),
                pColumn->columnIdx(),
                pColumn->preferredWidth(),
                pColumn->autoHideWidth(),
                pColumn->delegate(),
                pColumn->role()));
    }

    auto* pTrackModel = dynamic_cast<TrackModel*>(pModel);
    if (pTrackModel) {
        pTrackModel->select();
    }
    setSourceModel(pModel);
}

int QmlLibraryTrackListModel::rowCount(const QModelIndex& parent) const {
    if (parent.isValid()) {
        return 0;
    }
    return sourceModel() ? sourceModel()->rowCount() : 0;
}

int QmlLibraryTrackListModel::columnCount(const QModelIndex& parent) const {
    if (parent.isValid()) {
        return 0;
    }
    return m_columns.size();
}

QVariant QmlLibraryTrackListModel::data(const QModelIndex& proxyIndex, int role) const {
    if (!proxyIndex.isValid()) {
        return {};
    }

    auto columnIdx = proxyIndex.column();
    if (columnIdx < 0 || columnIdx >= m_columns.size()) {
        return {};
    }

    auto* const pTrackTableModel = qobject_cast<BaseTrackTableModel*>(sourceModel());
    auto* const pTrackModel = dynamic_cast<TrackModel*>(sourceModel());

    const auto& pColumn = m_columns[columnIdx];

    switch (role) {
    case TrackProxyRole: {
        // Only for tracks of a database (not of the folder browser, see
        // CoverArtRole)
        if (dynamic_cast<const BaseSqlTableModel*>(pTrackModel) == nullptr) {
            return {};
        }
        auto pTrack = pTrackModel->getTrack(mapToSource(proxyIndex));
        if (!pTrack) {
            return {};
        }
        return QVariant::fromValue(make_qml_owned<QmlTrackProxy>(pTrack).get());
    }
    case Qt::DecorationRole: {
        if (pTrackTableModel == nullptr) {
            return {};
        };
        return colorFromRgbCode(QIdentityProxyModel::data(
                proxyIndex.siblingAtColumn(pTrackTableModel->fieldIndex(
                        ColumnCache::COLUMN_LIBRARYTABLE_COLOR)),
                Qt::DisplayRole)
                        .toDouble());
    }
    case CoverArtRole: {
        // Loading the Track of a row of the folder browser adds the file to
        // the library, which takes seconds when opening a folder. Only tracks
        // of a database (Mixxx, Rekordbox, Engine DJ) have covers loaded here.
        if (dynamic_cast<const BaseSqlTableModel*>(pTrackModel) == nullptr) {
            return {};
        }
        return LibraryCoverLoader::loadCover(pTrackModel, mapToSource(proxyIndex));
    }
    case FileURLRole: {
        if (pTrackModel == nullptr) {
            return {};
        }
        return pTrackModel->getTrackUrl(mapToSource(proxyIndex));
    }
    case DelegateRole:
        return QVariant::fromValue(pColumn->delegate());
    }

    if (pColumn->columnIdx() < 0) {
        return QIdentityProxyModel::data(proxyIndex, role);
    }

    return QIdentityProxyModel::data(
            proxyIndex.siblingAtColumn(sourceColumn(pColumn->columnIdx())), role);
}

QVariant QmlLibraryTrackListModel::headerData(
        int section, Qt::Orientation orientation, int role) const {
    if (orientation != Qt::Horizontal || section < 0 || section >= m_columns.size()) {
        return {};
    }
    if (role == Qt::DisplayRole) {
        return m_columns[section]->label();
    }
    return {};
}

QHash<int, QByteArray> QmlLibraryTrackListModel::roleNames() const {
    return kRoleNames;
}

QUrl QmlLibraryTrackListModel::getUrl(int row) const {
    auto* const pTrackModel = dynamic_cast<TrackModel*>(sourceModel());
    if (pTrackModel == nullptr || sourceModel() == nullptr) {
        return {};
    }
    if (row < 0 || row >= sourceModel()->rowCount()) {
        return {};
    }
    const QModelIndex srcIndex = sourceModel()->index(row, 0);
    if (!srcIndex.isValid()) {
        return {};
    }
    return pTrackModel->getTrackUrl(srcIndex);
}

QmlTrackProxy* QmlLibraryTrackListModel::getTrack(int row) const {
    auto* const pTrackModel = dynamic_cast<TrackModel*>(sourceModel());
    if (pTrackModel == nullptr || sourceModel() == nullptr) {
        return nullptr;
    }
    if (row < 0 || row >= sourceModel()->rowCount()) {
        return nullptr;
    }
    const QModelIndex srcIndex = sourceModel()->index(row, 0);
    if (!srcIndex.isValid()) {
        return nullptr;
    }
    auto pTrack = pTrackModel->getTrack(srcIndex);
    if (!pTrack) {
        return nullptr;
    }
    return make_qml_owned<QmlTrackProxy>(pTrack).get();
}

TrackModel::Capabilities QmlLibraryTrackListModel::getCapabilities() const {
    auto* const pTrackModel = dynamic_cast<TrackModel*>(sourceModel());
    if (pTrackModel != nullptr) {
        return pTrackModel->getCapabilities();
    }
    return TrackModel::Capability::None;
}

bool QmlLibraryTrackListModel::hasCapabilities(TrackModel::Capabilities caps) const {
    return (getCapabilities() & caps) == caps;
}

void QmlLibraryTrackListModel::sort(int column, Qt::SortOrder order) {
    if (column < 0 || column >= m_columns.size() || sourceModel() == nullptr) {
        return;
    }
    const auto& pColumn = m_columns[column];
    Q_EMIT layoutAboutToBeChanged(QList<QPersistentModelIndex>(),
            QAbstractItemModel::VerticalSortHint);
    sourceModel()->sort(sourceColumn(pColumn->columnIdx()), order);
    Q_EMIT layoutChanged(QList<QPersistentModelIndex>(), QAbstractItemModel::VerticalSortHint);
}

void QmlLibraryTrackListModel::search(const QString& searchText) {
    auto* const pTrackModel = dynamic_cast<TrackModel*>(sourceModel());
    if (pTrackModel != nullptr) {
        pTrackModel->search(searchText);
    }
}

} // namespace qml
} // namespace mixxx

#include "moc_qmllibrarytracklistmodel.cpp"
