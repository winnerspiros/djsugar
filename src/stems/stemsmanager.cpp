#include "stems/stemsmanager.h"

#include <QCoreApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJniObject>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QStandardPaths>

#include "control/controlobject.h"
#include "moc_stemsmanager.cpp"
#include "stems/stemcompanion.h"
#include "util/logger.h"
#ifdef __ANDROID_USB_DRIVE__
#include "library/usbdrive/androidusbdrive.h"
#endif

namespace mixxx {
namespace stems {

namespace {

const Logger kLogger("StemsManager");

const QString kGroup = QStringLiteral("[Stems]");
const ConfigKey kQueueKey(kGroup, QStringLiteral("Queue"));
const ConfigKey kFileSizeKey(kGroup, QStringLiteral("FileSize"));
const ConfigKey kWhilePlayingKey(kGroup, QStringLiteral("WhilePlaying"));
const ConfigKey kChargingOnlyKey(kGroup, QStringLiteral("ChargingOnly"));

constexpr int kDecks = 4;
// Threads that run the models: all big cores of a phone, or two while a
// deck plays so that the audio keeps its share
constexpr int kThreads = 4;
constexpr int kThreadsWhilePlaying = 2;

StemsManager* s_pInstance = nullptr;

QString modelName(Model model) {
    return model == Model::Spleeter ? QStringLiteral("spleeter") : QStringLiteral("htdemucs");
}

Model modelFromName(const QString& name) {
    return name == QStringLiteral("htdemucs") ? Model::HTDemucs : Model::Spleeter;
}

ConfigKey speedKey(Model model) {
    return ConfigKey(kGroup, QStringLiteral("Speed_") + modelName(model));
}

int bitrateKbps(const QString& fileSize) {
    if (fileSize == QStringLiteral("light")) {
        return 96;
    }
    if (fileSize == QStringLiteral("high")) {
        return 192;
    }
    return 128;
}

QJniObject androidContext() {
    return QNativeInterface::QAndroidApplication::context();
}

bool isCharging() {
    return QJniObject::callStaticMethod<jboolean>("org/mixxx/StemsService",
            "isCharging",
            "(Landroid/content/Context;)Z",
            androidContext().object<jobject>());
}

} // namespace

// static
StemsManager* StemsManager::instance(UserSettingsPointer pConfig) {
    if (!s_pInstance) {
        s_pInstance = new StemsManager(std::move(pConfig), QCoreApplication::instance());
    }
    return s_pInstance;
}

StemsManager::StemsManager(UserSettingsPointer pConfig, QObject* pParent)
        : QObject(pParent),
          m_pConfig(std::move(pConfig)),
          m_pWorker(new StemsWorker),
          m_hold(Hold::None),
          m_userPaused(false),
          m_stage(StemsWorker::Stage::Loading),
          m_progress(0.0),
          m_benchmarking(false) {
    m_thread.setObjectName(QStringLiteral("Stems"));
    m_pWorker->moveToThread(&m_thread);
    connect(&m_thread, &QThread::finished, m_pWorker, &QObject::deleteLater);
    connect(m_pWorker, &StemsWorker::progress, this, &StemsManager::slotProgress);
    connect(m_pWorker, &StemsWorker::finished, this, &StemsManager::slotFinished);
    connect(m_pWorker,
            &StemsWorker::benchmarkFinished,
            this,
            [this](int model, double speed, const QString& error) {
                m_benchmarking = false;
                if (error.isEmpty()) {
                    m_pConfig->setValue(speedKey(static_cast<Model>(model)), speed);
                    m_pConfig->save();
                }
                emit benchmarkFinished(static_cast<Model>(model), speed, error);
                updateHold();
                startNextJob();
            });
    m_thread.start(QThread::LowPriority);

    m_holdTimer.setInterval(1000);
    connect(&m_holdTimer, &QTimer::timeout, this, &StemsManager::updateHold);

    loadQueue();
}

StemsManager::~StemsManager() {
    m_pWorker->cancel();
    m_thread.quit();
    m_thread.wait();
}

int StemsManager::enqueue(const QStringList& locations, Model model, bool replace) {
    int added = 0;
    for (const QString& location : locations) {
        // The tracks of USB drives that Mixxx reads itself are copied when
        // they are separated
        bool onDrive = false;
#ifdef __ANDROID_USB_DRIVE__
        onDrive = mixxx::usbdrive::isOnDrive(location);
#endif
        if (location.isEmpty() || (!onDrive && !QFileInfo(location).isFile())) {
            continue;
        }
        if (!replace && !findStemFile(location).isEmpty()) {
            continue;
        }
        bool queued = false;
        for (int i = m_jobs.size() - 1; i >= 0; --i) {
            if (m_jobs[i].location != location) {
                continue;
            }
            if (m_jobs[i].state == JobState::Queued || m_jobs[i].state == JobState::Running) {
                queued = true;
            } else {
                m_jobs.removeAt(i);
            }
        }
        if (queued) {
            continue;
        }
        Job job;
        job.location = location;
        job.model = model;
        m_jobs.append(job);
        ++added;
    }
    if (added > 0) {
        // The progress notification, Android 13+ asks for it once
        QNativeInterface::QAndroidApplication::runOnAndroidMainThread([]() {
            QJniObject::callStaticMethod<void>("org/mixxx/StemsService",
                    "requestNotificationPermission",
                    "(Landroid/content/Context;)V",
                    androidContext().object<jobject>());
        });
        saveQueue();
        emit jobsChanged();
        updateHold();
        startNextJob();
    }
    return added;
}

void StemsManager::cancel(const QString& location) {
    if (location == m_runningLocation) {
        m_pWorker->cancel();
    }
    for (int i = m_jobs.size() - 1; i >= 0; --i) {
        if (m_jobs[i].location == location) {
            m_jobs.removeAt(i);
        }
    }
    saveQueue();
    emit jobsChanged();
    updateService();
}

void StemsManager::cancelAll() {
    if (!m_runningLocation.isEmpty()) {
        m_pWorker->cancel();
    }
    for (int i = m_jobs.size() - 1; i >= 0; --i) {
        if (m_jobs[i].state == JobState::Queued || m_jobs[i].state == JobState::Running) {
            m_jobs.removeAt(i);
        }
    }
    saveQueue();
    emit jobsChanged();
    updateService();
}

void StemsManager::removeFinished() {
    for (int i = m_jobs.size() - 1; i >= 0; --i) {
        if (m_jobs[i].state == JobState::Done || m_jobs[i].state == JobState::Failed) {
            m_jobs.removeAt(i);
        }
    }
    emit jobsChanged();
}

void StemsManager::setPaused(bool paused) {
    m_userPaused = paused;
    updateHold();
}

int StemsManager::secondsLeft() const {
    if (m_runningLocation.isEmpty() || m_progress < 0.03 || !m_jobTimer.isValid()) {
        return -1;
    }
    return static_cast<int>(m_jobTimer.elapsed() / 1000.0 * (1.0 - m_progress) / m_progress);
}

void StemsManager::startNextJob() {
    if (!m_runningLocation.isEmpty() || m_benchmarking || m_hold != Hold::None) {
        return;
    }
    for (Job& job : m_jobs) {
        if (job.state != JobState::Queued) {
            continue;
        }
        if (!StemModels::isInstalled(job.model)) {
            job.state = JobState::Failed;
            job.error = QStringLiteral("model");
            continue;
        }
        job.state = JobState::Running;
        m_runningLocation = job.location;
        m_stage = StemsWorker::Stage::Decoding;
        m_progress = 0.0;
        m_jobTimer.start();
        const int bitrate = bitrateKbps(m_pConfig->getValue(kFileSizeKey, QStringLiteral("normal")));
        kLogger.info() << "Separating" << job.location << "with" << modelName(job.model);
        QMetaObject::invokeMethod(m_pWorker,
                "run",
                Qt::QueuedConnection,
                Q_ARG(QString, job.location),
                Q_ARG(int, static_cast<int>(job.model)),
                Q_ARG(int, bitrate),
                Q_ARG(int, threadsForJob()));
        break;
    }
    emit jobsChanged();
    emit progressChanged();
    updateService();
}

void StemsManager::slotProgress(
        const QString& location, StemsWorker::Stage stage, double fraction) {
    if (location != m_runningLocation) {
        return;
    }
    m_stage = stage;
    // Decoding takes about a tenth of the time
    switch (stage) {
    case StemsWorker::Stage::Decoding:
        m_progress = 0.1 * fraction;
        break;
    case StemsWorker::Stage::Loading:
        m_progress = 0.1;
        break;
    case StemsWorker::Stage::Separating:
        m_progress = 0.1 + 0.88 * fraction;
        break;
    case StemsWorker::Stage::Saving:
        m_progress = 0.98;
        break;
    }
    emit progressChanged();
    updateService();
}

void StemsManager::slotFinished(
        const QString& location, const QString& stemFile, const QString& error) {
    if (location == m_runningLocation) {
        m_runningLocation.clear();
    }
    for (Job& job : m_jobs) {
        if (job.location == location && job.state == JobState::Running) {
            job.state = error.isEmpty() ? JobState::Done : JobState::Failed;
            job.error = error;
        }
    }
    if (error.isEmpty()) {
        kLogger.info() << "Separated" << location << "into" << stemFile;
    } else if (error != QStringLiteral("canceled")) {
        kLogger.warning() << "Unable to separate" << location << ":" << error;
    }
    saveQueue();
    emit jobFinished(location, stemFile, error);
    startNextJob();
}

void StemsManager::updateHold() {
    Hold hold = Hold::None;
    if (m_userPaused) {
        hold = Hold::Paused;
    } else if (m_pConfig->getValue(kChargingOnlyKey, false) && !isCharging()) {
        hold = Hold::NotCharging;
    } else if (m_pConfig->getValue(kWhilePlayingKey, QStringLiteral("slow")) ==
                    QStringLiteral("pause") &&
            isAnyDeckPlaying()) {
        hold = Hold::Playing;
    }
    m_pWorker->setHold(hold != Hold::None);

    bool pending = false;
    for (const Job& job : std::as_const(m_jobs)) {
        pending = pending || job.state == JobState::Queued || job.state == JobState::Running;
    }
    if (pending && !m_holdTimer.isActive()) {
        m_holdTimer.start();
    } else if (!pending) {
        m_holdTimer.stop();
    }
    if (hold != m_hold) {
        m_hold = hold;
        emit holdChanged();
        updateService();
        startNextJob();
    }
}

bool StemsManager::isAnyDeckPlaying() const {
    for (int deck = 1; deck <= kDecks; ++deck) {
        if (ControlObject::get(ConfigKey(QStringLiteral("[Channel%1]").arg(deck),
                    QStringLiteral("play"))) > 0.0) {
            return true;
        }
    }
    return false;
}

int StemsManager::threadsForJob() const {
    const int threads = isAnyDeckPlaying() ? kThreadsWhilePlaying : kThreads;
    return std::max(1, std::min(threads, QThread::idealThreadCount()));
}

void StemsManager::saveQueue() {
    QJsonArray queue;
    for (const Job& job : std::as_const(m_jobs)) {
        if (job.state == JobState::Queued || job.state == JobState::Running) {
            queue.append(QJsonObject{
                    {QStringLiteral("location"), job.location},
                    {QStringLiteral("model"), modelName(job.model)},
            });
        }
    }
    m_pConfig->setValue(kQueueKey,
            QString::fromUtf8(QJsonDocument(queue).toJson(QJsonDocument::Compact)));
    // Android may close the app without the usual shutdown
    m_pConfig->save();
}

void StemsManager::loadQueue() {
    const QJsonArray queue = QJsonDocument::fromJson(
            m_pConfig->getValue(kQueueKey, QString()).toUtf8())
                                     .array();
    for (const auto& value : queue) {
        const QJsonObject object = value.toObject();
        Job job;
        job.location = object.value(QStringLiteral("location")).toString();
        job.model = modelFromName(object.value(QStringLiteral("model")).toString());
        bool onDrive = false;
#ifdef __ANDROID_USB_DRIVE__
        onDrive = mixxx::usbdrive::isOnDrive(job.location);
#endif
        if (onDrive || QFileInfo(job.location).isFile()) {
            m_jobs.append(job);
        }
    }
    if (!m_jobs.isEmpty()) {
        // Never start by itself: the app may have been closed by Android
        // while separating
        m_userPaused = true;
        updateHold();
    }
}

void StemsManager::updateService() {
    int done = 0;
    int total = 0;
    for (const Job& job : std::as_const(m_jobs)) {
        if (job.state != JobState::Failed) {
            ++total;
        }
        if (job.state == JobState::Done) {
            ++done;
        }
    }
    const bool active = !m_runningLocation.isEmpty() && m_hold == Hold::None;
    QString text;
    if (active) {
        text = tr("Track %1 of %2: %3%")
                       .arg(done + 1)
                       .arg(total)
                       .arg(qRound(m_progress * 100));
    }
    if (text == m_serviceText) {
        return;
    }
    m_serviceText = text;
    const QString title = tr("Separating stems");
    QNativeInterface::QAndroidApplication::runOnAndroidMainThread([text, title]() {
        if (text.isEmpty()) {
            QJniObject::callStaticMethod<void>("org/mixxx/StemsService",
                    "stop",
                    "(Landroid/content/Context;)V",
                    androidContext().object<jobject>());
            return;
        }
        QJniObject::callStaticMethod<void>("org/mixxx/StemsService",
                "start",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
                androidContext().object<jobject>(),
                QJniObject::fromString(title).object<jstring>(),
                QJniObject::fromString(text).object<jstring>());
    });
}

int StemsManager::deleteStems(const QStringList& locations) {
    int deleted = 0;
    for (const QString& location : locations) {
        cancel(location);
        bool removed = QFile::remove(companionPath(location));
        removed = QFile::remove(fallbackPath(location)) || removed;
        if (removed) {
            ++deleted;
        }
    }
    return deleted;
}

int StemsManager::deleteStoredStems() {
    int deleted = 0;
    const QDir dir(QFileInfo(fallbackPath(QStringLiteral("x"))).absolutePath());
    for (const QString& name : dir.entryList(QDir::Files)) {
        if (QFile::remove(dir.filePath(name))) {
            ++deleted;
        }
    }
    return deleted;
}

qint64 StemsManager::storedStemsSize() const {
    qint64 size = 0;
    QDirIterator it(QFileInfo(fallbackPath(QStringLiteral("x"))).absolutePath(), QDir::Files);
    while (it.hasNext()) {
        it.next();
        size += it.fileInfo().size();
    }
    return size;
}

void StemsManager::benchmark(Model model) {
    if (m_benchmarking || !m_runningLocation.isEmpty()) {
        return;
    }
    m_benchmarking = true;
    QMetaObject::invokeMethod(m_pWorker,
            "benchmark",
            Qt::QueuedConnection,
            Q_ARG(int, static_cast<int>(model)),
            Q_ARG(int, threadsForJob()));
}

double StemsManager::measuredSpeed(Model model) const {
    return m_pConfig->getValue(speedKey(model), 0.0);
}

} // namespace stems
} // namespace mixxx
