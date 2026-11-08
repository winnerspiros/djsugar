#include "qml/qmlstemsproxy.h"

#include <QFileInfo>
#include <QUrl>

#include "moc_qmlstemsproxy.cpp"
#include "qml/qmlconfigproxy.h"
#include "qml/qmllibrarytracklistmodel.h"
#include "stems/stemcompanion.h"
#include "stems/stemsmanager.h"

namespace mixxx {
namespace qml {

namespace {

const QString kGroup = QStringLiteral("[Stems]");
const ConfigKey kDefaultModelKey(kGroup, QStringLiteral("DefaultModel"));
const ConfigKey kFileSizeKey(kGroup, QStringLiteral("FileSize"));
const ConfigKey kWhilePlayingKey(kGroup, QStringLiteral("WhilePlaying"));
const ConfigKey kChargingOnlyKey(kGroup, QStringLiteral("ChargingOnly"));

stems::Model modelFromName(const QString& name) {
    return name == QStringLiteral("htdemucs") ? stems::Model::HTDemucs : stems::Model::Spleeter;
}

QString modelName(stems::Model model) {
    return model == stems::Model::Spleeter ? QStringLiteral("spleeter")
                                           : QStringLiteral("htdemucs");
}

QString locationOf(const QVariant& track) {
    if (track.canConvert<QUrl>() && track.metaType().id() == QMetaType::QUrl) {
        return track.toUrl().toLocalFile();
    }
    const QString text = track.toString();
    if (text.startsWith(QStringLiteral("file:"))) {
        return QUrl(text).toLocalFile();
    }
    return text;
}

QString displayName(const QString& location) {
    return QFileInfo(location).completeBaseName();
}

} // namespace

QmlStemsProxy::QmlStemsProxy(QObject* pParent)
        : QObject(pParent),
          m_pManager(stems::StemsManager::instance(QmlConfigProxy::get())),
          m_downloadProgress(0.0) {
    connect(m_pManager, &stems::StemsManager::jobsChanged, this, &QmlStemsProxy::jobsChanged);
    connect(m_pManager,
            &stems::StemsManager::progressChanged,
            this,
            &QmlStemsProxy::progressChanged);
    connect(m_pManager, &stems::StemsManager::holdChanged, this, &QmlStemsProxy::holdChanged);
    connect(m_pManager,
            &stems::StemsManager::jobFinished,
            this,
            [this](const QString& location, const QString&, const QString& error) {
                emit trackFinished(location, displayName(location), error);
                emit jobsChanged();
            });
    connect(m_pManager,
            &stems::StemsManager::benchmarkFinished,
            this,
            [this](stems::Model model, double speed, const QString& error) {
                emit benchmarkChanged();
                emit benchmarkFinished(modelName(model), speed, error);
            });

    stems::StemModels* pModels = m_pManager->models();
    connect(pModels,
            &stems::StemModels::downloadProgress,
            this,
            [this](stems::Model, qint64 received, qint64 total) {
                m_downloadProgress = total > 0 ? static_cast<double>(received) / total : 0.0;
                emit downloadChanged();
            });
    connect(pModels,
            &stems::StemModels::downloadFinished,
            this,
            [this](stems::Model model, const QString& error) {
                m_downloadingModel.clear();
                m_downloadProgress = 0.0;
                emit downloadChanged();
                emit modelsChanged();
                if (!error.isEmpty() && error != QStringLiteral("canceled")) {
                    emit downloadFailed(modelName(model), error);
                }
            });
}

// static
QmlStemsProxy* QmlStemsProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlStemsProxy(pQmlEngine);
}

bool QmlStemsProxy::spleeterInstalled() const {
    return stems::StemModels::isInstalled(stems::Model::Spleeter);
}

bool QmlStemsProxy::htdemucsInstalled() const {
    return stems::StemModels::isInstalled(stems::Model::HTDemucs);
}

QVariantList QmlStemsProxy::jobs() const {
    QVariantList jobs;
    for (const auto& job : m_pManager->jobs()) {
        QString state;
        switch (job.state) {
        case stems::StemsManager::JobState::Queued:
            state = QStringLiteral("queued");
            break;
        case stems::StemsManager::JobState::Running:
            state = QStringLiteral("running");
            break;
        case stems::StemsManager::JobState::Done:
            state = QStringLiteral("done");
            break;
        case stems::StemsManager::JobState::Failed:
            state = QStringLiteral("failed");
            break;
        }
        jobs.append(QVariantMap{
                {QStringLiteral("location"), job.location},
                {QStringLiteral("name"), displayName(job.location)},
                {QStringLiteral("model"), modelName(job.model)},
                {QStringLiteral("state"), state},
                {QStringLiteral("error"), job.error},
        });
    }
    return jobs;
}

int QmlStemsProxy::pendingCount() const {
    int count = 0;
    for (const auto& job : m_pManager->jobs()) {
        if (job.state == stems::StemsManager::JobState::Queued ||
                job.state == stems::StemsManager::JobState::Running) {
            ++count;
        }
    }
    return count;
}

QString QmlStemsProxy::runningName() const {
    for (const auto& job : m_pManager->jobs()) {
        if (job.state == stems::StemsManager::JobState::Running) {
            return displayName(job.location);
        }
    }
    return {};
}

QString QmlStemsProxy::stage() const {
    switch (m_pManager->stage()) {
    case stems::StemsWorker::Stage::Loading:
        return QStringLiteral("loading");
    case stems::StemsWorker::Stage::Decoding:
        return QStringLiteral("decoding");
    case stems::StemsWorker::Stage::Separating:
        return QStringLiteral("separating");
    case stems::StemsWorker::Stage::Saving:
        return QStringLiteral("saving");
    }
    return {};
}

double QmlStemsProxy::progress() const {
    return m_pManager->progress();
}

int QmlStemsProxy::secondsLeft() const {
    return m_pManager->secondsLeft();
}

QString QmlStemsProxy::hold() const {
    switch (m_pManager->hold()) {
    case stems::StemsManager::Hold::None:
        return {};
    case stems::StemsManager::Hold::Paused:
        return QStringLiteral("paused");
    case stems::StemsManager::Hold::Playing:
        return QStringLiteral("playing");
    case stems::StemsManager::Hold::NotCharging:
        return QStringLiteral("charging");
    }
    return {};
}

bool QmlStemsProxy::benchmarking() const {
    return m_pManager->isBenchmarking();
}

int QmlStemsProxy::totalMemoryMb() const {
    return stems::totalMemoryMb();
}

int QmlStemsProxy::availableMemoryMb() const {
    return stems::availableMemoryMb();
}

QString QmlStemsProxy::defaultModel() const {
    return m_pManager->config()->getValue(kDefaultModelKey, QStringLiteral("ask"));
}

void QmlStemsProxy::setDefaultModel(const QString& model) {
    m_pManager->config()->setValue(kDefaultModelKey, model);
    m_pManager->config()->save();
    emit settingsChanged();
}

QString QmlStemsProxy::fileSize() const {
    return m_pManager->config()->getValue(kFileSizeKey, QStringLiteral("normal"));
}

void QmlStemsProxy::setFileSize(const QString& fileSize) {
    m_pManager->config()->setValue(kFileSizeKey, fileSize);
    m_pManager->config()->save();
    emit settingsChanged();
}

QString QmlStemsProxy::whilePlaying() const {
    return m_pManager->config()->getValue(kWhilePlayingKey, QStringLiteral("slow"));
}

void QmlStemsProxy::setWhilePlaying(const QString& whilePlaying) {
    m_pManager->config()->setValue(kWhilePlayingKey, whilePlaying);
    m_pManager->config()->save();
    emit settingsChanged();
}

bool QmlStemsProxy::chargingOnly() const {
    return m_pManager->config()->getValue(kChargingOnlyKey, false);
}

void QmlStemsProxy::setChargingOnly(bool chargingOnly) {
    m_pManager->config()->setValue(kChargingOnlyKey, chargingOnly);
    m_pManager->config()->save();
    emit settingsChanged();
}

int QmlStemsProxy::modelSizeMb(const QString& model) const {
    return static_cast<int>(stems::StemModels::downloadSize(modelFromName(model)) / (1024 * 1024));
}

void QmlStemsProxy::downloadModel(const QString& model) {
    if (!m_downloadingModel.isEmpty()) {
        return;
    }
    m_downloadingModel = model;
    m_downloadProgress = 0.0;
    emit downloadChanged();
    m_pManager->models()->download(modelFromName(model));
}

void QmlStemsProxy::cancelDownload() {
    m_pManager->models()->cancelDownload();
}

void QmlStemsProxy::removeModel(const QString& model) {
    stems::StemModels::remove(modelFromName(model));
    emit modelsChanged();
}

bool QmlStemsProxy::hasStems(const QVariant& track) const {
    return !stems::findStemFile(locationOf(track)).isEmpty();
}

QStringList QmlStemsProxy::listLocations(QAbstractItemModel* pTrackList) const {
    auto* pModel = qobject_cast<QmlLibraryTrackListModel*>(pTrackList);
    if (!pModel) {
        return {};
    }
    // The track lists load their rows on demand
    while (pModel->canFetchMore(QModelIndex())) {
        pModel->fetchMore(QModelIndex());
    }
    QStringList locations;
    locations.reserve(pModel->rowCount());
    for (int row = 0; row < pModel->rowCount(); ++row) {
        const QUrl url = pModel->getUrl(row);
        if (url.isValid() && url.isLocalFile()) {
            locations.append(url.toLocalFile());
        }
    }
    return locations;
}

QVariantMap QmlStemsProxy::listStats(QAbstractItemModel* pTrackList) const {
    const QStringList locations = listLocations(pTrackList);
    int withStems = 0;
    for (const QString& location : locations) {
        if (!stems::findStemFile(location).isEmpty()) {
            ++withStems;
        }
    }
    return QVariantMap{
            {QStringLiteral("total"), locations.size()},
            {QStringLiteral("withStems"), withStems},
    };
}

int QmlStemsProxy::separateTrack(const QVariant& track, const QString& model, bool replace) {
    return m_pManager->enqueue({locationOf(track)}, modelFromName(model), replace);
}

int QmlStemsProxy::separateList(
        QAbstractItemModel* pTrackList, const QString& model, bool replace) {
    return m_pManager->enqueue(listLocations(pTrackList), modelFromName(model), replace);
}

void QmlStemsProxy::cancel(const QString& location) {
    m_pManager->cancel(location);
}

void QmlStemsProxy::cancelAll() {
    m_pManager->cancelAll();
}

void QmlStemsProxy::clearFinished() {
    m_pManager->removeFinished();
}

void QmlStemsProxy::pause() {
    m_pManager->setPaused(true);
}

void QmlStemsProxy::resume() {
    m_pManager->setPaused(false);
}

bool QmlStemsProxy::deleteTrackStems(const QVariant& track) {
    return m_pManager->deleteStems({locationOf(track)}) > 0;
}

int QmlStemsProxy::deleteListStems(QAbstractItemModel* pTrackList) {
    return m_pManager->deleteStems(listLocations(pTrackList));
}

int QmlStemsProxy::storedStemsMb() const {
    return static_cast<int>(m_pManager->storedStemsSize() / (1024 * 1024));
}

int QmlStemsProxy::deleteStoredStems() {
    return m_pManager->deleteStoredStems();
}

void QmlStemsProxy::benchmark(const QString& model) {
    m_pManager->benchmark(modelFromName(model));
    emit benchmarkChanged();
}

double QmlStemsProxy::measuredSpeed(const QString& model) const {
    return m_pManager->measuredSpeed(modelFromName(model));
}

} // namespace qml
} // namespace mixxx
