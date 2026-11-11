#pragma once

#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QString>

namespace mixxx {
namespace soundcloud {

/// The SoundCloud sign-in (AccountLoginActivity): the account's OAuth
/// token, which SoundCloud's own player sends. With it, a Go+ account gets
/// the full Go+ tracks (yt-dlp: --username oauth --password <token>).
inline QString accountFile() {
    const QString dir =
            QDir(QStandardPaths::writableLocation(QStandardPaths::AppDataLocation))
                    .filePath(QStringLiteral("soundcloud"));
    QDir().mkpath(dir);
    return QDir(dir).filePath(QStringLiteral("account"));
}

/// "" when not signed in
inline QString token() {
    QFile file(accountFile());
    return file.open(QIODevice::ReadOnly) ? QString::fromUtf8(file.readAll()).trimmed()
                                          : QString();
}

inline void setToken(const QString& token) {
    if (token.isEmpty()) {
        QFile::remove(accountFile());
        return;
    }
    QFile file(accountFile());
    if (file.open(QIODevice::WriteOnly | QIODevice::Truncate)) {
        file.setPermissions(QFileDevice::ReadOwner | QFileDevice::WriteOwner);
        file.write(token.toUtf8());
    }
}

} // namespace soundcloud
} // namespace mixxx
