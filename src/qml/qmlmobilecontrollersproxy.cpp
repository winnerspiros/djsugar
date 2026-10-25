#include "qml/qmlmobilecontrollersproxy.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QStandardPaths>

#include "controllers/controller.h"
#include "controllers/mappingmatcher.h"
#include "moc_qmlmobilecontrollersproxy.cpp"

namespace mixxx {
namespace qml {

QmlMobileControllersProxy::QmlMobileControllersProxy(QObject* pParent)
        : QObject(pParent) {
}

// static
QmlMobileControllersProxy* QmlMobileControllersProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlMobileControllersProxy(pQmlEngine);
}

QString QmlMobileControllersProxy::mappingsDirectory() const {
#if defined(Q_OS_ANDROID)
    return QStringLiteral("/storage/emulated/0/Mixxx/controllers/");
#else
    return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation) +
            QStringLiteral("/controllers/");
#endif
}

void QmlMobileControllersProxy::copyToClipboard(const QString& text) const {
    if (auto* pClipboard = QGuiApplication::clipboard()) {
        pClipboard->setText(text);
    }
}

QmlControllerMappingProxy* QmlMobileControllersProxy::suggestedMapping(
        QmlControllerDeviceProxy* pDevice,
        const QmlControllerMappingProxyList& mappings) const {
    if (!pDevice) {
        return nullptr;
    }
    QmlControllerMappingProxy* pBestMatch = nullptr;
    int bestScore = 0;
    for (auto* pMapping : mappings) {
        const int score = mixxx::mappingNameMatchScore(
                pDevice->getName(), pMapping->getName());
        if (score <= 0) {
            continue;
        }
        if (score > bestScore ||
                (score == bestScore &&
                        pMapping->getName().size() < pBestMatch->getName().size())) {
            bestScore = score;
            pBestMatch = pMapping;
        }
    }
    return pBestMatch;
}

} // namespace qml
} // namespace mixxx
