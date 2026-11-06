#pragma once

#include <QCryptographicHash>
#include <QFile>
#include <QList>
#include <QObject>
#include <QPointer>

#include "stems/stemseparator.h"

class QNetworkAccessManager;
class QNetworkReply;

namespace mixxx {
namespace stems {

/// The model files, downloaded on demand from the release
/// "stems-models-v1" of the app's GitHub repository into the app's storage
class StemModels : public QObject {
    Q_OBJECT
  public:
    explicit StemModels(QObject* pParent = nullptr);
    ~StemModels() override;

    static QString modelDir(Model model);
    static bool isInstalled(Model model);
    /// Size of the download in bytes
    static qint64 downloadSize(Model model);
    /// Deletes the files of a model
    static void remove(Model model);

    void download(Model model);
    void cancelDownload();
    bool isDownloading() const {
        return m_pReply != nullptr;
    }

  signals:
    void downloadProgress(mixxx::stems::Model model, qint64 received, qint64 total);
    /// Emitted with an empty error on success
    void downloadFinished(mixxx::stems::Model model, const QString& error);

  private:
    void downloadNextFile();
    void finishDownload(const QString& error);

    QNetworkAccessManager* m_pNetwork;
    QPointer<QNetworkReply> m_pReply;
    Model m_model;
    int m_fileIndex;
    qint64 m_receivedBefore;
    QFile m_file;
    QCryptographicHash m_hash;
};

} // namespace stems
} // namespace mixxx
