#include "library/samples/bundledsamples.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QMutex>
#include <QStandardPaths>

#include "library/samples/sampleeditor.h"
#include "sources/soundsourceproxy.h"

namespace mixxx {
namespace samples {

namespace {

const QStringList kSampleFilters = {
        QStringLiteral("*.mp3"),
        QStringLiteral("*.wav"),
        QStringLiteral("*.aiff"),
        QStringLiteral("*.m4a"),
        QStringLiteral("*.ogg"),
        QStringLiteral("*.flac"),
};

QString bundledSourceDir(const UserSettingsPointer& pConfig) {
    const QStringList candidates = {
            pConfig->getResourcePath() + QStringLiteral("samples/"),
            QStringLiteral("assets:/samples/"),
            QStringLiteral("res/samples/"),
    };
    for (const auto& candidate : candidates) {
        // The index marks the APK assets, which may not report as a folder
        if (QFile::exists(candidate + QStringLiteral("index.txt")) ||
                QFileInfo(candidate).isDir()) {
            // Callers append relative paths: one trailing slash
            return candidate.startsWith(QStringLiteral("assets:"))
                    ? candidate
                    : QDir(candidate).absolutePath() + QLatin1Char('/');
        }
    }
    return QString();
}

/// Paths relative to sourceDir. Android cannot list assets:/ subdirectories,
/// so the APK ships an index.txt written at build time.
QStringList bundledRelativePaths(const QString& sourceDir) {
    QStringList paths;
    QFile index(sourceDir + QStringLiteral("index.txt"));
    if (index.open(QIODevice::ReadOnly | QIODevice::Text)) {
        const QStringList lines = QString::fromUtf8(index.readAll()).split(QLatin1Char('\n'));
        for (const QString& line : lines) {
            const QString path = line.trimmed();
            if (!path.isEmpty()) {
                paths.append(path);
            }
        }
        return paths;
    }
    const QDir source(sourceDir);
    QDirIterator it(sourceDir, kSampleFilters, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        paths.append(source.relativeFilePath(it.next()));
    }
    return paths;
}

} // namespace

QString bundledSamplesCacheDir() {
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation) +
            QStringLiteral("/samples/");
}

bool isBundledSample(const QString& path) {
    static const QString bundled = bundledSamplesCacheDir();
    static const QString user = userSamplesDir();
    static const QString work = SampleEditor::workDir();
    return path.startsWith(bundled) || path.startsWith(user) || path.startsWith(work);
}

QString bundledSamplesDir(const UserSettingsPointer& pConfig) {
    const QString sourceDir = bundledSourceDir(pConfig);
    if (sourceDir.isEmpty()) {
        return QString();
    }
    // Cache location keeps the library scanner away from the samples
    const QString destDir = bundledSamplesCacheDir();
    // The pads UI and the FX Station may both sync the cache
    static QMutex mutex;
    const QMutexLocker lock(&mutex);
    const QStringList relativePaths = bundledRelativePaths(sourceDir);
    for (const QString& relativePath : relativePaths) {
        const QString path = sourceDir + relativePath;
        const QString destPath = destDir + relativePath;
        const QFileInfo dest(destPath);
        // Same size: already current. Otherwise an update reworked it.
        if (dest.exists() && dest.size() == QFileInfo(path).size()) {
            continue;
        }
        QFile::remove(destPath);
        QDir().mkpath(dest.absolutePath());
        if (!QFile::copy(path, destPath)) {
            qWarning() << "Failed to copy sample" << path << "to" << destPath;
        }
    }
    // Drop samples that a later version no longer ships
    QDirIterator stale(destDir, kSampleFilters, QDir::Files, QDirIterator::Subdirectories);
    while (stale.hasNext()) {
        const QString path = stale.next();
        if (!relativePaths.contains(QDir(destDir).relativeFilePath(path))) {
            QFile::remove(path);
        }
    }
    return destDir;
}

QString userSamplesDir() {
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation) +
            QStringLiteral("/samples/");
}

bool isSupportedSample(const QString& fileName) {
    return SoundSourceProxy::isFileNameSupported(fileName);
}

QStringList userSampleFiles() {
    QStringList files;
    const QFileInfoList entries = QDir(userSamplesDir())
                                          .entryInfoList(QDir::Files, QDir::Time);
    for (const QFileInfo& fi : entries) {
        if (isSupportedSample(fi.fileName())) {
            files.append(fi.absoluteFilePath());
        }
    }
    return files;
}

QStringList bundledSampleFiles(const UserSettingsPointer& pConfig) {
    const QString dir = bundledSamplesDir(pConfig);
    if (dir.isEmpty()) {
        return {};
    }
    const QDir root(dir);
    QStringList relative;
    QDirIterator it(dir, kSampleFilters, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        relative.append(root.relativeFilePath(it.next()));
    }
    relative.sort(Qt::CaseInsensitive);
    QStringList files;
    files.reserve(relative.size());
    for (const auto& path : std::as_const(relative)) {
        files.append(root.absoluteFilePath(path));
    }
    return files;
}

QStringList defaultSamplerKit(const UserSettingsPointer& pConfig) {
    const QString dir = bundledSamplesDir(pConfig);
    if (dir.isEmpty()) {
        return {};
    }
    const QStringList kit = {
            // Bank A: drums
            QStringLiteral("djsugar/01_kick.wav"),
            QStringLiteral("djsugar/02_snare.wav"),
            QStringLiteral("djsugar/03_clap.wav"),
            QStringLiteral("djsugar/04_hat_closed.wav"),
            QStringLiteral("djsugar/05_hat_open.wav"),
            QStringLiteral("djsugar/06_rim.wav"),
            QStringLiteral("djsugar/12_808.wav"),
            QStringLiteral("djsugar/08_crash.wav"),
            // Bank B: FX
            QStringLiteral("djsugar/14_airhorn.wav"),
            QStringLiteral("djsugar/09_riser.wav"),
            QStringLiteral("djsugar/10_downlifter.wav"),
            QStringLiteral("djsugar/11_sub_drop.wav"),
            QStringLiteral("djsugar/16_noise_sweep.wav"),
            QStringLiteral("dj-scratch.ogg"),
            QStringLiteral("djsugar/15_vinyl_stop.wav"),
            QStringLiteral("djsugar/17_siren.ogg"),
    };
    QStringList files;
    for (const auto& name : kit) {
        const QString path = dir + name;
        files.append(QFile::exists(path) ? path : QString());
    }
    return files;
}

} // namespace samples
} // namespace mixxx
