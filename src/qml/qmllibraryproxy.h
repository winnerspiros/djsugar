#pragma once
#include <qqmlintegration.h>

#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <Qt>
#include <memory>

#include "qml_owned_ptr.h"
#include "qmllibrarytracklistmodel.h"
#include "qml/qmltrackproxy.h"

class Library;
class LibraryScanner;
class KeyboardEventFilter;

namespace mixxx {
namespace qml {

class QmlLibraryProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(mixxx::qml::QmlLibraryTrackListModel* model MEMBER m_pModelProperty CONSTANT)
    Q_PROPERTY(QQmlListProperty<QmlLibrarySource> sources READ sources CONSTANT)
    Q_PROPERTY(mixxx::qml::QmlLibraryScannerProxy* scanner MEMBER m_pScanner CONSTANT)
    QML_NAMED_ELEMENT(Library)
    QML_SINGLETON

  public:
    enum class AddResult {
        Ok,
        AlreadyWatching,
        InvalidOrMissingDirectory,
        UnreadableDirectory,
        SqlError,
    };
    Q_ENUM(AddResult);
    enum class RemoveResult {
        Ok,
        NotFound,
        SqlError,
    };
    Q_ENUM(RemoveResult);
    enum class RelocateResult {
        Ok,
        InvalidOrMissingDirectory,
        UnreadableDirectory,
        SqlError,
    };
    Q_ENUM(RelocateResult);
    enum class SourceRemovalType {
        KeepTracks,
        HideTracks,
        PurgeTracks
    };
    Q_ENUM(SourceRemovalType);

    explicit QmlLibraryProxy(std::shared_ptr<Library> pLibrary, QObject* parent = nullptr);
    ~QmlLibraryProxy() override;

    static QmlLibraryProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerLibrary(std::shared_ptr<Library> pLibrary) {
        s_pLibrary = std::move(pLibrary);
    }

    static Library* get() {
        return s_pLibrary.get();
    }

    QQmlListProperty<QmlLibrarySource> sources() {
        return {this,
                nullptr,
                nullptr,
                &QmlLibraryProxy::sources_count,
                &QmlLibraryProxy::sources_at,
                &QmlLibraryProxy::sources_clear};
    }

    Q_INVOKABLE AddResult addSource(const QUrl& newPath);
    Q_INVOKABLE RemoveResult removeSource(const QUrl& oldPath, SourceRemovalType type);
    Q_INVOKABLE RelocateResult relinkSource(const QUrl& oldPath, const QUrl& newPath);

    static void registerKeyboardEventFilter(std::shared_ptr<KeyboardEventFilter> pKeyboard) {
        s_pKeyboard = std::move(pKeyboard);
    }

    static KeyboardEventFilter* getKeyboard() {
        return s_pKeyboard.get();
    }

    QmlLibraryTrackListModel* model() const;
    Q_INVOKABLE void analyze(const mixxx::qml::QmlTrackProxy* track) const;
    Q_INVOKABLE QString deckHotcueLabel(
            mixxx::qml::QmlTrackProxy* track,
            int hotcueNumber) const;
    Q_INVOKABLE bool setDeckHotcueLabel(
            mixxx::qml::QmlTrackProxy* track,
            int hotcueNumber,
            const QString& label);
    Q_INVOKABLE bool setDeckHotcueType(
            mixxx::qml::QmlTrackProxy* track,
            const QString& group,
            int hotcueNumber,
            const QString& action);
    Q_INVOKABLE void cleanupDeckHotcuePopup(
            mixxx::qml::QmlTrackProxy* track,
            int hotcueNumber);

  private:
    static inline std::shared_ptr<Library> s_pLibrary;

    std::shared_ptr<Library> m_pLibrary;

    /// This needs to be a plain pointer because it's used as a `Q_PROPERTY` member variable.
    QmlLibraryTrackListModel* m_pModelProperty;
    QmlLibraryScannerProxy* m_pScanner;

    static qsizetype sources_count(QQmlListProperty<QmlLibrarySource>* property);
    static QmlLibrarySource* sources_at(
            QQmlListProperty<QmlLibrarySource>* property, qsizetype index);
    static void sources_clear(QQmlListProperty<QmlLibrarySource>* property);
    static inline std::shared_ptr<KeyboardEventFilter> s_pKeyboard;
};

} // namespace qml
} // namespace mixxx
