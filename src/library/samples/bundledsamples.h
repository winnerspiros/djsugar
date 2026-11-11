#pragma once

#include <QString>
#include <QStringList>
#include <QUrl>

#include "preferences/usersettings.h"

namespace mixxx {
namespace samples {

/// Writable folder holding the samples shipped with DJ Sugar. On first use
/// (and after an update that adds samples) the bundled files are copied
/// there, because Android assets can't be opened by the audio decoder.
/// Returns an empty string when no bundled samples exist.
QString bundledSamplesDir(const UserSettingsPointer& pConfig);

/// Folder the bundled samples are copied to (ends with a slash).
QString bundledSamplesCacheDir();

/// True for files inside bundledSamplesDir(). These are kept out of the
/// track library: they are pad sounds, not tracks.
bool isBundledSample(const QString& path);

/// Absolute paths of all bundled samples, sorted by their path relative to
/// bundledSamplesDir() (so "djsugar/01_kick.wav" sorts before "kick.ogg").
QStringList bundledSampleFiles(const UserSettingsPointer& pConfig);

/// Writable folder for samples the user imported or recorded (ends with a
/// slash). Kept apart from the bundled cache so app updates never touch it.
QString userSamplesDir();

/// Absolute paths of the user's samples, newest first.
QStringList userSampleFiles();

/// True for files the audio decoders can open, any format they support.
bool isSupportedSample(const QString& fileName);

/// Copies an audio file (local path or Android content:// URL) into
/// userSamplesDir(). Returns the new path, or an empty string if the file
/// can't be read or decoded.
QString importSample(const QUrl& url);

/// The kit loaded into samplers 1-16 when there is no saved sampler bank,
/// rekordbox style: drums on bank A (1-8), FX/one-shots on bank B (9-16).
QStringList defaultSamplerKit(const UserSettingsPointer& pConfig);

} // namespace samples
} // namespace mixxx
