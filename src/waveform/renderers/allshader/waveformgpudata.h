#pragma once

#include <QImage>
#include <algorithm>

#include "waveform/waveform.h"

namespace allshader {

/// The waveform data of the waveforms drawn by the GPU
/// (WaveformRendererRGBGpu, WaveformRendererStemGpu), as an RGBA texture:
/// one texel per visual sample, left and right interleaved, row by row.
namespace waveformgpudata {

/// Texels per row. 2048 is the smallest maximum texture size of OpenGL ES 3;
/// a 20 minute track takes about 520 rows.
constexpr int kTextureWidth = 2048;

enum class Content {
    /// low, mid, high and all
    Bands,
    /// the first four stems
    Stems,
};

/// While a track is analyzed, its data is uploaded again at most this often
constexpr int kAnalysisUploadMillis = 300;

inline int textureHeight(int dataSize) {
    return std::max(1, (dataSize + kTextureWidth - 1) / kTextureWidth);
}

inline QImage image(const Waveform& waveform, Content content) {
    const int dataSize = waveform.getDataSize();
    // Declared premultiplied so that Qt uploads the bytes as they are: it
    // premultiplies other formats, which multiplied r, g and b (e.g. three
    // of the stems) by a
    QImage image(kTextureWidth, textureHeight(dataSize), QImage::Format_RGBA8888_Premultiplied);
    image.fill(Qt::transparent);
    const WaveformData* pData = waveform.data();
    // Through bits() once: scanLine() checks whether the image is shared on
    // every call, which took longer than the rest of the loop
    uchar* pBits = image.bits();
    const qsizetype bytesPerLine = image.bytesPerLine();
    for (int index = 0; index < dataSize; ++index) {
        const WaveformData& data = pData[index];
        uchar* pTexel = pBits + bytesPerLine * (index / kTextureWidth) +
                4 * (index % kTextureWidth);
        if (content == Content::Bands) {
            pTexel[0] = data.filtered.low;
            pTexel[1] = data.filtered.mid;
            pTexel[2] = data.filtered.high;
            pTexel[3] = data.filtered.all;
        } else {
            for (int stem = 0; stem < std::min(4, mixxx::kMaxSupportedStems); ++stem) {
                pTexel[stem] = data.stems[stem];
            }
        }
    }
    return image;
}

} // namespace waveformgpudata
} // namespace allshader
