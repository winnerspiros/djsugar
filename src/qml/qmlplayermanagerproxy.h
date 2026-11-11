#pragma once
#include <QObject>
#include <QQmlEngine>
#include <QString>
#include <QStringList>
#include <QVariantList>

#include "library/samples/samplerecorder.h"
#include "mixer/playermanager.h"
#include "qml/qmlplayerproxy.h"

namespace mixxx {
namespace qml {

class QmlPlayerManagerProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(QStringList supportedAudioFileNameFilters READ
                    supportedAudioFileNameFilters CONSTANT)
    Q_PROPERTY(QUrl initialTrackDirectoryUrl READ initialTrackDirectoryUrl CONSTANT)
    Q_PROPERTY(bool sampleRecording READ sampleRecording NOTIFY sampleRecordingChanged)
    Q_PROPERTY(double sampleRecordSeconds READ sampleRecordSeconds NOTIFY
                    sampleRecordSecondsChanged)
    QML_NAMED_ELEMENT(PlayerManager)
    QML_SINGLETON
  public:
    explicit QmlPlayerManagerProxy(
            std::shared_ptr<PlayerManager> pPlayerManager,
            QObject* parent = nullptr);

    Q_INVOKABLE mixxx::qml::QmlPlayerProxy* getPlayer(const QString& deck);
    Q_INVOKABLE void loadLocationIntoNextAvailableDeck(const QString& location, bool play = false);
    Q_INVOKABLE void loadLocationUrlIntoNextAvailableDeck(
            const QUrl& locationUrl, bool play = false);
    Q_INVOKABLE void loadLocationUrlToDeck(const QUrl& locationUrl, int deck);
    Q_INVOKABLE void loadLocationToPlayer(
            const QString& location, const QString& group, bool play = false);
    Q_INVOKABLE void loadTrackToPlayer(TrackPointer track,
            const QString& group,
#ifdef __STEM__
            mixxx::StemChannelSelection stemSelection,
#endif
            bool play);
    /// Pad samples as {name, path, user}: the user's own (newest first),
    /// then the ones shipped with the app.
    Q_INVOKABLE QVariantList bundledSamples() const;
    /// Copies any decodable audio file into the user's samples. Returns the
    /// new path, or "" (and emits sampleError) on failure.
    Q_INVOKABLE QString importSample(const QUrl& url);
    /// Deletes one of the user's own samples.
    Q_INVOKABLE void deleteSample(const QString& path);
    /// The sound picked for an X-Pad instrument of a user bank (1 to 3)
    Q_INVOKABLE QString fxStationSound(int bank, int instrument) const;
    /// An empty path goes back to the bank folder
    Q_INVOKABLE void setFxStationSound(int bank, int instrument, const QString& path);
    Q_INVOKABLE bool startSampleRecording();
    /// Saves the take (silence trimmed, normalized); returns its path or "".
    Q_INVOKABLE QString stopSampleRecording();
    Q_INVOKABLE void showNoDeckPassthroughInputConfiguredWarning();
    Q_INVOKABLE void showNoVinylControlInputConfiguredWarning();

    QStringList supportedAudioFileNameFilters() const;
    bool sampleRecording() const {
        return m_sampleRecorder.isRecording();
    }
    double sampleRecordSeconds() const {
        return m_sampleRecorder.seconds();
    }
    QUrl initialTrackDirectoryUrl() const;

    static QmlPlayerManagerProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);
    static void registerPlayerManager(std::shared_ptr<PlayerManager> pPlayerManager) {
        s_pPlayerManager = std::move(pPlayerManager);
    }

    /// Non-QML accessor used by C++ helpers (e.g. QmlSuggestionsModel) that
    /// need to load tracks without going through a QQmlEngine instance.
    static PlayerManager* get() {
        return s_pPlayerManager.get();
    }

  signals:
    void sampleRecordingChanged();
    void sampleRecordSecondsChanged();
    void sampleError(const QString& reason);

  private:
    static inline std::shared_ptr<PlayerManager> s_pPlayerManager;

    const std::shared_ptr<PlayerManager> m_pPlayerManager;
    mixxx::samples::SampleRecorder m_sampleRecorder;
};

} // namespace qml
} // namespace mixxx
