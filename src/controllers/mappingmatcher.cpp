#include "controllers/mappingmatcher.h"

#include <QRegularExpression>
#include <QSet>
#include <QStringList>
#include <algorithm>

#include "controllers/controller.h"
#include "controllers/controllermappinginfo.h"

namespace mixxx {

namespace {

const QSet<QString> kGenericNameTokens = {
        QStringLiteral("midi"),
        QStringLiteral("usb"),
        QStringLiteral("hid"),
        QStringLiteral("port"),
        QStringLiteral("in"),
        QStringLiteral("out"),
        QStringLiteral("dj"),
        QStringLiteral("controller"),
        QStringLiteral("audio"),
        QStringLiteral("device"),
        QStringLiteral("corporation"),
        QStringLiteral("inc"),
        QStringLiteral("ltd"),
};

QStringList significantNameTokens(QString name) {
    // Strip port numbers like in "DDJ-400 MIDI 1"
    static const QRegularExpression kPortSuffix(
            QStringLiteral("\\b(midi|port)\\s*\\d+\\s*$"),
            QRegularExpression::CaseInsensitiveOption);
    static const QRegularExpression kSeparators(QStringLiteral("[^a-z0-9]+"));
    name.remove(kPortSuffix);
    QStringList tokens = name.toLower().split(kSeparators, Qt::SkipEmptyParts);
    tokens.removeIf([](const QString& token) {
        return kGenericNameTokens.contains(token);
    });
    return tokens;
}

bool isModelToken(const QString& token) {
    return std::any_of(token.cbegin(), token.cend(), [](QChar c) { return c.isDigit(); });
}

} // namespace

int mappingNameMatchScore(const QString& deviceName, const QString& mappingName) {
    const QStringList deviceTokens = significantNameTokens(deviceName);
    const QStringList mappingTokens = significantNameTokens(mappingName);
    if (deviceTokens.isEmpty() || mappingTokens.isEmpty()) {
        return 0;
    }
    const QString compactDevice = deviceTokens.join(QString());
    const QString compactMapping = mappingTokens.join(QString());
    if (compactDevice == compactMapping) {
        return 1000;
    }

    // A model number of the mapping must appear in the device name
    for (const QString& token : mappingTokens) {
        if (isModelToken(token) && !deviceTokens.contains(token) &&
                !compactDevice.contains(token)) {
            return 0;
        }
    }
    // Handles spelling differences like "DDJ400" vs "Pioneer DDJ-400"
    const bool compactDeviceIsNumber = std::all_of(compactDevice.cbegin(),
            compactDevice.cend(),
            [](QChar c) { return c.isDigit(); });
    if (compactMapping.contains(compactDevice) && isModelToken(compactDevice) &&
            !compactDeviceIsNumber) {
        return static_cast<int>(compactDevice.size());
    }

    int score = 0;
    int matchedTokens = 0;
    for (const QString& token : deviceTokens) {
        if (mappingTokens.contains(token)) {
            score += static_cast<int>(token.size());
            ++matchedTokens;
        } else if (isModelToken(token)) {
            // The device has a model number the mapping doesn't mention
            return 0;
        }
    }
    // A model number alone is not enough ("DDJ-400" vs "Vestax VCI-400")
    if (matchedTokens < 2) {
        return 0;
    }
    return score;
}

const MappingInfo* bestMatchingMapping(
        Controller* pController, const QList<MappingInfo>& mappings) {
    const MappingInfo* pBestMatch = nullptr;
    int bestScore = 0;
    for (const MappingInfo& mapping : mappings) {
        const int score = pController->matchMapping(mapping)
                ? 10000
                : mappingNameMatchScore(pController->getName(), mapping.getName());
        if (score <= 0) {
            continue;
        }
        if (score > bestScore ||
                (score == bestScore &&
                        mapping.getName().size() < pBestMatch->getName().size())) {
            bestScore = score;
            pBestMatch = &mapping;
        }
    }
    return pBestMatch;
}

} // namespace mixxx
