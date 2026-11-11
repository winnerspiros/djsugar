#include "qml/qmlcontrollerlearningproxy.h"

#include <QDir>
#include <QRegularExpression>

#include "controllers/controllermanager.h"
#include "controllers/learningutils.h"
#include "controllers/midi/legacymidicontrollermapping.h"
#include "controllers/midi/midicontroller.h"
#include "controllers/midi/midiutils.h"
#include "moc_qmlcontrollerlearningproxy.cpp"
#include "qml/qmlpreferencesproxy.h"

namespace mixxx {
namespace qml {

namespace {

// A control is learned once its messages stop for this long (a knob turned
// sends many)
constexpr int kSettleMillis = 800;

QList<MidiController*> openMidiControllers() {
    QList<MidiController*> result;
    const auto pManager = QmlControllerManagerProxy::manager();
    if (!pManager) {
        return result;
    }
    const auto controllers = pManager->getControllers();
    for (Controller* pController : controllers) {
        auto* pMidi = qobject_cast<MidiController*>(pController);
        if (pMidi && pMidi->isOpen()) {
            result.append(pMidi);
        }
    }
    return result;
}

QString describe(const MidiInputMappings& mappings) {
    QStringList parts;
    for (const MidiInputMapping& mapping : mappings) {
        const MidiOpCode opCode = MidiUtils::opCodeFromStatus(mapping.key.status);
        const QString number = QStringLiteral("0x%1").arg(
                QString::number(mapping.key.control, 16).toUpper().rightJustified(2, '0'));
        QString kind;
        switch (opCode) {
        case MidiOpCode::NoteOn:
        case MidiOpCode::NoteOff:
            kind = QStringLiteral("note %1").arg(number);
            break;
        case MidiOpCode::ControlChange:
            kind = QStringLiteral("CC %1").arg(number);
            break;
        case MidiOpCode::PitchBendChange:
            kind = QStringLiteral("pitch bend");
            break;
        default:
            kind = number;
            break;
        }
        parts.append(QStringLiteral("%1, ch %2")
                             .arg(kind)
                             .arg(MidiUtils::channelFromStatus(mapping.key.status) + 1));
    }
    parts.removeDuplicates();
    return parts.join(QStringLiteral(" · "));
}

} // namespace

QmlControllerLearningProxy::QmlControllerLearningProxy(QObject* pParent)
        : QObject(pParent),
          m_listening(false) {
    m_settleTimer.setSingleShot(true);
    m_settleTimer.setInterval(kSettleMillis);
    connect(&m_settleTimer, &QTimer::timeout, this, &QmlControllerLearningProxy::slotSettled);
    if (const auto pManager = QmlControllerManagerProxy::manager()) {
        connect(pManager.get(),
                &ControllerManager::devicesChanged,
                this,
                &QmlControllerLearningProxy::controllersChanged);
    }
}

QmlControllerLearningProxy::~QmlControllerLearningProxy() {
    disconnectController();
}

// static
QmlControllerLearningProxy* QmlControllerLearningProxy::create(QQmlEngine* pQmlEngine, QJSEngine*) {
    return new QmlControllerLearningProxy(pQmlEngine);
}

QStringList QmlControllerLearningProxy::controllers() const {
    QStringList names;
    for (MidiController* pController : openMidiControllers()) {
        names.append(pController->getName());
    }
    return names;
}

QStringList QmlControllerLearningProxy::learned() const {
    QStringList result;
    for (const ConfigKey& key : m_learnedKeys) {
        result.append(key.group + QLatin1Char(',') + key.item);
    }
    return result;
}

void QmlControllerLearningProxy::connectController(MidiController* pController) {
    m_pController = pController;
    connect(pController,
            &MidiController::messageReceived,
            this,
            &QmlControllerLearningProxy::slotMessageReceived);
    // On the controller thread, like DlgControllerLearning
    connect(this,
            &QmlControllerLearningProxy::startLearningRequested,
            pController,
            &MidiController::startLearning);
    connect(this,
            &QmlControllerLearningProxy::stopLearningRequested,
            pController,
            &MidiController::stopLearning);
    connect(this,
            &QmlControllerLearningProxy::learnRequested,
            pController,
            &MidiController::learnTemporaryInputMappings);
    connect(this,
            &QmlControllerLearningProxy::clearRequested,
            pController,
            &MidiController::clearTemporaryInputMappings);
    emit startLearningRequested();
}

void QmlControllerLearningProxy::disconnectController() {
    if (!m_pController) {
        return;
    }
    emit clearRequested();
    emit stopLearningRequested();
    disconnect(m_pController, nullptr, this, nullptr);
    disconnect(this, nullptr, m_pController, nullptr);
    m_pController.clear();
}

bool QmlControllerLearningProxy::start(int index) {
    const auto controllers = openMidiControllers();
    if (index < 0 || index >= controllers.size()) {
        return false;
    }
    cancel();
    connectController(controllers[index]);
    emit activeChanged();
    return true;
}

void QmlControllerLearningProxy::listen(const QString& group, const QString& key) {
    if (!m_pController) {
        return;
    }
    m_listenKey = ConfigKey(group, key);
    m_messages.clear();
    m_settleTimer.stop();
    m_listening = true;
    emit listeningChanged();
}

void QmlControllerLearningProxy::stopListening() {
    m_settleTimer.stop();
    m_messages.clear();
    if (m_listening) {
        m_listening = false;
        emit listeningChanged();
    }
}

void QmlControllerLearningProxy::slotMessageReceived(
        unsigned char status, unsigned char control, unsigned char value) {
    const MidiOpCode opCode = MidiUtils::opCodeFromStatus(status);
    m_lastMessage = MidiUtils::isMessageTwoBytes(opCode)
            ? QStringLiteral("0x%1 0x%2 0x%3")
                      .arg(QString::number(status, 16).toUpper(),
                              QString::number(control, 16).toUpper().rightJustified(2, '0'),
                              QString::number(value, 16).toUpper().rightJustified(2, '0'))
            : QStringLiteral("0x%1 0x%2")
                      .arg(QString::number(status, 16).toUpper(),
                              QString::number(control, 16).toUpper().rightJustified(2, '0'));
    emit lastMessageChanged();
    if (!m_listening) {
        return;
    }
    m_messages.append(qMakePair(MidiKey(status, control), value));
    m_settleTimer.start();
}

void QmlControllerLearningProxy::slotSettled() {
    if (!m_listening || m_messages.isEmpty()) {
        return;
    }
    const ConfigKey key = m_listenKey;
    const MidiInputMappings mappings = LearningUtils::guessMidiInputMappings(key, m_messages);
    m_listening = false;
    m_messages.clear();
    emit listeningChanged();
    if (mappings.isEmpty()) {
        emit controlNotLearned(key.group, key.item);
        return;
    }
    // A function learned again replaces what it had
    for (int i = m_learnedMappings.size() - 1; i >= 0; --i) {
        const auto& existing = m_learnedMappings[i];
        if (std::holds_alternative<ConfigKey>(existing.control) &&
                std::get<ConfigKey>(existing.control) == key) {
            m_learnedMappings.removeAt(i);
        }
    }
    m_learnedMappings.append(mappings);
    m_learnedKeys.removeAll(key);
    m_learnedKeys.append(key);
    emit learnRequested(mappings);
    emit learnedChanged();
    emit controlLearned(key.group, key.item, describe(mappings));
}

QString QmlControllerLearningProxy::save(const QString& name) {
    if (!m_pController || m_learnedMappings.isEmpty()) {
        return QString();
    }
    // The mapping the controller had, and what was learned over it
    auto pMapping = std::make_shared<LegacyMidiControllerMapping>();
    if (const auto pCurrent = m_pController->getMapping()) {
        if (const auto* pMidi = dynamic_cast<const LegacyMidiControllerMapping*>(pCurrent.get())) {
            pMapping = std::make_shared<LegacyMidiControllerMapping>(*pMidi);
        }
    }
    for (const MidiInputMapping& mapping : std::as_const(m_learnedMappings)) {
        pMapping->removeInputMapping(mapping.key.key);
    }
    for (const MidiInputMapping& mapping : std::as_const(m_learnedMappings)) {
        pMapping->addInputMapping(mapping.key.key, mapping);
    }
    const QString cleanName = name.trimmed().isEmpty() ? m_pController->getName() : name.trimmed();
    pMapping->setName(cleanName);
    pMapping->setAuthor(QStringLiteral("Mixxx Android (learned)"));
    pMapping->setDescription(QStringLiteral("Made with Learn controller on Android for %1")
                                     .arg(m_pController->getName()));

    const QString directory = QStringLiteral("/storage/emulated/0/Mixxx/controllers/");
    QDir().mkpath(directory);
    QString fileName = cleanName;
    fileName.replace(QRegularExpression(QStringLiteral("[^A-Za-z0-9 _.-]")), QStringLiteral("_"));
    const QString path = directory + fileName + QStringLiteral(".midi.xml");
    pMapping->setFilePath(path);
    if (!pMapping->saveMapping(path)) {
        return QString();
    }

    // Used from now on (and remembered for this controller)
    if (const auto pManager = QmlControllerManagerProxy::manager()) {
        MidiController* pController = m_pController;
        QMetaObject::invokeMethod(
                pManager.get(),
                [pManager, pController, pMapping]() {
                    pManager->slotApplyMapping(pController, pMapping, true);
                },
                Qt::QueuedConnection);
    }
    m_learnedMappings.clear();
    m_learnedKeys.clear();
    emit learnedChanged();
    disconnectController();
    emit activeChanged();
    return path;
}

void QmlControllerLearningProxy::cancel() {
    stopListening();
    m_learnedMappings.clear();
    m_learnedKeys.clear();
    emit learnedChanged();
    const bool wasActive = active();
    disconnectController();
    if (wasActive) {
        emit activeChanged();
    }
}

} // namespace qml
} // namespace mixxx
