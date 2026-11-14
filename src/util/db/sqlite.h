#pragma once

#include <QDateTime>
#include <QSqlDatabase>
#include <QVariant>

namespace mixxx {
namespace sqlite {

QDateTime readGeneratedTimestamp(const QVariant& value);

QVariant writeGeneratedTimestamp(const QDateTime& value);

/// Runs SQLite ANALYZE to update query planner statistics.
/// Should be called after schema migrations that add indexes.
void updateQueryPlannerStatistics(const QSqlDatabase& database);

} // namespace sqlite

} // namespace mixxx
