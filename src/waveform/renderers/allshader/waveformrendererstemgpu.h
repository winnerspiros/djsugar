#pragma once

#include <QElapsedTimer>
#include <QVarLengthArray>
#include <memory>
#include <vector>

#include "rendergraph/geometrynode.h"
#include "util/class.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"
#include "waveform/waveform.h"

class ControlProxy;

namespace allshader {
class WaveformRendererStemGpu;
} // namespace allshader

/// The stem waveform drawn by the GPU (QML only), like
/// WaveformRendererRGBGpu: the stem data is uploaded once into a texture and
/// a vertex shader (waveformstem.vert) gives a geometry made once (eight
/// rectangles per strip: the outline and the fill of four stems) its
/// heights and colors, with the same formulas as WaveformRendererStem. On
/// every frame only a few uniforms change.
///
/// Not for the slip renderer: WaveformRendererStem draws it.
class allshader::WaveformRendererStemGpu final
        : public allshader::WaveformRendererSignalBase,
          public rendergraph::GeometryNode {
  public:
    explicit WaveformRendererStemGpu(WaveformWidgetRenderer* waveformWidget);
    ~WaveformRendererStemGpu() override;

    // Pure virtual from WaveformRendererSignalBase, not used
    void onSetup(const QDomNode& node) override;

    bool init() override;

    // Virtuals for rendergraph::Node
    void preprocess() override;

    void setSplitStemTracks(bool splitStemTracks) {
        m_splitStemTracks = splitStemTracks;
    }
    void setReorderOnChange(bool value);
    void setOutlineOpacity(float value) {
        m_outlineOpacity = value;
    }
    void setOpacity(float value) {
        m_opacity = value;
    }

  private:
    bool preprocessInner();
    void buildGeometry(int stripCount);
    bool updateTexture(const ConstWaveformPointer& pWaveform);

    bool m_splitStemTracks;
    bool m_reorderOnChange;
    float m_outlineOpacity;
    float m_opacity;

    std::vector<std::unique_ptr<ControlProxy>> m_pStemGain;
    std::vector<std::unique_ptr<ControlProxy>> m_pStemMute;
    QVarLengthArray<int, mixxx::kMaxSupportedStems> m_stackOrder;

    int m_stripCount;
    const Waveform* m_pTextureWaveform;
    int m_textureCompletion;
    int m_textureDataSize;
    int m_textureHeight;
    QElapsedTimer m_textureAge;

    DISALLOW_COPY_AND_ASSIGN(WaveformRendererStemGpu);
};
