#pragma once

#include <QObject>
#include <QQmlEngine>
#include <QQmlParserStatus>
#include <QTimer>
#include <memory>

#include "control/controlproxy.h"

namespace mixxx {
namespace qml {

class QmlControlProxy : public QObject, public QQmlParserStatus {
    Q_OBJECT
    Q_INTERFACES(QQmlParserStatus)
    // TODO: The REQUIRED flag only exists in Qt 5.14 and later. Once we
    // require that as minimum dependency, add it to the group and key
    // properties.
    Q_PROPERTY(QString group READ getGroup WRITE setGroup NOTIFY groupChanged REQUIRED)
    Q_PROPERTY(QString key READ getKey WRITE setKey NOTIFY keyChanged REQUIRED)
    Q_PROPERTY(bool keyValid READ isKeyValid NOTIFY keyValidChanged)
    Q_PROPERTY(bool initialized READ isInitialized NOTIFY initializedChanged)
    Q_PROPERTY(double value READ getValue WRITE setValue NOTIFY valueChanged)
    Q_PROPERTY(double parameter READ getParameter WRITE setParameter NOTIFY parameterChanged)
    /// Mixxx Android: the minimum time in milliseconds between two
    /// valueChanged/parameterChanged signals, 0 (the default) for every
    /// change. For controls that change with every audio buffer, e.g.
    /// playposition (up to some 250 times a second), whose texts and
    /// markers don't need more than the frame rate: each signal re-evaluates
    /// the bindings on the GUI thread. The last value is always delivered.
    /// Not for buttons, whose presses and releases must all arrive.
    Q_PROPERTY(int updateInterval READ updateInterval WRITE setUpdateInterval NOTIFY
                    updateIntervalChanged)
    QML_NAMED_ELEMENT(ControlProxy)

  public:
    explicit QmlControlProxy(QObject* parent = nullptr);

    /// Implementing the QQmlParserStatus interface requires overriding this
    /// method, but we don't need it.
    // Invoked after class creation, but before any properties have been set.
    void classBegin() override{};

    /// QML cannot pass arguments to C++ constructors so this class needs to
    /// rely on the QML object setting the group and key properties to
    /// initialize the ControlProxy. We want to deplay the initialization of
    /// the underlying ControlProxy until the object has been fully created and
    /// all properties (group and key in particular) have been set.  Perform
    /// some initialization here now that the object is fully created.
    void componentComplete() override;

    void setGroup(const QString& group);
    const QString& getGroup() const;

    void setKey(const QString& key);
    const QString& getKey() const;

    void setValue(double newValue);
    double getValue() const;

    void setParameter(double newValue);
    double getParameter() const;

    bool isKeyValid() const;
    bool isInitialized() const;

    int updateInterval() const {
        return m_updateInterval;
    }
    void setUpdateInterval(int updateInterval);

    /// Reset the control to the default value.
    Q_INVOKABLE void reset();
    Q_INVOKABLE void trigger();
    Q_INVOKABLE void toggle();

  signals:
    void groupChanged(const QString& group);
    void keyChanged(const QString& key);
    void keyValidChanged(bool valid);
    void initializedChanged(bool initialized);
    void valueChanged(double newValue);
    void parameterChanged(double newParameter);
    void updateIntervalChanged(int updateInterval);

  private slots:
    /// Emits both the valueChanged and parameterChanged signals
    void slotControlProxyValueChanged(double newValue);
    /// The update interval elapsed: delivers a value that changed meanwhile
    void slotUpdateTimeout();

  private:
    /// (Re-)Initializes or resets the pointer to the underlying control proxy.
    /// Called for the first time when component construction has been
    /// completed. From that moment on, it's called whenever the group or key
    /// changes.
    void reinitializeFromKey();

    ConfigKey m_coKey;

    /// Set to true in the componentComplete() method, which is called when the
    /// QML object creation is complete.
    bool m_isComponentComplete;
    std::unique_ptr<ControlProxy> m_pControlProxy;
    int m_updateInterval;
    bool m_updatePending;
    QTimer m_updateTimer;
};

} // namespace qml
} // namespace mixxx
