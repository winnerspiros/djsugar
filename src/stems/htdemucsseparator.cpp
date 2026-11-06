#include "stems/htdemucsseparator.h"

#include <QDir>
#include <QFile>
#include <algorithm>
#include <cmath>

#include "util/logger.h"

namespace mixxx {
namespace stems {

namespace {

const Logger kLogger("HTDemucsSeparator");

constexpr SINT kSegment = 343980;
constexpr SINT kStride = kSegment * 3 / 4;
// Stop before Android kills the app for lack of memory
constexpr int kMinAvailableMemoryMb = 300;

const QString kModelFile = QStringLiteral("htdemucs.onnx");
// The model as optimized by ONNX Runtime on the first run, which loads in a
// second instead of half a minute
const QString kOptimizedModelFile = QStringLiteral("htdemucs.optimized.onnx");

const char* const kInputName = "mix";
const char* const kOutputName = "stems";

} // namespace

bool HTDemucsSeparator::load(const QString& modelDir, const Options& options) {
    const QDir dir(modelDir);
    const QString optimizedPath = dir.filePath(kOptimizedModelFile);
    try {
        Ort::SessionOptions sessionOptions = StemSeparator::sessionOptions(options);
        if (QFile::exists(optimizedPath)) {
            sessionOptions.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_DISABLE_ALL);
            m_pSession = std::make_unique<Ort::Session>(onnxRuntimeEnv(),
                    optimizedPath.toUtf8().constData(),
                    sessionOptions);
            return true;
        }
        const QString pendingPath = optimizedPath + QStringLiteral(".part");
        sessionOptions.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_EXTENDED);
        sessionOptions.SetOptimizedModelFilePath(pendingPath.toUtf8().constData());
        m_pSession = std::make_unique<Ort::Session>(onnxRuntimeEnv(),
                dir.filePath(kModelFile).toUtf8().constData(),
                sessionOptions);
        if (!QFile::rename(pendingPath, optimizedPath)) {
            kLogger.warning() << "Unable to save the optimized model" << optimizedPath;
            QFile::remove(pendingPath);
        }
    } catch (const Ort::Exception& e) {
        m_error = QString::fromUtf8(e.what());
        return false;
    }
    return true;
}

bool HTDemucsSeparator::separate(const float* pStereo,
        SINT frameCount,
        const StemSink& sink,
        const ProgressCallback& progress) {
    // The model expects the audio normalized like the whole track
    double sum = 0.0;
    double sumOfSquares = 0.0;
    for (SINT i = 0; i < frameCount; ++i) {
        const double mono = 0.5 * (pStereo[2 * i] + pStereo[2 * i + 1]);
        sum += mono;
        sumOfSquares += mono * mono;
    }
    const double mean = frameCount > 0 ? sum / frameCount : 0.0;
    const float deviation = static_cast<float>(std::sqrt(std::max(0.0,
                                                 frameCount > 0
                                                         ? sumOfSquares / frameCount - mean * mean
                                                         : 0.0))) +
            1e-8f;

    // Triangular weights of the overlap-add
    std::vector<float> weights(kSegment);
    for (SINT i = 0; i < kSegment / 2; ++i) {
        weights[i] = static_cast<float>(i + 1) / (kSegment / 2);
        weights[kSegment - 1 - i] = weights[i];
    }

    const SINT segments = 1 + (std::max<SINT>(0, frameCount - kSegment) + kStride - 1) / kStride;
    std::vector<float> input(2 * kSegment);
    std::vector<float> output(kStemCount * 2 * kSegment);
    const std::array<int64_t, 3> inputShape = {1, 2, kSegment};
    const std::array<int64_t, 4> outputShape = {1, kStemCount, 2, kSegment};
    const auto memoryInfo = Ort::MemoryInfo::CreateCpu(OrtDeviceAllocator, OrtMemTypeDefault);

    // Overlap-add of the stems from the first sample not handed to the sink
    SINT origin = 0;
    std::array<std::vector<float>, kStemCount * 2> sums;
    std::vector<float> weightSums;
    std::array<std::vector<float>, kStemCount> emitBuffers;

    for (SINT segment = 0; segment < segments; ++segment) {
        if (availableMemoryMb() >= 0 && availableMemoryMb() < kMinAvailableMemoryMb) {
            m_error = QStringLiteral("Low memory");
            return false;
        }
        const SINT offset = segment * kStride;
        const SINT length = std::min(kSegment, frameCount - offset);
        // The last, shorter segment is centered
        const SINT pad = kSegment - length;
        const SINT start = offset - pad / 2;
        for (SINT i = 0; i < kSegment; ++i) {
            const SINT at = start + i;
            const bool inside = at >= 0 && at < frameCount;
            input[i] = inside ? static_cast<float>((pStereo[2 * at] - mean) / deviation) : 0.0f;
            input[kSegment + i] = inside ? static_cast<float>((pStereo[2 * at + 1] - mean) / deviation) : 0.0f;
        }
        try {
            Ort::Value inputTensor = Ort::Value::CreateTensor<float>(memoryInfo,
                    input.data(),
                    input.size(),
                    inputShape.data(),
                    inputShape.size());
            Ort::Value outputTensor = Ort::Value::CreateTensor<float>(memoryInfo,
                    output.data(),
                    output.size(),
                    outputShape.data(),
                    outputShape.size());
            m_pSession->Run(Ort::RunOptions{nullptr},
                    &kInputName,
                    &inputTensor,
                    1,
                    &kOutputName,
                    &outputTensor,
                    1);
        } catch (const Ort::Exception& e) {
            m_error = QString::fromUtf8(e.what());
            return false;
        }

        const SINT end = offset + length - origin;
        if (static_cast<SINT>(weightSums.size()) < end) {
            weightSums.resize(end, 0.0f);
            for (auto& stemSums : sums) {
                stemSums.resize(end, 0.0f);
            }
        }
        for (SINT i = 0; i < length; ++i) {
            weightSums[offset - origin + i] += weights[i];
        }
        for (int stemChannel = 0; stemChannel < kStemCount * 2; ++stemChannel) {
            const float* pOutput = &output[stemChannel * kSegment + pad / 2];
            float* pSums = &sums[stemChannel][offset - origin];
            for (SINT i = 0; i < length; ++i) {
                pSums[i] += pOutput[i] * weights[i];
            }
        }

        // The samples before the next segment are final
        const SINT finalSample = segment == segments - 1 ? frameCount : offset + kStride;
        const SINT count = finalSample - origin;
        for (int stem = 0; stem < kStemCount; ++stem) {
            std::vector<float>& buffer = emitBuffers[stem];
            buffer.resize(2 * count);
            for (SINT i = 0; i < count; ++i) {
                const float scale = weightSums[i] > 0.0f ? deviation / weightSums[i] : 0.0f;
                buffer[2 * i] = sums[stem * 2][i] * scale;
                buffer[2 * i + 1] = sums[stem * 2 + 1][i] * scale;
            }
        }
        if (!sink({emitBuffers[0].data(),
                            emitBuffers[1].data(),
                            emitBuffers[2].data(),
                            emitBuffers[3].data()},
                    count)) {
            m_error = QStringLiteral("Canceled");
            return false;
        }
        weightSums.erase(weightSums.begin(), weightSums.begin() + count);
        for (auto& stemSums : sums) {
            stemSums.erase(stemSums.begin(), stemSums.begin() + count);
        }
        origin = finalSample;

        if (progress && !progress(static_cast<double>(segment + 1) / segments)) {
            m_error = QStringLiteral("Canceled");
            return false;
        }
    }
    return true;
}

} // namespace stems
} // namespace mixxx
