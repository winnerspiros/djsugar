#pragma once

#include <QObject>
#include <QPointer>
#include <QQmlEngine>

#include "qml/qmllibrarysource.h"

class LibraryFeature;

namespace mixxx {
namespace qml {

/// Exposes one of the sidebar features of the library (playlists, crates,
/// folders...) The feature is shared with the library, it isn't owned.
class QmlLibraryFeatureSource : public QmlLibraryAbstractSource {
    Q_OBJECT
    Q_PROPERTY(Feature feature MEMBER m_feature)
    QML_NAMED_ELEMENT(LibraryFeatureSource)
  public:
    enum class Feature {
        Playlists,
        Crates,
        Browse,
        History,
        Recordings,
        /// Rekordbox USB drives
        Rekordbox,
        /// Engine DJ USB drives
        EngineDJ,
    };
    Q_ENUM(Feature)

    explicit QmlLibraryFeatureSource(QObject* parent = nullptr);

    LibraryFeature* internal() override;

  private:
    Feature m_feature;
    QPointer<LibraryFeature> m_pLibraryFeature;
};

} // namespace qml
} // namespace mixxx
