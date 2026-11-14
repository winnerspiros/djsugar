#pragma once

#include <QString>
#include <QStringList>

#include "preferences/usersettings.h"

namespace mixxx {
namespace samples {

/// Writable folder for shipped samples (copied from Android assets; empty if none).
QString bundledSamplesDir(const UserSettingsPointer& pConfig);

/// Cache dir for bundled samples (ends with slash).
QString bundledSamplesCacheDir();

/// True for pad sounds (not tracks; kept out of the track library).
bool isBundledSample(const QString& path);

/// Paths of all bundled samples, sorted by relative path.
QStringList bundledSampleFiles(const UserSettingsPointer& pConfig);

/// Writable user samples folder (kept apart from bundled cache).
QString userSamplesDir();

/// User's sample files, newest first.
QStringList userSampleFiles();

/// True for files decoders can open (any supported format).
bool isSupportedSample(const QString& fileName);

/// Default sampler kit (rekordbox style): drums on A (1-8), FX on B (9-16).
QStringList defaultSamplerKit(const UserSettingsPointer& pConfig);

} // namespace samples
} // namespace mixxx
