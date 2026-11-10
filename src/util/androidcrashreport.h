#pragma once

#include <QDir>
#include <QString>

namespace mixxx {
namespace android {

/// Returns a world-writable directory on the primary shared storage
/// (e.g. /storage/emulated/0/Mixxx/logs)
QString crashReportDirectory();

/// Writes a short report with the call stack of the crashing thread to
/// crashReportDirectory() when Mixxx crashes (crash-<time>.txt). The
/// addresses are given as library + offset, which can be resolved with the
/// unstripped libraries of the same build. The system's crash handler runs
/// afterwards as before.
void installCrashReporter();

/// Copies the log of the previous run (mixxx.log.1 after the rotation) to
/// crashReportDirectory()/mixxx-previous.log
void exportPreviousLog(const QString& logDirPath);
/// Logs how the previous run ended when it crashed or was killed (Android
/// 11+ keeps a record of that, also of crashes no handler could write down)
void logPreviousExit();

} // namespace android
} // namespace mixxx
