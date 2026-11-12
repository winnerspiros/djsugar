#pragma once

#include <QDir>
#include <QStandardPaths>
#include <QStringList>

// Library scanner utility methods.
class ScannerUtil {
  public:
    static QStringList getDirectoryBlacklist() {
        QStringList blacklist;
        // The "Album Artwork" folder within iTunes stores album art.  It has
        // hundreds of subfolders but no audio files. We put this folder on a
        // blacklist. On Windows, the iTunes folder is contained within the
        // standard music folder.
        QString iTunesArtFolder = QDir::toNativeSeparators(
                QStandardPaths::writableLocation(QStandardPaths::MusicLocation) +
                "/iTunes/Album Artwork");
        blacklist << iTunesArtFolder;
#ifdef __WINDOWS__
        // Blacklist the _Serato_ directory that pollutes "My Music" on Windows.
        QString seratoDir = QDir::toNativeSeparators(
                QStandardPaths::writableLocation(QStandardPaths::MusicLocation) +
                "/_Serato_");
        blacklist << seratoDir;
#endif
#ifdef __ANDROID__
        // Copies of USB sticks, read like the sticks themselves, see
        // util/removablestorage.cpp (from JaviLabarumDJ/mixxx-android)
        blacklist << QStringLiteral("/storage/emulated/0/Music/Mixxx/Rekordbox")
                  << QStringLiteral("/sdcard/Music/Mixxx/Rekordbox");
#endif
        return blacklist;
    }

    /// Containers of video, which the scanner skips
    static QStringList videoFileTypes() {
        return {QStringLiteral("mp4"),
                QStringLiteral("m4v"),
                QStringLiteral("mov"),
                QStringLiteral("webm"),
                QStringLiteral("mkv"),
                QStringLiteral("avi"),
                QStringLiteral("wmv"),
                QStringLiteral("flv"),
                QStringLiteral("3gp"),
                QStringLiteral("3g2")};
    }

  private:
    ScannerUtil() {
    }
};
