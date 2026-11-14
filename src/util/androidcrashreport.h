#pragma once

#include <QDir>
#include <QString>

namespace mixxx {
namespace android {

/// Returns a world-writable directory on the primary shared storage
/// (e.g. /storage/emulated/0/Mixxx/logs)
QString crashReportDirectory();

/// Writes crash-<time>.txt (library + offset) to crashReportDirectory(), then
/// the system handler runs. The addresses resolve with unstripped libraries of
/// the same build.
void installCrashReporter();

/// Copies the log of the previous run (mixxx.log.1 after the rotation) to
/// crashReportDirectory()/mixxx-previous.log
void exportPreviousLog(const QString& logDirPath);
/// Logs how the previous run ended (Android 11+ keeps crash/kill records).
void logPreviousExit(const QString& logDirPath);

} // namespace android
} // namespace mixxx
