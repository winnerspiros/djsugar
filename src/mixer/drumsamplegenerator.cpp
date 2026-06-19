#include "mixer/drumsamplegenerator.h"

#include <QDir>
#include <QFile>
#include <QtEndian>
#include <QtGlobal>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <vector>

#include "audio/types.h"
#include "util/logger.h"
#include "util/sample.h"

namespace {

constexpr double k2Pi = 6.283185307179586476925286766559;

constexpr double kNormalisationPeak = 0.90;

// Write a simple 16-bit PCM mono WAV file
bool writeWav16(const std::vector<CSAMPLE>& samples,
        const QString& filePath,
        int sampleRate) {
    QFile file(filePath);
    if (!file.open(QIODevice::WriteOnly)) {
        return false;
    }

    const int numSamples = static_cast<int>(samples.size());
    const int bytesPerSample = 2; // 16-bit
    const int dataSize = numSamples * bytesPerSample;
    const int fileSize = 36 + dataSize;

    auto writeExact = [&file](const void* data, qint64 len) {
        return file.write(reinterpret_cast<const char*>(data), len) == len;
    };

    // RIFF header
    if (!writeExact("RIFF", 4)) return false;
    qint32 riffSize = qToLittleEndian(fileSize);
    if (!writeExact(&riffSize, 4)) return false;
    if (!writeExact("WAVE", 4)) return false;

    // fmt chunk
    if (!writeExact("fmt ", 4)) return false;
    qint32 fmtSize = qToLittleEndian(16);
    if (!writeExact(&fmtSize, 4)) return false;
    qint16 audioFormat = qToLittleEndian(static_cast<qint16>(1)); // PCM
    if (!writeExact(&audioFormat, 2)) return false;
    qint16 numChannels = qToLittleEndian(static_cast<qint16>(1)); // mono
    if (!writeExact(&numChannels, 2)) return false;
    qint32 sampleRateLE = qToLittleEndian(static_cast<qint32>(sampleRate));
    if (!writeExact(&sampleRateLE, 4)) return false;
    qint32 byteRate = qToLittleEndian(static_cast<qint32>(sampleRate * bytesPerSample));
    if (!writeExact(&byteRate, 4)) return false;
    qint16 blockAlign = qToLittleEndian(static_cast<qint16>(bytesPerSample));
    if (!writeExact(&blockAlign, 2)) return false;
    qint16 bitsPerSample = qToLittleEndian(static_cast<qint16>(16));
    if (!writeExact(&bitsPerSample, 2)) return false;

    // data chunk
    if (!writeExact("data", 4)) return false;
    qint32 dataSizeLE = qToLittleEndian(static_cast<qint32>(dataSize));
    if (!writeExact(&dataSizeLE, 4)) return false;

    // Write samples as 16-bit PCM
    for (const CSAMPLE& s : samples) {
        double clamped = std::max(-1.0, std::min(1.0, static_cast<double>(s)));
        qint16 sample = static_cast<qint16>(clamped * 32767.0);
        sample = qToLittleEndian(sample);
        if (!writeExact(&sample, 2)) return false;
    }

    file.close();
    return true;
}

// Normalize a sample vector to kNormalisationPeak
void normalize(std::vector<CSAMPLE>& samples) {
    CSAMPLE peak = 0;
    for (const auto& s : samples) {
        peak = std::max(peak, std::abs(s));
    }
    if (peak > 1e-9) {
        CSAMPLE scale = static_cast<CSAMPLE>(kNormalisationPeak / peak);
        SampleUtil::applyGain(samples.data(), scale, samples.size());
    }
}

} // namespace

const QList<DrumSampleGenerator::DrumSample>& DrumSampleGenerator::sampleList() {
    static const QList<DrumSample> list = {
            {"Kick", "kick.wav"},
            {"Snare", "snare.wav"},
            {"Hi-Hat Closed", "hihat_closed.wav"},
            {"Hi-Hat Open", "hihat_open.wav"},
            {"Clap", "clap.wav"},
            {"Tom High", "tom_high.wav"},
            {"Tom Low", "tom_low.wav"},
            {"Crash", "crash.wav"},
            {"Ride", "ride.wav"},
    };
    return list;
}

QStringList DrumSampleGenerator::generateAll(const QDir& outputDir,
        mixxx::audio::SampleRate sampleRate) {
    QStringList generated;

    struct GenInfo {
        const char* name;
        const char* fileName;
        std::vector<CSAMPLE> (*generator)(mixxx::audio::SampleRate);
    };

    QList<GenInfo> generators = {
            {"Kick", "kick.wav", &generateKick},
            {"Snare", "snare.wav", &generateSnare},
            {"Hi-Hat Closed", "hihat_closed.wav", &generateHiHat},
            {"Hi-Hat Open", "hihat_open.wav", &generateHiHat},
            {"Clap", "clap.wav", &generateClap},
            {"Tom High", "tom_high.wav", &generateTom},
            {"Tom Low", "tom_low.wav", &generateTom},
            {"Crash", "crash.wav", &generateCrash},
            {"Ride", "ride.wav", &generateRide},
    };

    for (const auto& info : generators) {
        auto samples = info.generator(sampleRate);
        normalize(samples);
        QString filePath = outputDir.filePath(info.fileName);
        if (saveSample(samples, outputDir, info.fileName, sampleRate)) {
            generated << filePath;
        }
    }

    return generated;
}

bool DrumSampleGenerator::saveSample(const std::vector<CSAMPLE>& samples,
        const QDir& outputDir,
        const QString& fileName,
        mixxx::audio::SampleRate sampleRate) {
    QString filePath = outputDir.filePath(fileName);
    return writeWav16(samples, filePath, sampleRate.value());
}

// --- Individual drum sound generators ---

// Kick: fast sine sweep from 150Hz to 50Hz with exponential decay
std::vector<CSAMPLE> DrumSampleGenerator::generateKick(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.3;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double phase = 150.0 * duration * (1.0 - std::exp(-t * 80.0));
        const double env = std::exp(-t * 12.0);
        samples[i] = static_cast<CSAMPLE>(std::sin(k2Pi * phase) * env);
    }
    return samples;
}

// Snare: tone + noise burst
std::vector<CSAMPLE> DrumSampleGenerator::generateSnare(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.2;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    // Simple LCG noise
    quint32 seed = 12345;
    auto nextNoise = [&seed]() -> double {
        seed = seed * 1103515245 + 12345;
        return static_cast<double>(static_cast<qint32>(seed)) / 2147483648.0;
    };

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double toneEnv = std::exp(-t * 40.0);
        const double noiseEnv = std::exp(-t * 25.0);
        const double tone = std::sin(k2Pi * 200.0 * t) * toneEnv;
        const double noise = nextNoise() * noiseEnv;
        samples[i] = static_cast<CSAMPLE>(tone * 0.4 + noise * 0.6);
    }
    return samples;
}

// Hi-Hat: filtered noise burst
std::vector<CSAMPLE> DrumSampleGenerator::generateHiHat(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.08;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    quint32 seed = 54321;
    auto nextNoise = [&seed]() -> double {
        seed = seed * 1103515245 + 12345;
        return static_cast<double>(static_cast<qint32>(seed)) / 2147483648.0;
    };

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double env = std::exp(-t * 80.0);
        samples[i] = static_cast<CSAMPLE>(nextNoise() * env);
    }
    return samples;
}

// Clap: short noise burst with rapid decay
std::vector<CSAMPLE> DrumSampleGenerator::generateClap(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.15;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    quint32 seed = 99999;
    auto nextNoise = [&seed]() -> double {
        seed = seed * 1103515245 + 12345;
        return static_cast<double>(static_cast<qint32>(seed)) / 2147483648.0;
    };

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double env = std::exp(-t * 35.0);
        samples[i] = static_cast<CSAMPLE>(nextNoise() * env);
    }
    return samples;
}

// Tom: pitched sine sweep similar to kick but higher
std::vector<CSAMPLE> DrumSampleGenerator::generateTom(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.25;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double freq = 200.0 - 80.0 * t;
        const double env = std::exp(-t * 15.0);
        samples[i] = static_cast<CSAMPLE>(std::sin(k2Pi * freq * t) * env);
    }
    return samples;
}

// Crash: long noise burst with slow decay
std::vector<CSAMPLE> DrumSampleGenerator::generateCrash(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 1.0;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    quint32 seed = 77777;
    auto nextNoise = [&seed]() -> double {
        seed = seed * 1103515245 + 12345;
        return static_cast<double>(static_cast<qint32>(seed)) / 2147483648.0;
    };

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double env = std::exp(-t * 5.0);
        samples[i] = static_cast<CSAMPLE>(nextNoise() * env);
    }
    return samples;
}

// Ride: metallic noise with moderate decay
std::vector<CSAMPLE> DrumSampleGenerator::generateRide(mixxx::audio::SampleRate sr) {
    const int fs = sr.value();
    const double duration = 0.6;
    const int n = static_cast<int>(fs * duration);
    std::vector<CSAMPLE> samples(n);

    quint32 seed = 33333;
    auto nextNoise = [&seed]() -> double {
        seed = seed * 1103515245 + 12345;
        return static_cast<double>(static_cast<qint32>(seed)) / 2147483648.0;
    };

    for (int i = 0; i < n; ++i) {
        const double t = static_cast<double>(i) / fs;
        const double env = std::exp(-t * 6.0);
        // Add some "metallic" character with a high sine
        const double metallic = std::sin(k2Pi * 8000.0 * t) * 0.3;
        samples[i] = static_cast<CSAMPLE>((nextNoise() * 0.7 + metallic) * env);
    }
    return samples;
}
