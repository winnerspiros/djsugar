#pragma once

#include <QAbstractListModel>
#include <QQmlEngine>
#include <QString>
#include <QVector>

namespace mixxx {
namespace qml {

/// Suggested next-track model for QML.
/// Priority: AutoDJ queue first, then recently-added library tracks (not on any deck).
/// refresh() re-queries the DB; call on first show and after deck eject.
class QmlSuggestionsModel : public QAbstractListModel {
    Q_OBJECT
    Q_PROPERTY(int count READ count WRITE setCount NOTIFY countChanged)
    QML_NAMED_ELEMENT(SuggestionsModel)

  public:
    enum Roles {
        TrackIdRole = Qt::UserRole + 1,
        TitleRole,
        ArtistRole,
        DurationSecRole,
        BpmRole,
        LocationRole,
        SourceRole, // "autodj" or "recent"
    };

    explicit QmlSuggestionsModel(QObject* parent = nullptr);

    int rowCount(const QModelIndex& parent = {}) const override;
    QVariant data(const QModelIndex& index, int role) const override;
    QHash<int, QByteArray> roleNames() const override;

    int count() const {
        return m_count;
    }
    void setCount(int c);

    /// Re-query the DB. Cheap enough to call on every deck change.
    Q_INVOKABLE void refresh();

    /// Load suggestion at row onto the next free deck.
    Q_INVOKABLE void loadOntoNextDeck(int row);

    /// Append suggestion at row to the AutoDJ queue.
    Q_INVOKABLE void appendToAutoDj(int row);

  signals:
    void countChanged();

  private:
    struct Row {
        int trackId = -1;
        QString title;
        QString artist;
        int durationSec = 0;
        double bpm = 0.0;
        QString location;
        QString source;
    };

    QVector<Row> m_rows;
    int m_count = 4;
};

} // namespace qml
} // namespace mixxx
