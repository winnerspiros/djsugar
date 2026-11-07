#pragma once

#include <vector>

#include "stems/stemseparator.h"

namespace mixxx {
namespace stems {

/// Hybrid Transformer Demucs (Meta, MIT), quantized to 8 bits: the best
/// quality, but several times slower than Spleeter and it needs about 2.5 GB
/// of memory. Separates 7.8 s segments with 25% overlap, like demucs.apply.
class HTDemucsSeparator : public StemSeparator {
  public:
    bool load(const QString& modelDir, const Options& options) override;
    bool separate(const float* pStereo,
            SINT frameCount,
            const StemSink& sink,
            const ProgressCallback& progress) override;

  private:
    std::unique_ptr<Ort::Session> m_pSession;
};

} // namespace stems
} // namespace mixxx
