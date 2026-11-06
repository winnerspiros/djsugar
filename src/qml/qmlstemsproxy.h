#pragma once

#include <QAbstractItemModel>
#include <QObject>
#include <QQmlEngine>
#include <QVariantList>

namespace mixxx {
namespace stems {
class StemsManager;
}

namespace qml {

/// Separation of tracks into stems, exposed as the `Mixxx.Stems` singleton.
/// Models are named "spleeter" and "htdemucs". Tracks are identified by their
/// URL or file location.
class QmlStemsProxy : public QObject {
    Q_OBJECT
    Q_PROPERTY(bool spleeterInstalled READ spleeterInstalled NOTIFY modelsChanged)
    Q_PROPERTY(bool htdemucsInstalled READ htdemucsInstalled NOTIFY modelsChanged)
    /// The model being downloaded, empty if none
    Q_PROPERTY(QString downloadingModel READ downloadingModel NOTIFY downloadChanged)
    Q_PROPERTY(double downloadProgress READ downloadProgress NOTIFY downloadChanged)
    /// The tracks of the queue, done and failed ones included: maps with
    /// location, name, model and state ("queued", "running", "done",
    /// "failed") and error
    Q_PROPERTY(QVariantList jobs READ jobs NOTIFY jobsChanged)
    /// Tracks waiting or being separated
    Q_PROPERTY(int pendingCount READ pendingCount NOTIFY jobsChanged)
    Q_PROPERTY(QString runningName READ runningName NOTIFY jobsChanged)
    /// "decoding", "loading", "separating" or "saving"
    Q_PROPERTY(QString stage READ stage NOTIFY progressChanged)
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(int secondsLeft READ secondsLeft NOTIFY progressChanged)
    /// Why the queue waits: "", "paused", "playing" or "charging"
    Q_PROPERTY(QString hold READ hold NOTIFY holdChanged)
    Q_PROPERTY(bool benchmarking READ benchmarking NOTIFY benchmarkChanged)
    /// Memory of the device in MB
    Q_PROPERTY(int totalMemoryMb READ totalMemoryMb CONSTANT)

    // Settings
    /// "ask", "spleeter" or "htdemucs"
    Q_PROPERTY(QString defaultModel READ defaultModel WRITE setDefaultModel NOTIFY settingsChanged)
    /// "light", "normal" or "high"
    Q_PROPERTY(QString fileSize READ fileSize WRITE setFileSize NOTIFY settingsChanged)
    /// "slow" (fewer threads) or "pause"
    Q_PROPERTY(QString whilePlaying READ whilePlaying WRITE setWhilePlaying NOTIFY settingsChanged)
    Q_PROPERTY(bool chargingOnly READ chargingOnly WRITE setChargingOnly NOTIFY settingsChanged)
    QML_NAMED_ELEMENT(Stems)
    QML_SINGLETON

  public:
    explicit QmlStemsProxy(QObject* pParent = nullptr);

    static QmlStemsProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    bool spleeterInstalled() const;
    bool htdemucsInstalled() const;
    QString downloadingModel() const {
        return m_downloadingModel;
    }
    double downloadProgress() const {
        return m_downloadProgress;
    }
    QVariantList jobs() const;
    int pendingCount() const;
    QString runningName() const;
    QString stage() const;
    double progress() const;
    int secondsLeft() const;
    QString hold() const;
    bool benchmarking() const;
    int totalMemoryMb() const;

    QString defaultModel() const;
    void setDefaultModel(const QString& model);
    QString fileSize() const;
    void setFileSize(const QString& fileSize);
    QString whilePlaying() const;
    void setWhilePlaying(const QString& whilePlaying);
    bool chargingOnly() const;
    void setChargingOnly(bool chargingOnly);

    /// Download size of a model in MB
    Q_INVOKABLE int modelSizeMb(const QString& model) const;
    Q_INVOKABLE void downloadModel(const QString& model);
    Q_INVOKABLE void cancelDownload();
    Q_INVOKABLE void removeModel(const QString& model);

    Q_INVOKABLE bool hasStems(const QVariant& track) const;
    /// Number of tracks of a list and how many have stems:
    /// {total, withStems}
    Q_INVOKABLE QVariantMap listStats(QAbstractItemModel* pTrackList) const;
    /// Queues tracks. Returns the number of tracks added.
    Q_INVOKABLE int separateTrack(const QVariant& track, const QString& model, bool replace);
    Q_INVOKABLE int separateList(QAbstractItemModel* pTrackList, const QString& model, bool replace);
    Q_INVOKABLE void cancel(const QString& location);
    Q_INVOKABLE void cancelAll();
    Q_INVOKABLE void clearFinished();
    Q_INVOKABLE void pause();
    Q_INVOKABLE void resume();

    Q_INVOKABLE bool deleteTrackStems(const QVariant& track);
    Q_INVOKABLE int deleteListStems(QAbstractItemModel* pTrackList);
    /// Size of the stems kept in the app's storage, in MB
    Q_INVOKABLE int storedStemsMb() const;
    Q_INVOKABLE int deleteStoredStems();

    /// Measures how fast this device separates, see benchmarkFinished()
    Q_INVOKABLE void benchmark(const QString& model);
    /// Seconds of audio separated per second, 0 if not measured
    Q_INVOKABLE double measuredSpeed(const QString& model) const;
    Q_INVOKABLE int availableMemoryMb() const;

  signals:
    void modelsChanged();
    void downloadChanged();
    void downloadFailed(const QString& model, const QString& error);
    void jobsChanged();
    void progressChanged();
    void holdChanged();
    void benchmarkChanged();
    void benchmarkFinished(const QString& model, double speed, const QString& error);
    void settingsChanged();
    /// A track was separated (error empty) or failed
    void trackFinished(const QString& location, const QString& name, const QString& error);

  private:
    QStringList listLocations(QAbstractItemModel* pTrackList) const;

    stems::StemsManager* m_pManager;
    QString m_downloadingModel;
    double m_downloadProgress;
};

} // namespace qml
} // namespace mixxx
