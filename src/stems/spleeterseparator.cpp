#include "stems/spleeterseparator.h"

#include <sys/resource.h>
#include <unistd.h>

#include <QDir>
#include <QElapsedTimer>
#include <algorithm>
#include <cmath>
#include <exception>
#include <thread>

#include "util/logger.h"

namespace mixxx {
namespace stems {

namespace {

const Logger kLogger("Spleeter");

// The STFT of Spleeter: 4096 point periodic Hann window, hop 1024, the
// first frame starts 3072 samples before the audio
constexpr int kFftSize = 4096;
constexpr int kHop = 1024;
constexpr int kBins = kFftSize / 2 + 1;
constexpr SINT kPad = kFftSize - kHop;
// The models see the bins below 11 kHz, in blocks of 512 frames (11.9 s)
constexpr int kModelBins = 1024;
constexpr int kBlockFrames = 512;

// The machine epsilon of float, which norbert uses for the Wiener filter
constexpr float kEps = 1.1920929e-07f;
const float kRegularization = std::sqrt(kEps);

// Below 86 Hz the bass stem gets everything but the drums, fading to the
// estimate of the models at 151 Hz
constexpr int kBassFullBin = 8;
constexpr int kBassEndBin = 14;

const char* const kModelFiles[kStemCount] = {
        "drums.fp16.onnx",
        "bass.fp16.onnx",
        "other.fp16.onnx",
        "vocals.fp16.onnx",
};
constexpr int kBass = 1;
constexpr int kOther = 2;
constexpr int kVocals = 3;

const char* const kInputName = "x";
const char* const kOutputName = "y";

// Like the threads of the models
constexpr int kOutputThreadNiceness = 10;

// |x| without the overflow handling of std::abs (hypot)
inline float magnitude(std::complex<float> x) {
    return std::sqrt(x.real() * x.real() + x.imag() * x.imag());
}

} // namespace

SpleeterSeparator::SpleeterSeparator()
        : m_window(kFftSize),
          m_pFftTime(fftwf_alloc_real(kFftSize)),
          m_pFftBins(fftwf_alloc_complex(kBins)),
          m_pInverseBins(fftwf_alloc_complex(kBins)),
          m_pInverseTime(fftwf_alloc_real(kFftSize)),
          m_modelInput(2 * kBlockFrames * kModelBins),
          m_blockCount(0),
          m_blocksEstimated(0),
          m_blocksOutput(0),
          m_outputFailed(false),
          m_stop(false),
          m_outputNanos(0),
          m_frameStems(kStemCount * 2 * kBins),
          m_outputOrigin(0),
          m_emittedFrames(0) {
    for (int n = 0; n < kFftSize; ++n) {
        m_window[n] = 0.5f - 0.5f * std::cos(2.0f * static_cast<float>(M_PI) * n / kFftSize);
    }
    for (auto& block : m_blocks) {
        block.spectra.resize(2 * kBlockFrames * kBins);
        block.estimates.resize(kStemCount * 2 * kBlockFrames * kModelBins);
        block.estimateTails.resize(kStemCount * 2 * kBlockFrames);
    }
    // Planned here: planning isn't thread-safe, executing different plans
    // at the same time is
    m_forwardPlan = fftwf_plan_dft_r2c_1d(kFftSize, m_pFftTime, m_pFftBins, FFTW_ESTIMATE);
    m_inversePlan = fftwf_plan_dft_c2r_1d(kFftSize, m_pInverseBins, m_pInverseTime, FFTW_ESTIMATE);
}

SpleeterSeparator::~SpleeterSeparator() {
    fftwf_destroy_plan(m_forwardPlan);
    fftwf_destroy_plan(m_inversePlan);
    fftwf_free(m_pFftTime);
    fftwf_free(m_pFftBins);
    fftwf_free(m_pInverseBins);
    fftwf_free(m_pInverseTime);
}

bool SpleeterSeparator::load(const QString& modelDir, const Options& options) {
    try {
        Ort::SessionOptions sessionOptions = StemSeparator::sessionOptions(options);
        sessionOptions.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_EXTENDED);
        for (int stem = 0; stem < kStemCount; ++stem) {
            const QByteArray path = QDir(modelDir).filePath(kModelFiles[stem]).toUtf8();
            m_sessions[stem] = std::make_unique<Ort::Session>(
                    onnxRuntimeEnv(), path.constData(), sessionOptions);
        }
    } catch (const Ort::Exception& e) {
        m_error = QString::fromUtf8(e.what());
        return false;
    }
    return true;
}

bool SpleeterSeparator::separate(const float* pStereo,
        SINT frameCount,
        const StemSink& sink,
        const ProgressCallback& progress) {
    const SINT stftFrames = (kPad + frameCount + kHop - 1) / kHop;
    const SINT blocks = (stftFrames + kBlockFrames - 1) / kBlockFrames;
    m_outputOrigin = 0;
    m_emittedFrames = 0;
    for (auto& output : m_output) {
        output.clear();
    }
    m_windowSum.clear();
    m_blockCount = blocks;
    m_blocksEstimated = 0;
    m_blocksOutput = 0;
    m_outputFailed = false;
    m_outputError.clear();
    m_stop = false;
    m_outputNanos = 0;
    // Where the time goes, logged at the end
    qint64 stftNanos = 0;
    qint64 modelNanos = 0;
    qint64 waitNanos = 0;
    QElapsedTimer timer;

    std::thread outputThread([this, frameCount, &sink]() {
        setpriority(PRIO_PROCESS, static_cast<id_t>(gettid()), kOutputThreadNiceness);
        runOutputStage(frameCount, sink);
    });
    // Stops the second stage however the first one ends
    const auto stopOutput = [this, &outputThread]() {
        {
            std::lock_guard lock(m_mutex);
            m_stop = true;
        }
        m_condition.notify_all();
        outputThread.join();
    };

    try {
        for (SINT block = 0; block < blocks; ++block) {
            Block* pBlock = &m_blocks[block % 2];
            timer.start();
            {
                // The second stage is done with the block before the last
                std::unique_lock lock(m_mutex);
                m_condition.wait(lock, [this, block]() {
                    return m_blocksOutput >= block - 1 || m_outputFailed;
                });
                if (m_outputFailed) {
                    break;
                }
            }
            waitNanos += timer.nsecsElapsed();
            pBlock->firstFrame = block * kBlockFrames;
            pBlock->frames = static_cast<int>(
                    std::min<SINT>(kBlockFrames, stftFrames - pBlock->firstFrame));
            pBlock->finalPaddedSample = block == blocks - 1
                    ? kPad + frameCount
                    : (pBlock->firstFrame + pBlock->frames) * kHop;
            timer.start();
            analyzeBlock(pStereo, frameCount, pBlock);
            stftNanos += timer.nsecsElapsed();
            timer.start();
            if (!estimateBlock(pBlock)) {
                stopOutput();
                return false;
            }
            modelNanos += timer.nsecsElapsed();
            SINT blocksOutput;
            {
                std::lock_guard lock(m_mutex);
                m_blocksEstimated = block + 1;
                blocksOutput = m_blocksOutput;
            }
            m_condition.notify_all();
            if (progress && !progress(static_cast<double>(blocksOutput) / blocks)) {
                stopOutput();
                m_error = QStringLiteral("Canceled");
                return false;
            }
        }
        timer.start();
        {
            std::unique_lock lock(m_mutex);
            m_condition.wait(lock, [this, blocks]() {
                return m_blocksOutput == blocks || m_outputFailed;
            });
        }
        waitNanos += timer.nsecsElapsed();
    } catch (...) {
        stopOutput();
        throw;
    }
    stopOutput();
    if (m_outputFailed) {
        m_error = m_outputError;
        return false;
    }
    if (progress && !progress(1.0)) {
        m_error = QStringLiteral("Canceled");
        return false;
    }
    kLogger.info() << "STFT" << stftNanos / 1000000 << "ms, models" << modelNanos / 1000000
                   << "ms, waiting for the output" << waitNanos / 1000000
                   << "ms; output stage (Wiener filter, sink)" << m_outputNanos / 1000000 << "ms";
    return true;
}

void SpleeterSeparator::analyzeBlock(const float* pStereo, SINT frameCount, Block* pBlock) {
    for (int channel = 0; channel < 2; ++channel) {
        for (int frame = 0; frame < kBlockFrames; ++frame) {
            float* pModelInput = &m_modelInput[(channel * kBlockFrames + frame) * kModelBins];
            if (frame >= pBlock->frames) {
                std::fill(pModelInput, pModelInput + kModelBins, 0.0f);
                continue;
            }
            // Frames start kPad samples before the audio
            const SINT start = (pBlock->firstFrame + frame) * kHop - kPad;
            for (int n = 0; n < kFftSize; ++n) {
                const SINT at = start + n;
                const float sample = at >= 0 && at < frameCount
                        ? pStereo[2 * at + channel]
                        : 0.0f;
                m_pFftTime[n] = sample * m_window[n];
            }
            fftwf_execute(m_forwardPlan);
            Complex* pSpectrum = &pBlock->spectra[(channel * kBlockFrames + frame) * kBins];
            for (int bin = 0; bin < kBins; ++bin) {
                pSpectrum[bin] = Complex(m_pFftBins[bin][0], m_pFftBins[bin][1]);
            }
            for (int bin = 0; bin < kModelBins; ++bin) {
                pModelInput[bin] = magnitude(pSpectrum[bin]);
            }
        }
    }
}

void SpleeterSeparator::runOutputStage(SINT frameCount, const StemSink& sink) {
    QElapsedTimer timer;
    for (SINT block = 0; block < m_blockCount; ++block) {
        {
            std::unique_lock lock(m_mutex);
            m_condition.wait(lock, [this, block]() {
                return m_blocksEstimated > block || m_stop;
            });
            if (m_stop) {
                return;
            }
        }
        timer.start();
        const Block& current = m_blocks[block % 2];
        bool done;
        QString error;
        try {
            filterBlock(current);
            done = emitOutput(current.finalPaddedSample, frameCount, sink);
            if (!done) {
                error = QStringLiteral("Canceled");
            }
        } catch (const std::exception&) {
            done = false;
            error = QStringLiteral("Low memory");
        }
        m_outputNanos += timer.nsecsElapsed();
        {
            std::lock_guard lock(m_mutex);
            if (done) {
                m_blocksOutput = block + 1;
            } else {
                m_outputFailed = true;
                m_outputError = error;
            }
        }
        m_condition.notify_all();
        if (!done) {
            return;
        }
    }
}

bool SpleeterSeparator::estimateBlock(Block* pBlock) {
    const std::array<int64_t, 4> shape = {2, 1, kBlockFrames, kModelBins};
    const auto memoryInfo = Ort::MemoryInfo::CreateCpu(OrtDeviceAllocator, OrtMemTypeDefault);
    try {
        Ort::Value input = Ort::Value::CreateTensor<float>(memoryInfo,
                m_modelInput.data(),
                m_modelInput.size(),
                shape.data(),
                shape.size());
        const size_t stemSize = m_modelInput.size();
        for (int stem = 0; stem < kStemCount; ++stem) {
            float* pEstimate = &pBlock->estimates[stem * stemSize];
            Ort::Value output = Ort::Value::CreateTensor<float>(
                    memoryInfo, pEstimate, stemSize, shape.data(), shape.size());
            m_sessions[stem]->Run(Ort::RunOptions{nullptr}, &kInputName, &input, 1, &kOutputName, &output, 1);
            for (int row = 0; row < 2 * kBlockFrames; ++row) {
                const float* pRow = pEstimate + row * kModelBins;
                float sum = 0.0f;
                for (int bin = 0; bin < kModelBins; ++bin) {
                    sum += pRow[bin];
                }
                pBlock->estimateTails[stem * 2 * kBlockFrames + row] = sum / kModelBins;
            }
        }
    } catch (const Ort::Exception& e) {
        m_error = QString::fromUtf8(e.what());
        return false;
    }
    return true;
}

void SpleeterSeparator::filterBlock(const Block& block) {
    const SINT firstFrame = block.firstFrame;
    const int frames = block.frames;
    // The estimated magnitude of a stem, the mean of the frame above the
    // model bins
    const auto estimate = [&block](int stem, int channel, int frame, int bin) {
        const int row = (stem * 2 + channel) * kBlockFrames + frame;
        return bin < kModelBins ? block.estimates[row * kModelBins + bin]
                                : block.estimateTails[row];
    };
    const Complex* pLeft = &block.spectra[0];
    const Complex* pRight = &block.spectra[kBlockFrames * kBins];

    // Multichannel Wiener filter, see norbert.wiener(v, x, iterations=1,
    // use_softmask=False). The spectra are scaled for numerical stability.
    float maxAbs = 0.0f;
    for (int frame = 0; frame < frames; ++frame) {
        for (int bin = 0; bin < kBins; ++bin) {
            maxAbs = std::max({maxAbs,
                    magnitude(pLeft[frame * kBins + bin]),
                    magnitude(pRight[frame * kBins + bin])});
        }
    }
    const float scale = std::max(1.0f, maxAbs / 10.0f);
    const float invScale = 1.0f / scale;

    // Spatial covariance of each stem: the estimates with the phase of the
    // mix, weighted by their power
    std::vector<float> r00(kStemCount * kBins, 0.0f);
    std::vector<float> r11(kStemCount * kBins, 0.0f);
    std::vector<Complex> r01(kStemCount * kBins, Complex(0.0f));
    std::vector<float> weight(kStemCount * kBins, kEps);
    for (int frame = 0; frame < frames; ++frame) {
        for (int bin = 0; bin < kBins; ++bin) {
            const Complex x0 = pLeft[frame * kBins + bin];
            const Complex x1 = pRight[frame * kBins + bin];
            const float abs0 = magnitude(x0);
            const float abs1 = magnitude(x1);
            const Complex phase0 = abs0 > 0.0f ? x0 / abs0 : Complex(1.0f);
            const Complex phase1 = abs1 > 0.0f ? x1 / abs1 : Complex(1.0f);
            const Complex crossPhase = phase0 * std::conj(phase1);
            for (int stem = 0; stem < kStemCount; ++stem) {
                const float a0 = estimate(stem, 0, frame, bin) * invScale;
                const float a1 = estimate(stem, 1, frame, bin) * invScale;
                const int at = stem * kBins + bin;
                r00[at] += a0 * a0;
                r11[at] += a1 * a1;
                r01[at] += a0 * a1 * crossPhase;
                weight[at] += 0.5f * (a0 * a0 + a1 * a1);
            }
        }
    }
    for (int at = 0; at < kStemCount * kBins; ++at) {
        r00[at] /= weight[at];
        r11[at] /= weight[at];
        r01[at] /= weight[at];
    }

    for (int frame = 0; frame < frames; ++frame) {
        for (int bin = 0; bin < kBins; ++bin) {
            const Complex x0 = pLeft[frame * kBins + bin] * invScale;
            const Complex x1 = pRight[frame * kBins + bin] * invScale;
            std::array<float, kStemCount> power;
            // The covariance of the mix model
            float c00 = kRegularization;
            float c11 = kRegularization;
            Complex c01(0.0f);
            for (int stem = 0; stem < kStemCount; ++stem) {
                const float a0 = estimate(stem, 0, frame, bin) * invScale;
                const float a1 = estimate(stem, 1, frame, bin) * invScale;
                power[stem] = 0.5f * (a0 * a0 + a1 * a1);
                const int at = stem * kBins + bin;
                c00 += power[stem] * r00[at];
                c11 += power[stem] * r11[at];
                c01 += power[stem] * r01[at];
            }
            const float invDet = 1.0f / (c00 * c11 - std::norm(c01));
            const Complex i00(c11 * invDet);
            const Complex i11(c00 * invDet);
            const Complex i01 = -c01 * invDet;
            const Complex i10 = -std::conj(c01) * invDet;
            for (int stem = 0; stem < kStemCount; ++stem) {
                // Gain: power * R * inverse(Cxx)
                const int at = stem * kBins + bin;
                const Complex r10 = std::conj(r01[at]);
                const Complex g00 = r00[at] * i00 + r01[at] * i10;
                const Complex g01 = r00[at] * i01 + r01[at] * i11;
                const Complex g10 = r10 * i00 + r11[at] * i10;
                const Complex g11 = r10 * i01 + r11[at] * i11;
                const float gain = power[stem] * scale;
                m_frameStems[(stem * 2) * kBins + bin] = gain * (g00 * x0 + g01 * x1);
                m_frameStems[(stem * 2 + 1) * kBins + bin] = gain * (g10 * x0 + g11 * x1);
            }
        }
        // The low end of the bass
        for (int bin = 0; bin < kBassEndBin; ++bin) {
            const float toBass = bin <= kBassFullBin
                    ? 1.0f
                    : static_cast<float>(kBassEndBin - bin) / (kBassEndBin - kBassFullBin);
            for (int channel = 0; channel < 2; ++channel) {
                Complex& vocals = m_frameStems[(kVocals * 2 + channel) * kBins + bin];
                Complex& other = m_frameStems[(kOther * 2 + channel) * kBins + bin];
                m_frameStems[(kBass * 2 + channel) * kBins + bin] += toBass * (vocals + other);
                vocals *= 1.0f - toBass;
                other *= 1.0f - toBass;
            }
        }
        addOutputFrame(firstFrame + frame);
    }
}

void SpleeterSeparator::addOutputFrame(SINT frame) {
    const SINT start = frame * kHop - m_outputOrigin;
    const size_t size = static_cast<size_t>(start + kFftSize);
    if (m_windowSum.size() < size) {
        m_windowSum.resize(size, 0.0f);
        for (auto& output : m_output) {
            output.resize(size, 0.0f);
        }
    }
    for (int n = 0; n < kFftSize; ++n) {
        m_windowSum[start + n] += m_window[n] * m_window[n];
    }
    // FFTW doesn't normalize the inverse transform
    constexpr float kNormalization = 1.0f / kFftSize;
    for (int stemChannel = 0; stemChannel < kStemCount * 2; ++stemChannel) {
        const Complex* pBins = &m_frameStems[stemChannel * kBins];
        for (int bin = 0; bin < kBins; ++bin) {
            m_pInverseBins[bin][0] = pBins[bin].real();
            m_pInverseBins[bin][1] = pBins[bin].imag();
        }
        fftwf_execute(m_inversePlan);
        float* pOutput = &m_output[stemChannel][start];
        for (int n = 0; n < kFftSize; ++n) {
            pOutput[n] += m_pInverseTime[n] * kNormalization * m_window[n];
        }
    }
}

bool SpleeterSeparator::emitOutput(
        SINT finalPaddedSample, SINT frameCount, const StemSink& sink) {
    const SINT begin = std::max(m_outputOrigin, kPad);
    const SINT end = std::min(finalPaddedSample, kPad + frameCount);
    if (end > begin) {
        const SINT count = end - begin;
        for (int stem = 0; stem < kStemCount; ++stem) {
            std::vector<float>& buffer = m_emitBuffers[stem];
            buffer.resize(2 * count);
            for (SINT i = 0; i < count; ++i) {
                const SINT at = begin + i - m_outputOrigin;
                const float windowSum = m_windowSum[at];
                const float norm = windowSum > 1e-8f ? 1.0f / windowSum : 0.0f;
                buffer[2 * i] = m_output[stem * 2][at] * norm;
                buffer[2 * i + 1] = m_output[stem * 2 + 1][at] * norm;
            }
        }
        if (!sink({m_emitBuffers[0].data(),
                            m_emitBuffers[1].data(),
                            m_emitBuffers[2].data(),
                            m_emitBuffers[3].data()},
                    count)) {
            return false;
        }
        m_emittedFrames += count;
    }
    // Drop the final samples
    const SINT drop = std::min<SINT>(finalPaddedSample - m_outputOrigin,
            static_cast<SINT>(m_windowSum.size()));
    if (drop > 0) {
        m_windowSum.erase(m_windowSum.begin(), m_windowSum.begin() + drop);
        for (auto& output : m_output) {
            output.erase(output.begin(), output.begin() + drop);
        }
    }
    m_outputOrigin = finalPaddedSample;
    return true;
}

} // namespace stems
} // namespace mixxx
