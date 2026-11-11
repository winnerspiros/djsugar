#include "library/samples/bundledsamples.h"

#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QStandardPaths>

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
            QStringLiteral("res/samples/"),
    };
    for (const auto& candidate : candidates) {
        const QFileInfo fi(candidate);
        if (fi.isDir()) {
            return fi.absoluteFilePath();
        }
    }
    return QString();
}

} // namespace

QString bundledSamplesCacheDir() {
    return QStandardPaths::writableLocation(QStandardPaths::CacheLocation) +
            QStringLiteral("/samples/");
}

bool isBundledSample(const QString& path) {
    static const QString dir = bundledSamplesCacheDir();
    return path.startsWith(dir);
}

QString bundledSamplesDir(const UserSettingsPointer& pConfig) {
    const QString sourceDir = bundledSourceDir(pConfig);
    if (sourceDir.isEmpty()) {
        return QString();
    }
    // Cache location keeps the library scanner away from the samples
    const QString destDir = bundledSamplesCacheDir();
    const QDir source(sourceDir);
    QDirIterator it(sourceDir, kSampleFilters, QDir::Files, QDirIterator::Subdirectories);
    while (it.hasNext()) {
        const QString path = it.next();
        const QString destPath = destDir + source.relativeFilePath(path);
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
        if (!QFile::exists(sourceDir + QDir(destDir).relativeFilePath(path))) {
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

QString importSample(const QUrl& url) {
    const QString source = url.isLocalFile() ? url.toLocalFile() : url.toString();
    QFile in(source);
    if (!in.open(QIODevice::ReadOnly)) {
        qWarning() << "Cannot open sample" << source;
        return QString();
    }
    // content:// URLs carry no usable name; Qt resolves the display name
    QString name = QFileInfo(in.fileName()).fileName();
    if (!url.isLocalFile()) {
        const QString display = QFileInfo(url.fileName()).fileName();
        if (!display.isEmpty()) {
            name = display;
        }
    }
    if (!isSupportedSample(name)) {
        qWarning() << "Unsupported sample format" << name;
        return QString();
    }
    const QString dir = userSamplesDir();
    QDir().mkpath(dir);
    const QFileInfo fi(name);
    QString dest = dir + name;
    for (int n = 2; QFile::exists(dest); ++n) {
        dest = dir + QStringLiteral("%1-%2.%3").arg(fi.completeBaseName()).arg(n).arg(fi.suffix());
    }
    QFile out(dest);
    if (!out.open(QIODevice::WriteOnly) || out.write(in.readAll()) < 0) {
        qWarning() << "Cannot write sample" << dest;
        out.remove();
        return QString();
    }
    return dest;
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
