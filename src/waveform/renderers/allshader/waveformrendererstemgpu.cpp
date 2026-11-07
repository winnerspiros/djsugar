#include "waveform/renderers/allshader/waveformrendererstemgpu.h"

#include <QColor>
#include <QMatrix4x4>
#include <QSGMaterial>
#include <QSGTexture>
#include <QVector4D>
#include <algorithm>
#include <numeric>

#include "control/controlproxy.h"
#include "engine/channels/enginedeck.h"
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

// Like WaveformRendererStem on the scene graph
constexpr float kPixelPerStrip = 2;
// Four stems, each drawn twice (outline and fill)
constexpr int kStems = 4;
constexpr int kLayers = 2 * kStems;

/// The uniforms of waveformstem.vert, in the order of its uniform block
enum Uniform {
    kMatrix,
    kColor0,
    kColor1,
    kColor2,
    kColor3,
    kStackOrder,
    kStemGain,
    kFirstFrame,
    kFramesPerPixel,
    kFrameCount,
    kTextureWidth,
    kTextureHeight,
    kHalfBreadth,
    kStemBreadth,
    kHeightFactor,
    kSplit,
    kOpacity,
    kOutlineOpacity,
    kStemCount,
    kStripWidth,
    kHalfStripWidth,
    kLowGain,
    kMidGain,
    kHighGain,
};

class WaveformStemGpuMaterial : public Material {
  public:
    WaveformStemGpuMaterial()
            : Material(uniforms()) {
        // The vertex shader computes the positions: the scene graph must not
        // merge this geometry with others, transforming it on the CPU
        setFlag(QSGMaterial::RequiresFullMatrix | QSGMaterial::Blending);
    }

    static const AttributeSet& attributes() {
        static AttributeSet set = makeAttributeSet<QVector4D>({"vertexIn"});
        return set;
    }

    static const UniformSet& uniforms() {
        static UniformSet set = makeUniformSet<QMatrix4x4,
                QVector4D,
                QVector4D,
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
                float,
                float,
                float,
                float,
                float>({"ubuf.matrix",
                "ubuf.color0",
                "ubuf.color1",
                "ubuf.color2",
                "ubuf.color3",
                "ubuf.stackOrder",
                "ubuf.stemGain",
                "ubuf.firstFrame",
                "ubuf.framesPerPixel",
                "ubuf.frameCount",
                "ubuf.textureWidth",
                "ubuf.textureHeight",
                "ubuf.halfBreadth",
                "ubuf.stemBreadth",
                "ubuf.heightFactor",
                "ubuf.split",
                "ubuf.opacity",
                "ubuf.outlineOpacity",
                "ubuf.stemCount",
                "ubuf.stripWidth",
                "ubuf.halfStripWidth",
                "ubuf.lowGain",
                "ubuf.midGain",
                "ubuf.highGain"});
        return set;
    }

    MaterialType* type() const override {
        static MaterialType type;
        return &type;
    }

    std::unique_ptr<MaterialShader> createShader() const override {
        return std::make_unique<MaterialShader>(
                "waveformstem.vert", "waveformstem.frag", uniforms(), attributes());
    }

    /// Binding 1: the stems, 2: the bands of the mix
    Texture* texture(int binding) const override {
        return binding == 2 ? m_pBandTexture.get() : m_pTexture.get();
    }

    void setTextures(std::unique_ptr<Texture> pTexture, std::unique_ptr<Texture> pBandTexture) {
        m_pTexture = std::move(pTexture);
        m_pBandTexture = std::move(pBandTexture);
    }

  private:
    std::unique_ptr<Texture> m_pTexture;
    std::unique_ptr<Texture> m_pBandTexture;
};

struct StripVertex {
    QVector4D vertex;
};

QVector4D colorVector(const QColor& color) {
    return QVector4D(static_cast<float>(color.redF()),
            static_cast<float>(color.greenF()),
            static_cast<float>(color.blueF()),
            static_cast<float>(color.alphaF()));
}

} // namespace

namespace allshader {

WaveformRendererStemGpu::WaveformRendererStemGpu(WaveformWidgetRenderer* waveformWidget)
        : WaveformRendererSignalBase(waveformWidget, ::WaveformRendererSignalBase::Option::None),
          m_splitStemTracks(false),
          m_reorderOnChange(false),
          m_outlineOpacity(0.15f),
          m_opacity(0.75f),
          m_stripCount(-1),
          m_pTextureWaveform(nullptr),
          m_textureCompletion(-1),
          m_textureDataSize(0),
          m_textureHeight(1) {
    setGeometry(std::make_unique<Geometry>(WaveformStemGpuMaterial::attributes(), 0));
    setMaterial(std::make_unique<WaveformStemGpuMaterial>());
    geometry().setDrawingMode(Geometry::DrawingMode::Triangles);
    setUsePreprocess(true);
    m_stackOrder.resize(mixxx::kMaxSupportedStems);
    std::iota(m_stackOrder.begin(), m_stackOrder.end(), 0);
}

WaveformRendererStemGpu::~WaveformRendererStemGpu() = default;

void WaveformRendererStemGpu::onSetup(const QDomNode&) {
}

void WaveformRendererStemGpu::setReorderOnChange(bool value) {
    m_reorderOnChange = value;
    // The natural order again
    std::iota(m_stackOrder.begin(), m_stackOrder.end(), 0);
}

bool WaveformRendererStemGpu::init() {
    // The EQ controls, for the gains of getGains()
    ::WaveformRendererSignalBase::init();
    // As WaveformRendererStem::init()
    m_pStemGain.clear();
    m_pStemMute.clear();
    m_stackOrder.resize(mixxx::kMaxSupportedStems);
    std::iota(m_stackOrder.begin(), m_stackOrder.end(), 0);
    if (m_waveformRenderer->getGroup().isEmpty()) {
        return true;
    }
    for (int stemIdx = 0; stemIdx < mixxx::kMaxSupportedStems; stemIdx++) {
        const QString stemGroup =
                EngineDeck::getGroupForStem(m_waveformRenderer->getGroup(), stemIdx);
        m_pStemGain.emplace_back(
                std::make_unique<ControlProxy>(stemGroup, QStringLiteral("volume")));
        m_pStemMute.emplace_back(
                std::make_unique<ControlProxy>(stemGroup, QStringLiteral("mute")));
        auto bringToForeground = [this, stemIdx](double) {
            if (!m_reorderOnChange) {
                return;
            }
            m_stackOrder.removeAll(stemIdx);
            m_stackOrder.append(stemIdx);
        };
        m_pStemGain.back()->connectValueChanged(this, bringToForeground);
        m_pStemMute.back()->connectValueChanged(this, bringToForeground);
    }
    return true;
}

void WaveformRendererStemGpu::preprocess() {
    if (!preprocessInner()) {
        if (geometry().vertexCount() != 0) {
            geometry().allocate(0);
            markDirtyGeometry();
            m_stripCount = -1;
        }
    }
}

void WaveformRendererStemGpu::buildGeometry(int stripCount) {
    constexpr float kCorners[6][2] = {{-1, -1}, {1, -1}, {-1, 1}, {1, -1}, {-1, 1}, {1, 1}};
    geometry().allocate(6 * kLayers * stripCount);
    auto* pVertex = geometry().vertexDataAs<StripVertex>();
    for (int strip = 0; strip < stripCount; ++strip) {
        // In drawing order: stack layer by stack layer, outline then fill
        for (int layer = 0; layer < kLayers; ++layer) {
            for (const auto& corner : kCorners) {
                (pVertex++)->vertex = QVector4D(static_cast<float>(strip),
                        corner[0],
                        corner[1],
                        static_cast<float>(layer));
            }
        }
    }
    markDirtyGeometry();
    m_stripCount = stripCount;
}

bool WaveformRendererStemGpu::updateTexture(const ConstWaveformPointer& pWaveform) {
    const int dataSize = pWaveform->getDataSize();
    const int completion = pWaveform->getCompletion();
    auto& material = static_cast<WaveformStemGpuMaterial&>(this->material());
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
    auto pTexture = std::make_unique<Texture>(pContext,
            waveformgpudata::image(*pWaveform, waveformgpudata::Content::Stems));
    auto pBandTexture = std::make_unique<Texture>(pContext,
            waveformgpudata::image(*pWaveform, waveformgpudata::Content::Bands));
    // Exact values, not mixed with the neighbours
    pTexture->backendTexture()->setFiltering(QSGTexture::Nearest);
    pBandTexture->backendTexture()->setFiltering(QSGTexture::Nearest);
    material.setTextures(std::move(pTexture), std::move(pBandTexture));
    markDirtyMaterial();

    m_pTextureWaveform = pWaveform.get();
    m_textureCompletion = completion;
    m_textureDataSize = dataSize;
    m_textureHeight = waveformgpudata::textureHeight(dataSize);
    m_textureAge.start();
    return true;
}

bool WaveformRendererStemGpu::preprocessInner() {
    const TrackPointer pTrack = m_waveformRenderer->getTrackInfo();
    if (!pTrack) {
        return false;
    }
    const auto stemInfo = pTrack->getStemInfo();
    if (stemInfo.isEmpty()) {
        return false;
    }
    const ConstWaveformPointer pWaveform = pTrack->getWaveform();
    if (pWaveform.isNull() || !pWaveform->hasStem()) {
        return false;
    }
    const int dataSize = pWaveform->getDataSize();
    if (dataSize <= 1 || pWaveform->data() == nullptr) {
        return false;
    }

    const float devicePixelRatio = m_waveformRenderer->getDevicePixelRatio();
    const int pixelLength = static_cast<int>(m_waveformRenderer->getLength() * devicePixelRatio);
    const int stripCount = static_cast<int>(static_cast<float>(pixelLength) / kPixelPerStrip);
    if (stripCount <= 0) {
        return false;
    }

    // As in WaveformRendererStem
    const int visualFramesSize = dataSize / 2;
    const double firstVisualFrame =
            m_waveformRenderer->getFirstDisplayedPosition() * visualFramesSize;
    const double lastVisualFrame =
            m_waveformRenderer->getLastDisplayedPosition() * visualFramesSize;
    const double visualIncrementPerPixel =
            (lastVisualFrame - firstVisualFrame) / static_cast<double>(stripCount);
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
    const float breadth = static_cast<float>(m_waveformRenderer->getBreadth());
    const float stemBreadth = m_splitStemTracks ? breadth / 4.0f : 0.0f;
    const float halfBreadth = (m_splitStemTracks ? stemBreadth : breadth) / 2.0f;

    if (!updateTexture(pWaveform)) {
        return false;
    }
    if (stripCount != m_stripCount) {
        buildGeometry(stripCount);
    }

    // The gain of the fill of each stem, as in WaveformRendererStem
    const uint selectedStems = m_waveformRenderer->getSelectedStems();
    float gains[kStems] = {1.f, 1.f, 1.f, 1.f};
    QVector4D colors[kStems];
    for (int stem = 0; stem < kStems; ++stem) {
        if (selectedStems) {
            gains[stem] = (selectedStems & 1 << stem) ? 1.f : 0.f;
        } else if (stem < static_cast<int>(m_pStemMute.size()) && m_pStemMute[stem]->toBool()) {
            gains[stem] = 0.f;
        } else if (stem < static_cast<int>(m_pStemGain.size())) {
            gains[stem] = static_cast<float>(m_pStemGain[stem]->get());
        }
        colors[stem] = stem < stemInfo.size() ? colorVector(stemInfo[stem].getColor())
                                              : QVector4D();
    }
    float order[kStems] = {0.f, 1.f, 2.f, 3.f};
    for (int layer = 0; layer < kStems && layer < m_stackOrder.size(); ++layer) {
        order[layer] = static_cast<float>(m_stackOrder[layer]);
    }

    Material& material = this->material();
    material.setUniform(kColor0, colors[0]);
    material.setUniform(kColor1, colors[1]);
    material.setUniform(kColor2, colors[2]);
    material.setUniform(kColor3, colors[3]);
    material.setUniform(kStackOrder, QVector4D(order[0], order[1], order[2], order[3]));
    material.setUniform(kStemGain, QVector4D(gains[0], gains[1], gains[2], gains[3]));
    material.setUniform(kFirstFrame, static_cast<float>(xVisualFrame));
    material.setUniform(kFramesPerPixel, static_cast<float>(visualIncrementPerPixel));
    material.setUniform(kFrameCount, static_cast<float>(visualFramesSize));
    material.setUniform(kTextureWidth, static_cast<float>(waveformgpudata::kTextureWidth));
    material.setUniform(kTextureHeight, static_cast<float>(m_textureHeight));
    material.setUniform(kHalfBreadth, halfBreadth);
    material.setUniform(kStemBreadth, stemBreadth);
    material.setUniform(kHeightFactor, allGain * halfBreadth);
    material.setUniform(kSplit, m_splitStemTracks ? 1.f : 0.f);
    material.setUniform(kOpacity, m_opacity);
    material.setUniform(kOutlineOpacity, m_outlineOpacity);
    material.setUniform(kStemCount, static_cast<float>(std::min<int>(kStems, stemInfo.size())));
    material.setUniform(kStripWidth, kPixelPerStrip / devicePixelRatio);
    material.setUniform(kHalfStripWidth, kPixelPerStrip / 2.0f / devicePixelRatio);
    material.setUniform(kLowGain, lowGain);
    material.setUniform(kMidGain, midGain);
    material.setUniform(kHighGain, highGain);
    markDirtyMaterial();
    return true;
}

} // namespace allshader
