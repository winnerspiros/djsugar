#include "track/keys.h"

#include <QtDebug>

#include "util/assert.h"

using mixxx::track::io::key::ChromaticKey;
using mixxx::track::io::key::KeyMap;

Keys::Keys(const QByteArray* pByteArray) {
    if (nullptr != pByteArray) {
        readByteArray(*pByteArray);
    }
}

Keys::Keys(const KeyMap& keyMap)
        : m_keyMap(keyMap) {
}

QByteArray Keys::toByteArray() const {
    std::string output;
    // Protobuf 30+ marks Serialize* [[nodiscard]].
    VERIFY_OR_DEBUG_ASSERT(m_keyMap.SerializeToString(&output)) {
        return {};
    }
    return QByteArray(output.data(), static_cast<int>(output.length()));
}

const QString& Keys::getSubVersion() const {
    return m_subVersion;
}

void Keys::setSubVersion(const QString& subVersion) {
    m_subVersion = subVersion;
}

ChromaticKey Keys::getGlobalKey() const {
    return m_keyMap.global_key();
}

QString Keys::getGlobalKeyText() const {
    return QString::fromStdString(m_keyMap.global_key_text());
}

double Keys::getGlobalTuningFrequencyHz() const {
    return m_keyMap.global_tuning_frequency_hz();
}

void Keys::setGlobalTuningFrequencyHz(double tuningFrequencyHz) {
    const double clamped = tuningFrequencyHz > 0.0 ? tuningFrequencyHz : 0.0;
    m_keyMap.set_global_tuning_frequency_hz(clamped);
}

bool Keys::readByteArray(const QByteArray& byteArray) {
    if (!m_keyMap.ParseFromArray(byteArray.constData(), byteArray.size())) {
        qDebug() << "ERROR: Could not parse Keys from QByteArray of size"
                 << byteArray.size();
        return false;
    }
    return true;
}

bool operator==(const Keys& lhs, const Keys& rhs) {
    return lhs.getSubVersion() == rhs.getSubVersion() &&
            // TODO: Is there a more efficient way to compare protobuf types?
            lhs.toByteArray() == rhs.toByteArray();
}
