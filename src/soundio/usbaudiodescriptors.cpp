#include "soundio/usbaudiodescriptors.h"

#include <algorithm>
#include <cstdlib>
#include <sstream>

namespace mixxx {
namespace usbaudio {

namespace {

constexpr uint8_t kDescriptorInterface = 0x04;
constexpr uint8_t kDescriptorEndpoint = 0x05;
constexpr uint8_t kCsInterface = 0x24;
constexpr uint8_t kCsEndpoint = 0x25;

constexpr uint8_t kClassAudio = 0x01;
constexpr uint8_t kSubclassAudioControl = 0x01;
constexpr uint8_t kSubclassAudioStreaming = 0x02;
constexpr uint8_t kSubclassMidiStreaming = 0x03;
constexpr uint8_t kProtocolUac2 = 0x20;

// Audio control interface descriptor subtypes (UAC2)
constexpr uint8_t kAcInputTerminal = 0x02;
constexpr uint8_t kAcOutputTerminal = 0x03;
constexpr uint8_t kAcClockSource = 0x0A;
constexpr uint8_t kAcClockSelector = 0x0B;
constexpr uint8_t kAcClockMultiplier = 0x0C;

// Audio streaming interface descriptor subtypes
constexpr uint8_t kAsGeneral = 0x01;
constexpr uint8_t kAsFormatType = 0x02;
constexpr uint8_t kFormatTypeI = 0x01;
constexpr uint16_t kFormatTagPcm = 0x0001;
constexpr uint8_t kEpGeneral = 0x01;

constexpr uint8_t kTransferTypeIsochronous = 0x01;
constexpr uint8_t kTransferTypeBulk = 0x02;
constexpr int kUsageData = 0;
constexpr int kUsageFeedback = 1;

int rate24(const uint8_t* p) {
    return p[0] | (p[1] << 8) | (p[2] << 16);
}

} // namespace

bool StreamFormat::supportsRate(int rate) const {
    if (std::find(sampleRates.begin(), sampleRates.end(), rate) != sampleRates.end()) {
        return true;
    }
    return minRate > 0 && rate >= minRate && rate <= maxRate;
}

int ControlInterface::clockSourceForTerminal(int terminalId) const {
    auto terminal = terminalClocks.find(terminalId);
    if (terminal == terminalClocks.end()) {
        return -1;
    }
    int clockId = terminal->second;
    // Follow selectors (first input) and multipliers, guarding against loops
    for (int depth = 0; depth < 8; ++depth) {
        auto clock = clocks.find(clockId);
        if (clock == clocks.end()) {
            // Unknown entity, assume it is the source
            return clockId;
        }
        if (clock->second.type == ClockEntity::Type::Source || clock->second.sources.empty()) {
            return clockId;
        }
        clockId = clock->second.sources.front();
    }
    return -1;
}

const ControlInterface* DeviceDescription::controlFor(const StreamFormat& format) const {
    const ControlInterface* pResult = nullptr;
    for (const auto& control : controls) {
        if (control.interfaceNumber < format.interfaceNumber &&
                (!pResult || control.interfaceNumber > pResult->interfaceNumber)) {
            pResult = &control;
        }
    }
    return pResult;
}

DeviceDescription parse(const uint8_t* pData, size_t length) {
    DeviceDescription description;
    int currentStream = -1;
    int currentControl = -1;
    bool inMidi = false;
    bool lastEndpointIsData = false;

    size_t pos = 0;
    while (pos + 2 <= length) {
        const uint8_t* d = pData + pos;
        const size_t descriptorLength = d[0];
        if (descriptorLength < 2 || pos + descriptorLength > length) {
            break;
        }
        const uint8_t type = d[1];

        if (type == kDescriptorInterface && descriptorLength >= 9) {
            const int number = d[2];
            const int alt = d[3];
            const uint8_t interfaceClass = d[5];
            const uint8_t subclass = d[6];
            const uint8_t protocol = d[7];
            currentStream = -1;
            currentControl = -1;
            inMidi = false;
            lastEndpointIsData = false;
            if (interfaceClass == kClassAudio && subclass == kSubclassAudioControl && alt == 0) {
                ControlInterface control;
                control.interfaceNumber = number;
                control.uacVersion = protocol == kProtocolUac2 ? 2 : 1;
                description.controls.push_back(control);
                currentControl = static_cast<int>(description.controls.size()) - 1;
            } else if (interfaceClass == kClassAudio && subclass == kSubclassAudioStreaming) {
                StreamFormat format;
                format.interfaceNumber = number;
                format.altSetting = alt;
                format.uacVersion = protocol == kProtocolUac2 ? 2 : 1;
                description.streams.push_back(format);
                currentStream = static_cast<int>(description.streams.size()) - 1;
            } else if (interfaceClass == kClassAudio && subclass == kSubclassMidiStreaming &&
                    description.midi.interfaceNumber < 0) {
                description.midi.interfaceNumber = number;
                inMidi = true;
            }
        } else if (type == kCsInterface && descriptorLength >= 3) {
            const uint8_t subtype = d[2];
            if (currentControl >= 0) {
                ControlInterface& control = description.controls[currentControl];
                if (control.uacVersion == 2) {
                    if (subtype == kAcInputTerminal && descriptorLength >= 8) {
                        control.terminalClocks[d[3]] = d[7];
                    } else if (subtype == kAcOutputTerminal && descriptorLength >= 9) {
                        control.terminalClocks[d[3]] = d[8];
                    } else if (subtype == kAcClockSource && descriptorLength >= 4) {
                        control.clocks[d[3]].type = ClockEntity::Type::Source;
                    } else if (subtype == kAcClockSelector && descriptorLength >= 5) {
                        ClockEntity& clock = control.clocks[d[3]];
                        clock.type = ClockEntity::Type::Selector;
                        const int pins = d[4];
                        for (int i = 0; i < pins && 5 + static_cast<size_t>(i) < descriptorLength;
                                ++i) {
                            clock.sources.push_back(d[5 + i]);
                        }
                    } else if (subtype == kAcClockMultiplier && descriptorLength >= 5) {
                        ClockEntity& clock = control.clocks[d[3]];
                        clock.type = ClockEntity::Type::Multiplier;
                        clock.sources.push_back(d[4]);
                    }
                }
            } else if (currentStream >= 0) {
                StreamFormat& format = description.streams[currentStream];
                if (subtype == kAsGeneral) {
                    if (format.uacVersion == 1 && descriptorLength >= 7) {
                        format.terminalLink = d[3];
                        format.isPcm = (d[5] | (d[6] << 8)) == kFormatTagPcm;
                    } else if (format.uacVersion == 2 && descriptorLength >= 16) {
                        format.terminalLink = d[3];
                        const uint32_t formats = d[6] | (d[7] << 8) | (d[8] << 16) |
                                (static_cast<uint32_t>(d[9]) << 24);
                        format.isPcm = d[5] == kFormatTypeI && (formats & 0x1);
                        format.channels = d[10];
                    }
                } else if (subtype == kAsFormatType && descriptorLength >= 4 &&
                        d[3] == kFormatTypeI) {
                    if (format.uacVersion == 1 && descriptorLength >= 8) {
                        format.channels = d[4];
                        format.subframeBytes = d[5];
                        format.bitResolution = d[6];
                        const int frequencyCount = d[7];
                        if (frequencyCount == 0 && descriptorLength >= 14) {
                            format.minRate = rate24(d + 8);
                            format.maxRate = rate24(d + 11);
                        } else {
                            for (int i = 0; i < frequencyCount &&
                                    8 + 3 * static_cast<size_t>(i + 1) <= descriptorLength;
                                    ++i) {
                                format.sampleRates.push_back(rate24(d + 8 + 3 * i));
                            }
                        }
                    } else if (format.uacVersion == 2 && descriptorLength >= 6) {
                        format.subframeBytes = d[4];
                        format.bitResolution = d[5];
                    }
                }
            }
        } else if (type == kDescriptorEndpoint && descriptorLength >= 7) {
            const uint8_t address = d[2];
            const uint8_t attributes = d[3];
            const int maxPacket = d[4] | (d[5] << 8);
            const uint8_t interval = d[6];
            const bool isIn = address & 0x80;
            lastEndpointIsData = false;
            if (currentStream >= 0 && (attributes & 0x3) == kTransferTypeIsochronous) {
                StreamFormat& format = description.streams[currentStream];
                const int usage = (attributes >> 4) & 0x3;
                // UAC1 feedback endpoints are often declared without the
                // usage bits: an IN endpoint next to an OUT data endpoint
                const bool isFeedback = usage == kUsageFeedback ||
                        (format.endpoint != 0 && !(format.endpoint & 0x80) && isIn);
                if (isFeedback) {
                    format.feedbackEndpoint = address;
                } else if (format.endpoint == 0) {
                    format.endpoint = address;
                    format.isOutput = !isIn;
                    // High bandwidth endpoints: additional transactions per microframe
                    format.maxPacketSize = (maxPacket & 0x7ff) * (1 + ((maxPacket >> 11) & 0x3));
                    format.bInterval = interval;
                    format.syncType = static_cast<SyncType>((attributes >> 2) & 0x3);
                    format.bSynchAddress = descriptorLength >= 9 ? d[8] : 0;
                    lastEndpointIsData = usage == kUsageData || usage == 2;
                }
            } else if (inMidi && (attributes & 0x3) == kTransferTypeBulk) {
                if (isIn) {
                    description.midi.inEndpoint = address;
                    description.midi.inMaxPacketSize = maxPacket & 0x7ff;
                } else {
                    description.midi.outEndpoint = address;
                }
            }
        } else if (type == kCsEndpoint && descriptorLength >= 4 && currentStream >= 0 &&
                lastEndpointIsData && d[2] == kEpGeneral) {
            StreamFormat& format = description.streams[currentStream];
            if (format.uacVersion == 1) {
                format.hasSampleRateControl = d[3] & 0x01;
            }
        }
        pos += descriptorLength;
    }
    return description;
}

const StreamFormat* chooseOutput(const DeviceDescription& description) {
    const StreamFormat* pBest = nullptr;
    for (const auto& format : description.streams) {
        if (!format.isOutput || !format.isUsable()) {
            continue;
        }
        if (format.syncType == SyncType::Asynchronous && format.feedbackEndpoint == 0 &&
                !implicitFeedbackSource(description, format)) {
            continue;
        }
        if (!pBest || format.channels > pBest->channels ||
                (format.channels == pBest->channels &&
                        format.bitResolution > pBest->bitResolution)) {
            pBest = &format;
        }
    }
    return pBest;
}

const StreamFormat* implicitFeedbackSource(
        const DeviceDescription& description, const StreamFormat& output) {
    if (output.syncType != SyncType::Asynchronous || output.feedbackEndpoint != 0) {
        return nullptr;
    }
    const StreamFormat* pResult = nullptr;
    for (const auto& format : description.streams) {
        if (format.isOutput || !format.isUsable()) {
            continue;
        }
        if (output.bSynchAddress != 0 && format.endpoint == output.bSynchAddress) {
            return &format;
        }
        if (!pResult || format.channels < pResult->channels) {
            // The capture stream with fewest channels uses the least bandwidth
            pResult = &format;
        }
    }
    return pResult;
}

int packetsPerSecond(uint8_t bInterval, bool highSpeed) {
    const int exponent = std::clamp(static_cast<int>(bInterval), 1, 16) - 1;
    return std::max(1, (highSpeed ? 8000 : 1000) >> exponent);
}

uint32_t feedbackToFramesPerPacket(uint32_t raw,
        int feedbackBytes,
        uint32_t nominalFramesPerPacket) {
    if (raw == 0 || nominalFramesPerPacket == 0) {
        return 0;
    }
    if (feedbackBytes == 3) {
        raw &= 0xffffff;
    }
    // Feedback may be in 10.14 or 16.16, per frame or per microframe, or
    // otherwise misscaled by the device. The interpretations differ by at
    // least a factor of 2, so the first one close to the nominal rate wins.
    // Shifts are tried by magnitude: 16.16, then 10.14, then the rest.
    const uint64_t tolerance = nominalFramesPerPacket / 8;
    for (int shift : {0, 2, -2, 1, -1, 3, -3, 4, -4}) {
        for (int multiplier : {1, 2, 4, 8}) {
            uint64_t candidate = shift >= 0
                    ? (static_cast<uint64_t>(raw) << shift)
                    : (static_cast<uint64_t>(raw) >> (-shift));
            candidate *= multiplier;
            const uint64_t distance = candidate > nominalFramesPerPacket
                    ? candidate - nominalFramesPerPacket
                    : nominalFramesPerPacket - candidate;
            if (distance <= tolerance) {
                return static_cast<uint32_t>(std::min<uint64_t>(candidate, UINT32_MAX));
            }
        }
    }
    return 0;
}

std::string describe(const StreamFormat& format) {
    std::ostringstream out;
    out << "UAC" << format.uacVersion << (format.isOutput ? " out" : " in") << " if "
        << format.interfaceNumber << " alt " << format.altSetting << " ep 0x" << std::hex
        << static_cast<int>(format.endpoint) << std::dec << " " << format.channels << "ch "
        << format.bitResolution << "bit/" << format.subframeBytes * 8 << " max packet "
        << format.maxPacketSize << " interval " << static_cast<int>(format.bInterval);
    switch (format.syncType) {
    case SyncType::Asynchronous:
        out << " async";
        if (format.feedbackEndpoint) {
            out << " feedback 0x" << std::hex << static_cast<int>(format.feedbackEndpoint)
                << std::dec;
        } else {
            out << " implicit feedback";
        }
        break;
    case SyncType::Adaptive:
        out << " adaptive";
        break;
    case SyncType::Synchronous:
        out << " sync";
        break;
    default:
        break;
    }
    out << " rates";
    for (int rate : format.sampleRates) {
        out << " " << rate;
    }
    if (format.minRate > 0) {
        out << " " << format.minRate << "-" << format.maxRate;
    }
    return out.str();
}

} // namespace usbaudio
} // namespace mixxx
