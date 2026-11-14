#pragma once

#include <QString>

namespace mixxx {
namespace android {

/// Request READ_MEDIA_AUDIO (API >= 33) or READ_EXTERNAL_STORAGE (<= 32).
/// On API <= 29 also requests WRITE_EXTERNAL_STORAGE.
/// Blocks for the system dialog. Must not be called from the Android UI thread.
bool requestMusicLibraryPermissions();

/// Requests RECORD_AUDIO, for recording pad samples. Blocks like
/// requestMusicLibraryPermissions().
bool requestMicrophonePermission();

/// True if MANAGE_EXTERNAL_STORAGE (API >= 30) or WRITE_EXTERNAL_STORAGE is granted.
bool hasFullExternalStorageAccess();

/// Opens the system settings for "All files access". No-op if already granted.
/// Asynchronous: result visible only after the user returns.
void requestFullExternalStorageAccess();

/// Shared Music, Download, Podcasts and Recordings folders. Scanned on start.
QStringList sharedAudioDirectories();
/// Every folder with an audio file in the media store (system sounds excluded).
QStringList mediaStoreAudioFolders();
/// MediaStore generation (Android 11+), changes on any file add/change/removal.
QString mediaStoreState();
/// Folders of phone and app sounds (notification tones, alarms, other apps' media).
bool isSystemSoundDirectory(const QString& path);

/// Default music directory, created if needed. Falls back to app-private if
/// shared storage is inaccessible.
QString defaultMusicDirectory();

} // namespace android
} // namespace mixxx
