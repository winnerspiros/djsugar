#pragma once

#include <QString>
#include <array>
#include <functional>
#include <memory>

#include "stems/onnxruntimeloader.h"
#include "util/types.h"

namespace mixxx {
namespace stems {

/// Stem order (Native Instruments style): drums, bass, other, vocals
constexpr int kStemCount = 4;
/// The models separate audio at this rate
constexpr int kModelSampleRate = 44100;

enum class Model {
    /// Spleeter 4 stems with a multichannel Wiener filter: fast and light
    Spleeter,
    /// Hybrid Transformer Demucs: the best quality, slow and memory hungry
    HTDemucs,
};

/// Receives separated audio in order (interleaved stereo). Returns false to stop.
using StemSink = std::function<bool(
        const std::array<const float*, kStemCount>& stems, SINT frameCount)>;

/// Progress callback (0..1). Returns false to cancel.
using ProgressCallback = std::function<bool(double)>;

/// Separates audio into stems with an ONNX model. Not thread-safe.
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
    /// Light footprint: no arena/busy-wait, low-priority threads
    static Ort::SessionOptions sessionOptions(const Options& options);

    QString m_error;
};

/// Moves to slow cores (big.LITTLE), leaving fast cores for the models.
void moveToEfficiencyCores();

/// Free memory of the system in MB (MemAvailable), -1 if unknown
int availableMemoryMb();

/// Total memory of the system in MB, -1 if unknown
int totalMemoryMb();

} // namespace stems
} // namespace mixxx
