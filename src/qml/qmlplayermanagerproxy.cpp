#include "qml/qmlplayermanagerproxy.h"

#include <QFileInfo>
#include <QQmlEngine>
#include <QRegularExpression>

#include "library/library.h"
#include "library/library_prefs.h"
#include "library/samples/bundledsamples.h"
#include "mixer/playermanager.h"
#include "moc_qmlplayermanagerproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "qml/qmllibraryproxy.h"
#include "qml/qmlplayerproxy.h"
#include "sources/soundsourceproxy.h"
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif
#include "track/track_decl.h"
#include "util/fileinfo.h"
#include "util/sandbox.h"

namespace mixxx {
namespace qml {

QmlPlayerManagerProxy::QmlPlayerManagerProxy(
        std::shared_ptr<PlayerManager> pPlayerManager, QObject* parent)
        : QObject(parent),
          m_pPlayerManager(pPlayerManager) {
}

QmlPlayerProxy* QmlPlayerManagerProxy::getPlayer(const QString& group) {
    BaseTrackPlayer* pPlayer = m_pPlayerManager->getPlayer(group);
    if (!pPlayer) {
        qWarning() << "PlayerManagerProxy failed to find player for group" << group;
        return nullptr;
    }

    // Don't set a parent here, so that the QML engine deletes the object when
    // the corresponding JS object is garbage collected.
    QmlPlayerProxy* pPlayerProxy = new QmlPlayerProxy(pPlayer);
    QQmlEngine::setObjectOwnership(pPlayerProxy, QQmlEngine::JavaScriptOwnership);
    connect(m_pPlayerManager.get(),
            &PlayerManager::trackAnalyzerProgress,
            pPlayerProxy,
            &QmlPlayerProxy::slotTrackAnalyzerProgress);
    if (Library* pLibrary = QmlLibraryProxy::get()) {
        connect(pLibrary,
                &Library::onTrackAnalyzerProgress,
                pPlayerProxy,
                &QmlPlayerProxy::slotTrackAnalyzerProgress);
    }
    connect(pPlayerProxy,
            &QmlPlayerProxy::loadTrackFromLocationRequested,
            this,
            [this, group](const QString& trackLocation, bool play) {
                loadLocationToPlayer(trackLocation, group, play);
            });
    connect(pPlayerProxy,
            &QmlPlayerProxy::loadTrackRequested,
            this,
            [this, group](TrackPointer track,
#ifdef __STEM__
                    mixxx::StemChannelSelection stemSelection,
#endif
                    bool play) {
                loadTrackToPlayer(track, group,
#ifdef __STEM__
                        stemSelection,
#endif
                        play);
            });
    connect(pPlayerProxy,
            &QmlPlayerProxy::cloneFromGroup,
            this,
            [this, group](const QString& sourceGroup) {
                m_pPlayerManager->slotCloneDeck(sourceGroup, group);
            });
    return pPlayerProxy;
}

void QmlPlayerManagerProxy::loadLocationIntoNextAvailableDeck(
        const QString& trackLocation, bool play) {
#ifdef __ANDROID_USB_DRIVE__
    // Tracks of USB drives that Mixxx reads itself are copied first
    // (from JaviLabarumDJ/mixxx-android android-stable).
    mixxx::usbdrive::fetchTrack(trackLocation, this, [this, trackLocation, play]() {
        m_pPlayerManager->slotLoadLocationIntoNextAvailableDeck(trackLocation, play);
    });
#else
    m_pPlayerManager->slotLoadLocationIntoNextAvailableDeck(trackLocation, play);
#endif
}

void QmlPlayerManagerProxy::loadLocationUrlIntoNextAvailableDeck(
        const QUrl& trackLocationUrl, bool play) {
    if (trackLocationUrl.isLocalFile()) {
        loadLocationIntoNextAvailableDeck(trackLocationUrl.toLocalFile(), play);
    } else {
        // Non-local URLs (e.g. youtube://) must be routed through the same
        // PlayerManager → Library path so that the YouTube feature can
        // download the video and load it onto the next available deck.
        loadLocationIntoNextAvailableDeck(trackLocationUrl.toString(), play);
    }
}

void QmlPlayerManagerProxy::loadLocationUrlToDeck(
        const QUrl& locationUrl, int deck) {
    if (!locationUrl.isLocalFile()) {
        qWarning() << "QmlPlayerManagerProxy: URL" << locationUrl
                   << "is not a local file!";
        return;
    }
    const QString location = locationUrl.toLocalFile();
    auto load = [this, location, deck]() {
        mixxx::FileInfo fileInfo(location);
        Sandbox::createSecurityToken(&fileInfo);
        m_pPlayerManager->slotLoadToDeck(fileInfo.location(), deck);
    };
#ifdef __ANDROID_USB_DRIVE__
    // Tracks of USB drives that Mixxx reads itself are copied first
    // (from JaviLabarumDJ/mixxx-android android-stable).
    mixxx::usbdrive::fetchTrack(location, this, load);
#else
    load();
#endif
}

QVariantList QmlPlayerManagerProxy::bundledSamples() const {
    QVariantList list;
    const QStringList files = mixxx::samples::bundledSampleFiles(m_pPlayerManager->config());
    for (const QString& path : files) {
        QString name = QFileInfo(path).completeBaseName();
        // "01_kick" -> "kick", "dj-scratch" -> "dj scratch"
        static const QRegularExpression kNumberPrefix(QStringLiteral("^\\d+_"));
        name.remove(kNumberPrefix);
        name.replace(QLatin1Char('_'), QLatin1Char(' '));
        name.replace(QLatin1Char('-'), QLatin1Char(' '));
        list.append(QVariantMap{{QStringLiteral("name"), name},
                {QStringLiteral("path"), path}});
    }
    return list;
}

void QmlPlayerManagerProxy::loadLocationToPlayer(
        const QString& location, const QString& group, bool play) {
#ifdef __ANDROID_USB_DRIVE__
    // Tracks of USB drives that Mixxx reads itself are copied first
    // (from JaviLabarumDJ/mixxx-android android-stable).
    mixxx::usbdrive::fetchTrack(location, this, [this, location, group, play]() {
        m_pPlayerManager->slotLoadLocationToPlayer(location, group, play);
    });
#else
    m_pPlayerManager->slotLoadLocationToPlayer(location, group, play);
#endif
}

void QmlPlayerManagerProxy::loadTrackToPlayer(TrackPointer track,
        const QString& group,
#ifdef __STEM__
        mixxx::StemChannelSelection stemSelection,
#endif
        bool play) {
    m_pPlayerManager->slotLoadTrackToPlayer(track, group,
#ifdef __STEM__
            stemSelection,
#endif
            play);
}

void QmlPlayerManagerProxy::showNoDeckPassthroughInputConfiguredWarning() {
    emit m_pPlayerManager->noDeckPassthroughInputConfigured();
}

void QmlPlayerManagerProxy::showNoVinylControlInputConfiguredWarning() {
    emit m_pPlayerManager->noVinylControlInputConfigured();
}

QStringList QmlPlayerManagerProxy::supportedAudioFileNameFilters() const {
    return {tr("Audio (%1)")
                    .arg(SoundSourceProxy::getSupportedFileNamePatterns().join(QLatin1Char(' ')))};
}

QUrl QmlPlayerManagerProxy::initialTrackDirectoryUrl() const {
    const UserSettingsPointer pConfig = QmlConfigProxy::get();
    return pConfig
            ? QUrl::fromLocalFile(pConfig->getValueString(
                      mixxx::library::prefs::kLegacyDirectoryConfigKey))
            : QUrl();
}

// static
QmlPlayerManagerProxy* QmlPlayerManagerProxy::create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine) {
    // The implementation of this method is mostly taken from the code example
    // that shows the replacement for `qmlRegisterSingletonInstance()` when
    // using `QML_SINGLETON`.
    // https://doc.qt.io/qt-6/qqmlengine.html#QML_SINGLETON

    // The instance has to exist before it is used. We cannot replace it.
    VERIFY_OR_DEBUG_ASSERT(s_pPlayerManager) {
        qWarning() << "PlayerManager hasn't been registered yet";
        return nullptr;
    }
    return new QmlPlayerManagerProxy(s_pPlayerManager, pQmlEngine);
}

} // namespace qml
} // namespace mixxx
