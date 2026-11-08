#pragma once

#include "util/db/dbconnectionpool.h"

class LibraryScanner;

namespace mixxx {
namespace android {

/// Orders the tracks of each library scan by the age of their files.
///
/// The scanner adds the files alphabetically, folder by folder, and each
/// track gets the time it was added as "date added". After a scan, the
/// "date added" values of the tracks that scan added are handed out again in
/// the order of the file dates: the oldest file gets the earliest value. All
/// values stay times of that scan, so sorting by "date added" lists each
/// scan's files from the oldest to the newest, after the tracks of earlier
/// scans. Tracks that were already in the library are not changed.
///
/// Must be called before anything else is connected to the scanner's
/// scanFinished() signal, so the dates are changed before the track lists
/// are refreshed.
void connectScanOrder(
        LibraryScanner* pScanner,
        const mixxx::DbConnectionPoolPtr& pDbConnectionPool);

} // namespace android
} // namespace mixxx
