#pragma once

#include <QList>
#include <QString>

class Controller;
class MappingInfo;

namespace mixxx {

/// Returns how well a mapping name matches a device name, 0 meaning no
/// match. Model numbers (tokens with digits like "400" or "S4") must match
/// exactly, so "DDJ-400" never matches "DDJ-4000" or "DDJ-SB2".
int mappingNameMatchScore(const QString& deviceName, const QString& mappingName);

/// Returns the mapping of `mappings` that fits the controller best, either
/// because the mapping declares the device or by name, or nullptr.
const MappingInfo* bestMatchingMapping(
        Controller* pController, const QList<MappingInfo>& mappings);

} // namespace mixxx
