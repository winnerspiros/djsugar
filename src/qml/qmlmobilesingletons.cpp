#include "qml/qmlmobilesingletons.h"

#include "coreservices.h"
#include "library/rekordbox/rekordboxandroid.h"
#include "library/trackcollection.h"
#include "library/trackcollectionmanager.h"
#include "qml/qmlstreamingproxy.h"
#include "track/androidautocue.h"

namespace mixxx {
namespace qml {

void registerMobileQmlSingletons(CoreServices* pCoreServices) {
    QmlStreamingProxy::registerPlayerManager(pCoreServices->getPlayerManager());
    QmlStreamingProxy::registerUserSettings(pCoreServices->getSettings());
#ifdef Q_OS_ANDROID
    // Before the automatic cue, which has to see the beatgrid of Rekordbox
    android::applyRekordboxDataOnLoad(pCoreServices->getPlayerManager().get(),
            pCoreServices->getTrackCollectionManager()->internalCollection()->database());
    android::snapAutoCuesOnLoad(pCoreServices->getPlayerManager().get());
#endif
}

void unregisterMobileQmlSingletons() {
    QmlStreamingProxy::registerPlayerManager(nullptr);
    QmlStreamingProxy::registerUserSettings(nullptr);
}

} // namespace qml
} // namespace mixxx
