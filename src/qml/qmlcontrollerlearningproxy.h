#pragma once

#include <QObject>
#include <QPair>
#include <QPointer>
#include <QQmlEngine>
#include <QStringList>
#include <QTimer>

#include "controllers/midi/midimessage.h"

class MidiController;

namespace mixxx {
namespace qml {

/// Makes a mapping by learning (Android), as the `Mixxx.ControllerLearning`
/// singleton: choose a function of Mixxx, move or press the control on the
/// controller, and the MIDI messages it sends are turned into a mapping
/// (LearningUtils guesses buttons, knobs, faders and encoders). The learned
/// controls work at once; save() writes them, with the mapping the
/// controller had, into a mapping file of its own.
class QmlControllerLearningProxy : public QObject {
    Q_OBJECT
    /// The MIDI controllers that are open
    Q_PROPERTY(QStringList controllers READ controllers NOTIFY controllersChanged)
    Q_PROPERTY(bool active READ active NOTIFY activeChanged)
    Q_PROPERTY(bool listening READ listening NOTIFY listeningChanged)
    /// The last message received, e.g. "0x90 0x0B 0x7F"
    Q_PROPERTY(QString lastMessage READ lastMessage NOTIFY lastMessageChanged)
    /// The functions learned in this session, as "group,key"
    Q_PROPERTY(QStringList learned READ learned NOTIFY learnedChanged)
    QML_NAMED_ELEMENT(ControllerLearning)
    QML_SINGLETON

  public:
    explicit QmlControllerLearningProxy(QObject* pParent = nullptr);
    ~QmlControllerLearningProxy() override;

    static QmlControllerLearningProxy* create(QQmlEngine* pQmlEngine, QJSEngine* pJsEngine);

    QStringList controllers() const;
    bool active() const {
        return !m_pController.isNull();
    }
    bool listening() const {
        return m_listening;
    }
    QString lastMessage() const {
        return m_lastMessage;
    }
    QStringList learned() const;

    /// Starts learning on a controller of `controllers`
    Q_INVOKABLE bool start(int index);
    /// Listens for the control of a function of Mixxx, e.g. "[Channel1]",
    /// "play"
    Q_INVOKABLE void listen(const QString& group, const QString& key);
    /// Stops listening without learning
    Q_INVOKABLE void stopListening();
    /// Saves the mapping as Mixxx/controllers/<name>.midi.xml and uses it.
    /// Returns the file, or an empty string on error.
    Q_INVOKABLE QString save(const QString& name);
    /// Stops learning, forgetting what wasn't saved
    Q_INVOKABLE void cancel();

  signals:
    void controllersChanged();
    void activeChanged();
    void listeningChanged();
    void lastMessageChanged();
    void learnedChanged();
    /// A function was learned (description: e.g. "button, note 0x0B")
    void controlLearned(const QString& group, const QString& key, const QString& description);
    /// Nothing usable was received for a function
    void controlNotLearned(const QString& group, const QString& key);

    // To the controller thread
    void startLearningRequested();
    void stopLearningRequested();
    void learnRequested(const MidiInputMappings& mappings);
    void clearRequested();

  private slots:
    void slotMessageReceived(unsigned char status, unsigned char control, unsigned char value);
    void slotSettled();

  private:
    void connectController(MidiController* pController);
    void disconnectController();

    QPointer<MidiController> m_pController;
    bool m_listening;
    ConfigKey m_listenKey;
    QList<QPair<MidiKey, unsigned char>> m_messages;
    QTimer m_settleTimer;
    QString m_lastMessage;
    /// What was learned in this session, for save()
    MidiInputMappings m_learnedMappings;
    QList<ConfigKey> m_learnedKeys;
};

} // namespace qml
} // namespace mixxx
