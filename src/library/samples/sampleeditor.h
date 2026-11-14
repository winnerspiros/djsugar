#pragma once

#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantMap>

namespace mixxx {
namespace samples {

/// Prepares audio for the trim popup and writes the selected range as a pad
/// sample. Works with any file decoders can read, regardless of extension.
class SampleEditor : public QObject {
    Q_OBJECT
  public:
    explicit SampleEditor(QObject* pParent = nullptr);

    /// Copies the sound to a work file and decodes its overview in the
    /// background; emits opened() or failed()
    void open(const QUrl& url);

    /// Writes [startSeconds, endSeconds] of the open sound into
    /// userSamplesDir() as a stereo WAV with short fades. Returns the new
    /// path, or an empty string.
    QString save(double startSeconds, double endSeconds, const QString& name) const;

    /// The work file, for previews
    QString workFile() const {
        return m_workFile;
    }

    /// Folder of the work files (ends with a slash)
    static QString workDir();

  signals:
    /// {path, name, duration (seconds), peaks (0..1, kOverviewPoints)}
    void opened(const QVariantMap& sound);
    void failed(const QString& reason);

  private:
    QString m_workFile;
    int m_generation;
};

} // namespace samples
} // namespace mixxx
