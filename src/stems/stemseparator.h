#pragma once

#include <QString>
#include <array>
#include <functional>
#include <memory>

#include "stems/onnxruntimeloader.h"
#include "util/types.h"

namespace mixxx {
namespace stems {

/// The separated stems, in the order of the stem files (like Native
/// Instruments): drums, bass, other, vocals
constexpr int kStemCount = 4;
/// The models separate audio at this rate
constexpr int kModelSampleRate = 44100;

enum class Model {
    /// Spleeter 4 stems with a multichannel Wiener filter: fast and light
    Spleeter,
    /// Hybrid Transformer Demucs: the best quality, slow and memory hungry
    HTDemucs,
};

/// Receives the separated audio in order, as interleaved stereo frames of
/// each stem. Returns false to stop.
using StemSink = std::function<bool(
        const std::array<const float*, kStemCount>& stems, SINT frameCount)>;

/// Receives the progress of a separation (0..1). Returns false to cancel.
using ProgressCallback = std::function<bool(double)>;

/// Separates audio into stems with an ONNX model. Not thread-safe: an
/// instance is used by one thread.
class StemSeparator {
  public:
    struct Options {
        /// The threads that run the model
        int threads = 2;
    };

    virtual ~StemSeparator() = default;

    static std::unique_ptr<StemSeparator> create(Model model);

    /// Loads the model files of modelDir. Returns false on error, see error().
    virtual bool load(const QString& modelDir, const Options& options) = 0;

    /// Separates interleaved stereo audio at kModelSampleRate. Returns false
    /// on error or if canceled, see error().
    virtual bool separate(const float* pStereo,
            SINT frameCount,
            const StemSink& sink,
            const ProgressCallback& progress) = 0;

    const QString& error() const {
        return m_error;
    }

  protected:
    /// Options for a light footprint: no memory arena, no busy waiting, low
    /// priority threads that leave the CPU to the audio and the UI
    static Ort::SessionOptions sessionOptions(const Options& options);

    QString m_error;
};

/// Moves the calling thread (and the threads it starts later) to the
/// slower cores of a big.LITTLE CPU, leaving the faster ones to the models.
/// Does nothing if all cores are alike.
void moveToEfficiencyCores();

/// Free memory of the system in MB (MemAvailable), -1 if unknown
int availableMemoryMb();

/// Total memory of the system in MB, -1 if unknown
int totalMemoryMb();

} // namespace stems
} // namespace mixxx
