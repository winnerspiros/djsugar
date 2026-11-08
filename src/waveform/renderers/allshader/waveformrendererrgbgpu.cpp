#include "waveform/renderers/allshader/waveformrendererrgbgpu.h"

#include <QImage>
#include <QMatrix4x4>
#include <QSGMaterial>
#include <QSGTexture>
#include <QVector3D>
#include <QVector4D>
#include <algorithm>

#include "rendergraph/material.h"
#include "rendergraph/materialshader.h"
#include "rendergraph/materialtype.h"
#include "rendergraph/texture.h"
#include "rendergraph/uniformset.h"
#include "track/track.h"
#include "waveform/renderers/allshader/waveformgpudata.h"
#include "waveform/renderers/waveformwidgetrenderer.h"

using namespace rendergraph;

namespace {

/// The uniforms of waveformrgb.vert, in the order of its uniform block (all
/// of them packed as in std140)
enum Uniform {
    kMatrix,
    kLowColor,
    kMidColor,
    kHighColor,
    kAxesColor,
    kFirstFrame,
    kFramesPerPixel,
    kFrameCount,
    kTextureWidthUniform,
    kTextureHeight,
    kHalfBreadth,
    kHeightFactor,
    kLowGain,
    kMidGain,
    kHighGain,
    kInvDevicePixelRatio,
    kHalfPixelSize,
    kLength,
};

class WaveformRGBGpuMaterial : public Material {
  public:
    WaveformRGBGpuMaterial()
            : Material(uniforms()) {
        // The vertex shader computes the positions: the scene graph must not
        // merge this geometry with others, transforming it on the CPU
        setFlag(QSGMaterial::RequiresFullMatrix);
    }

    static const AttributeSet& attributes() {
        static AttributeSet set = makeAttributeSet<QVector3D>({"vertexIn"});
        return set;
    }

    static const UniformSet& uniforms() {
        static UniformSet set = makeUniformSet<QMatrix4x4,
                QVector4D,
                QVector4D,
                QVector4D,
                QVector4D,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float,
                float>({"ubuf.matrix",
                "ubuf.lowColor",
                "ubuf.midColor",
                "ubuf.highColor",
                "ubuf.axesColor",
                "ubuf.firstFrame",
                "ubuf.framesPerPixel",
                "ubuf.frameCount",
                "ubuf.textureWidth",
                "ubuf.textureHeight",
                "ubuf.halfBreadth",
                "ubuf.heightFactor",
                "ubuf.lowGain",
                "ubuf.midGain",
                "ubuf.highGain",
                "ubuf.invDevicePixelRatio",
                "ubuf.halfPixelSize",
                "ubuf.waveformLength"});
        return set;
    }

    MaterialType* type() const override {
        static MaterialType type;
        return &type;
    }

    std::unique_ptr<MaterialShader> createShader() const override {
        return std::make_unique<MaterialShader>(
                "waveformrgb.vert", "waveformrgb.frag", uniforms(), attributes());
    }

    Texture* texture(int /*binding*/) const override {
        return m_pTexture.get();
    }

    void setTexture(std::unique_ptr<Texture> pTexture) {
        m_pTexture = std::move(pTexture);
    }

  private:
    std::unique_ptr<Texture> m_pTexture;
};

struct ColumnVertex {
    QVector3D vertex;
};

/// The two triangles of a rectangle of a column (-1 for the axis)
void addRectangle(ColumnVertex* pVertices, float column) {
    constexpr float kCorners[6][2] = {{-1, -1}, {1, -1}, {-1, 1}, {1, -1}, {-1, 1}, {1, 1}};
    for (int i = 0; i < 6; ++i) {
        pVertices[i].vertex = QVector3D(column, kCorners[i][0], kCorners[i][1]);
    }
}

} // namespace

namespace allshader {

WaveformRendererRGBGpu::WaveformRendererRGBGpu(
        WaveformWidgetRenderer* waveformWidget, ::WaveformRendererSignalBase::Options options)
        : WaveformRendererSignalBase(waveformWidget, options),
          m_pixelLength(-1),
          m_pTextureWaveform(nullptr),
          m_textureCompletion(-1),
          m_textureDataSize(0),
          m_textureWidth(waveformgpudata::kTextureWidth),
          m_textureHeight(1) {
    setGeometry(std::make_unique<Geometry>(WaveformRGBGpuMaterial::attributes(), 0));
    setMaterial(std::make_unique<WaveformRGBGpuMaterial>());
    geometry().setDrawingMode(Geometry::DrawingMode::Triangles);
    setUsePreprocess(true);
}

void WaveformRendererRGBGpu::onSetup(const QDomNode&) {
}

void WaveformRendererRGBGpu::preprocess() {
    if (!preprocessInner()) {
        if (geometry().vertexCount() != 0) {
            geometry().allocate(0);
            markDirtyGeometry();
            m_pixelLength = -1;
        }
    }
}

void WaveformRendererRGBGpu::buildGeometry(int pixelLength) {
    // The axis first, under the columns
    geometry().allocate(6 * (pixelLength + 1));
    auto* pVertices = geometry().vertexDataAs<ColumnVertex>();
    addRectangle(pVertices, -1.f);
    for (int column = 0; column < pixelLength; ++column) {
        addRectangle(pVertices + 6 * (column + 1), static_cast<float>(column));
    }
    markDirtyGeometry();
    m_pixelLength = pixelLength;
}

bool WaveformRendererRGBGpu::updateTexture(const ConstWaveformPointer& pWaveform) {
    const int dataSize = pWaveform->getDataSize();
    const int completion = pWaveform->getCompletion();
    auto& material = static_cast<WaveformRGBGpuMaterial&>(this->material());
    const bool sameData = pWaveform.get() == m_pTextureWaveform &&
            dataSize == m_textureDataSize && material.texture(0);
    if (sameData &&
            (completion == m_textureCompletion ||
                    m_textureAge.elapsed() < waveformgpudata::kAnalysisUploadMillis)) {
        return true;
    }
    rendergraph::Context* pContext = m_waveformRenderer->getContext();
    if (!pContext) {
        return false;
    }
    const int height = waveformgpudata::textureHeight(dataSize);
    auto pTexture = std::make_unique<Texture>(pContext,
            waveformgpudata::image(*pWaveform, waveformgpudata::Content::Bands));
    // Exact values, not mixed with the neighbours
    pTexture->backendTexture()->setFiltering(QSGTexture::Nearest);
    material.setTexture(std::move(pTexture));
    markDirtyMaterial();

    m_pTextureWaveform = pWaveform.get();
    m_textureCompletion = completion;
    m_textureDataSize = dataSize;
    m_textureHeight = height;
    m_textureAge.start();
    return true;
}

bool WaveformRendererRGBGpu::preprocessInner() {
    const TrackPointer pTrack = m_waveformRenderer->getTrackInfo();
    if (!pTrack) {
        return false;
    }
    const ConstWaveformPointer pWaveform = pTrack->getWaveform();
    if (pWaveform.isNull()) {
        return false;
    }
    const int dataSize = pWaveform->getDataSize();
    if (dataSize <= 1 || pWaveform->data() == nullptr) {
        return false;
    }
#ifdef __STEM__
    // The stem renderer draws stem tracks
    if (!pTrack->getStemInfo().isEmpty() && pWaveform->hasStem() && !m_ignoreStem) {
        return false;
    }
#endif

    const float devicePixelRatio = m_waveformRenderer->getDevicePixelRatio();
    const int length = static_cast<int>(m_waveformRenderer->getLength());
    const int pixelLength = static_cast<int>(length * devicePixelRatio);
    if (pixelLength <= 0) {
        return false;
    }

    // As in WaveformRendererRGB
    const int visualFramesSize = dataSize / 2;
    const double firstVisualFrame =
            m_waveformRenderer->getFirstDisplayedPosition() * visualFramesSize;
    const double lastVisualFrame =
            m_waveformRenderer->getLastDisplayedPosition() * visualFramesSize;
    const double visualIncrementPerPixel =
            (lastVisualFrame - firstVisualFrame) / static_cast<double>(pixelLength);
    if (visualIncrementPerPixel == 0.0) {
        return false;
    }
    const double xVisualFrame =
            qRound(firstVisualFrame / visualIncrementPerPixel) * visualIncrementPerPixel;

    float allGain = 1.0f;
    float lowGain = 1.0f;
    float midGain = 1.0f;
    float highGain = 1.0f;
    getGains(&allGain, &lowGain, &midGain, &highGain);
    const float halfBreadth = static_cast<float>(m_waveformRenderer->getBreadth()) / 2.0f;

    if (!updateTexture(pWaveform)) {
        return false;
    }
    if (pixelLength != m_pixelLength) {
        buildGeometry(pixelLength);
    }

    Material& material = this->material();
    material.setUniform(kLowColor,
            QVector4D(m_rgbLowColor_r, m_rgbLowColor_g, m_rgbLowColor_b, 1.f));
    material.setUniform(kMidColor,
            QVector4D(m_rgbMidColor_r, m_rgbMidColor_g, m_rgbMidColor_b, 1.f));
    material.setUniform(kHighColor,
            QVector4D(m_rgbHighColor_r, m_rgbHighColor_g, m_rgbHighColor_b, 1.f));
    material.setUniform(kAxesColor,
            QVector4D(m_axesColor_r, m_axesColor_g, m_axesColor_b, 1.f));
    material.setUniform(kFirstFrame, static_cast<float>(xVisualFrame));
    material.setUniform(kFramesPerPixel, static_cast<float>(visualIncrementPerPixel));
    material.setUniform(kFrameCount, static_cast<float>(visualFramesSize));
    material.setUniform(kTextureWidthUniform, static_cast<float>(m_textureWidth));
    material.setUniform(kTextureHeight, static_cast<float>(m_textureHeight));
    material.setUniform(kHalfBreadth, halfBreadth);
    material.setUniform(kHeightFactor, allGain * halfBreadth);
    material.setUniform(kLowGain, lowGain);
    material.setUniform(kMidGain, midGain);
    material.setUniform(kHighGain, highGain);
    material.setUniform(kInvDevicePixelRatio, 1.f / devicePixelRatio);
    material.setUniform(kHalfPixelSize, 0.5f / devicePixelRatio);
    material.setUniform(kLength, static_cast<float>(length));
    markDirtyMaterial();
    return true;
}

} // namespace allshader
