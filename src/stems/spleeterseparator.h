#pragma once

#include <fftw3.h>

#include <complex>
#include <condition_variable>
#include <mutex>
#include <vector>

#include "stems/stemseparator.h"

namespace mixxx {
namespace stems {

/// Spleeter 4 stems (Deezer, MIT): one model per stem estimates its
/// magnitude spectrogram, then a multichannel Wiener filter (as in
/// Spleeter's "mwf" mode, one EM iteration like norbert) builds the stems
/// from the mix. The bass of the stems is taken from the low end of the mix
/// minus the drums, which is where Spleeter is weakest.
///
/// Fast and light: the audio is processed in 11.9 s blocks, streaming the
/// stems out. Two stages overlap: while the models estimate a block, a
/// second thread filters the one before into the stems and hands them to
/// the sink.
class SpleeterSeparator : public StemSeparator {
  public:
    SpleeterSeparator();
    ~SpleeterSeparator() override;

    bool load(const QString& modelDir, const Options& options) override;
    bool separate(const float* pStereo,
            SINT frameCount,
            const StemSink& sink,
            const ProgressCallback& progress) override;

  private:
    using Complex = std::complex<float>;

    /// A block of the STFT on its way through the two stages
    struct Block {
        SINT firstFrame = 0;
        int frames = 0;
        /// The padded output sample up to which the output is final
        SINT finalPaddedSample = 0;
        /// Spectra of the mix: [channel][frame][bin]
        std::vector<Complex> spectra;
        /// Estimated magnitudes: [stem][channel][frame][model bin]
        std::vector<float> estimates;
        /// The mean estimate of a frame, used above the model bins:
        /// [stem][channel][frame]
        std::vector<float> estimateTails;
    };

    /// First stage: the spectra of a block and their magnitudes, the model
    /// input (m_modelInput)
    void analyzeBlock(const float* pStereo, SINT frameCount, Block* pBlock);
    /// First stage: runs the models on m_modelInput
    bool estimateBlock(Block* pBlock);
    /// Second stage: the blocks in order, until all are done or m_stop
    void runOutputStage(SINT frameCount, const StemSink& sink);
    /// Second stage: filters the spectra of the block into the stems and
    /// overlap-adds them to the output
    void filterBlock(const Block& block);
    void addOutputFrame(SINT frame);
    /// Second stage: hands the final output samples over to the sink
    bool emitOutput(SINT finalPaddedSample, SINT frameCount, const StemSink& sink);

    std::array<std::unique_ptr<Ort::Session>, kStemCount> m_sessions;

    // STFT of the first stage
    std::vector<float> m_window;
    float* m_pFftTime;
    fftwf_complex* m_pFftBins;
    fftwf_plan m_forwardPlan;
    // Inverse STFT of the second stage
    fftwf_complex* m_pInverseBins;
    float* m_pInverseTime;
    fftwf_plan m_inversePlan;

    /// Model input: [channel][frame][model bin]
    std::vector<float> m_modelInput;
    /// The block the first stage fills while the second stage works on the
    /// other one
    std::array<Block, 2> m_blocks;

    // Hand over between the stages
    std::mutex m_mutex;
    std::condition_variable m_condition;
    SINT m_blockCount;
    /// Blocks done by the first and by the second stage
    SINT m_blocksEstimated;
    SINT m_blocksOutput;
    /// The second stage stopped: the sink stopped, or out of memory
    bool m_outputFailed;
    QString m_outputError;
    /// Tells the second stage to stop
    bool m_stop;
    qint64 m_outputNanos;

    /// Spectra of the stems of the current frame: [stem][channel][bin]
    std::vector<Complex> m_frameStems;
    /// Overlap-add of the stems: [stem * 2 + channel][padded sample - origin]
    std::array<std::vector<float>, kStemCount * 2> m_output;
    std::vector<float> m_windowSum;
    SINT m_outputOrigin;
    SINT m_emittedFrames;
    std::array<std::vector<float>, kStemCount> m_emitBuffers;
};

} // namespace stems
} // namespace mixxx
