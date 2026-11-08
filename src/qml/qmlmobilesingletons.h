#pragma once

namespace mixxx {

class CoreServices;

namespace qml {

/// Hands the core services to the QML singletons that only exist in the
/// Android UI. Kept in one place so that coreservices.cpp only needs one
/// call each in initializeQMLSingletons() and finalize().
void registerMobileQmlSingletons(CoreServices* pCoreServices);
void unregisterMobileQmlSingletons();

} // namespace qml
} // namespace mixxx
