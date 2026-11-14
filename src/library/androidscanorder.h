#pragma once

#include "util/db/dbconnectionpool.h"

class LibraryScanner;

namespace mixxx {
namespace android {

/// Orders the tracks of each scan by file age.
/// After a scan, "date added" values are reassigned in file-date order:
/// oldest file gets the earliest value. Sorting by "date added" lists
/// each scan's files oldest-to-newest, after earlier scans.
/// Must connect before anything else receives scanFinished().
void connectScanOrder(
        LibraryScanner* pScanner,
        const mixxx::DbConnectionPoolPtr& pDbConnectionPool);

} // namespace android
} // namespace mixxx
