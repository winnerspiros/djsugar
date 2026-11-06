#include "stems/stemmodels.h"

#include <QDir>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QStandardPaths>

#include "moc_stemmodels.cpp"

namespace mixxx {
namespace stems {

namespace {

const QString kBaseUrl = QStringLiteral(
        "https://github.com/JaviLabarumDJ/mixxx-android/releases/download/stems-models-v1/");

struct ModelFile {
    const char* remoteName;
    const char* localName;
    qint64 size;
    const char* sha256;
};

// Spleeter: Deezer's models converted to ONNX (MIT). HTDemucs: Meta's model
// converted to ONNX (MIT), its matrix products quantized to 8 bits.
const QList<ModelFile> kSpleeterFiles = {
        {"spleeter-drums.fp16.onnx",
                "drums.fp16.onnx",
                19714140,
                "7ae4002e5633634674f74dc3356d5875b0da894d59ce0f60e844bb8f9cb8aa92"},
        {"spleeter-bass.fp16.onnx",
                "bass.fp16.onnx",
                19714139,
                "ba4c4949a27222492cca49859901a873b4b71461dc48c7c5a51f93d31eb11f55"},
        {"spleeter-other.fp16.onnx",
                "other.fp16.onnx",
                19714140,
                "3cc59116cb7195946ab9596d8ca25984d09c0f8a70db8cf85d063132f97bc61d"},
        {"spleeter-vocals.fp16.onnx",
                "vocals.fp16.onnx",
                19714141,
                "db47148ab1c52709ce694893f532c91abfe3edc4d46238939570e036a22878ca"},
};
const QList<ModelFile> kHTDemucsFiles = {
        {"htdemucs-int8.onnx",
                "htdemucs.onnx",
                156458446,
                "3e1f78f51d771b1abd0220f2559f2e85bff0f73560a974092f74a1376b04f4bc"},
};

const QList<ModelFile>& modelFiles(Model model) {
    return model == Model::Spleeter ? kSpleeterFiles : kHTDemucsFiles;
}

} // namespace

StemModels::StemModels(QObject* pParent)
        : QObject(pParent),
          m_pNetwork(new QNetworkAccessManager(this)),
          m_model(Model::Spleeter),
          m_fileIndex(0),
          m_receivedBefore(0),
          m_hash(QCryptographicHash::Sha256) {
}

StemModels::~StemModels() {
    cancelDownload();
}

// static
QString StemModels::modelDir(Model model) {
    return QDir(QStandardPaths::writableLocation(QStandardPaths::AppLocalDataLocation))
            .filePath(model == Model::Spleeter ? QStringLiteral("stem-models/spleeter")
                                               : QStringLiteral("stem-models/htdemucs"));
}

// static
bool StemModels::isInstalled(Model model) {
    const QDir dir(modelDir(model));
    for (const auto& file : modelFiles(model)) {
        if (QFileInfo(dir.filePath(file.localName)).size() != file.size) {
            return false;
        }
    }
    return true;
}

// static
qint64 StemModels::downloadSize(Model model) {
    qint64 size = 0;
    for (const auto& file : modelFiles(model)) {
        size += file.size;
    }
    return size;
}

// static
void StemModels::remove(Model model) {
    QDir(modelDir(model)).removeRecursively();
}

void StemModels::download(Model model) {
    if (m_pReply) {
        return;
    }
    m_model = model;
    m_fileIndex = 0;
    m_receivedBefore = 0;
    QDir().mkpath(modelDir(model));
    downloadNextFile();
}

void StemModels::cancelDownload() {
    if (!m_pReply) {
        return;
    }
    QNetworkReply* pReply = m_pReply;
    m_pReply = nullptr;
    pReply->abort();
    pReply->deleteLater();
    m_file.remove();
    emit downloadFinished(m_model, QStringLiteral("canceled"));
}

void StemModels::downloadNextFile() {
    const QList<ModelFile>& files = modelFiles(m_model);
    const QDir dir(modelDir(m_model));
    // Skip the files of an interrupted download that are complete
    while (m_fileIndex < files.size() &&
            QFileInfo(dir.filePath(files[m_fileIndex].localName)).size() ==
                    files[m_fileIndex].size) {
        m_receivedBefore += files[m_fileIndex].size;
        ++m_fileIndex;
    }
    if (m_fileIndex >= files.size()) {
        finishDownload(QString());
        return;
    }
    const ModelFile& modelFile = files[m_fileIndex];
    m_file.setFileName(dir.filePath(QString::fromLatin1(modelFile.localName) +
            QStringLiteral(".part")));
    if (!m_file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        finishDownload(QStringLiteral("Unable to write %1").arg(m_file.fileName()));
        return;
    }
    m_hash.reset();
    QNetworkRequest request(QUrl(kBaseUrl + QString::fromLatin1(modelFile.remoteName)));
    m_pReply = m_pNetwork->get(request);
    connect(m_pReply, &QNetworkReply::readyRead, this, [this]() {
        const QByteArray data = m_pReply->readAll();
        m_file.write(data);
        m_hash.addData(data);
    });
    connect(m_pReply, &QNetworkReply::downloadProgress, this, [this](qint64 received, qint64) {
        emit downloadProgress(m_model, m_receivedBefore + received, downloadSize(m_model));
    });
    connect(m_pReply, &QNetworkReply::finished, this, [this]() {
        QNetworkReply* pReply = m_pReply;
        if (!pReply) {
            // Canceled
            return;
        }
        m_pReply = nullptr;
        pReply->deleteLater();
        const QByteArray rest = pReply->readAll();
        m_file.write(rest);
        m_hash.addData(rest);
        m_file.close();
        const ModelFile& modelFile = modelFiles(m_model)[m_fileIndex];
        if (pReply->error() != QNetworkReply::NoError) {
            m_file.remove();
            finishDownload(pReply->errorString());
            return;
        }
        if (m_file.size() != modelFile.size ||
                m_hash.result().toHex() != QByteArray(modelFile.sha256)) {
            m_file.remove();
            finishDownload(QStringLiteral("The downloaded file is damaged"));
            return;
        }
        const QString path = QDir(modelDir(m_model)).filePath(modelFile.localName);
        QFile::remove(path);
        if (!m_file.rename(path)) {
            finishDownload(QStringLiteral("Unable to write %1").arg(path));
            return;
        }
        m_receivedBefore += modelFile.size;
        ++m_fileIndex;
        downloadNextFile();
    });
}

void StemModels::finishDownload(const QString& error) {
    emit downloadFinished(m_model, error);
}

} // namespace stems
} // namespace mixxx
