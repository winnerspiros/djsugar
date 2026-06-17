#pragma once
#include <vector>
#include <QString>
#include <QDir>

#include "audio/types.h"
#include "util/types.h"

/// Generates basic drum samples (kick, snare, hi-hat, clap, tom, crash, ride)
/// at first run and saves them to the config directory.
/// Each sample is a short synthesized sound suitable for DJing.
class DrumSampleGenerator {
  public:
    struct DrumSample {
        QString name;
        QString fileName;
    };

    static const QVector<DrumSample>& sampleList();

    /// Generate all samples into the given directory.
    /// Returns the list of successfully generated file paths.
    static QStringList generateAll(const QDir& outputDir,
            mixxx::audio::SampleRate sampleRate);

  private:
    static std::vector<CSAMPLE> generateKick(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateSnare(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateHiHat(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateClap(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateTom(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateCrash(mixxx::audio::SampleRate sr);
    static std::vector<CSAMPLE> generateRide(mixxx::audio::SampleRate sr);
    static bool saveSample(const std::vector<CSAMPLE>& samples,
            const QDir& outputDir, const QString& fileName,
            mixxx::audio::SampleRate sr);
};
