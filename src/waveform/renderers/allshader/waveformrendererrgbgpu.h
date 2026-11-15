#pragma once

#include <QElapsedTimer>

#include "rendergraph/geometrynode.h"
#include "track/track_decl.h"
#include "util/class.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"
#include "waveform/waveform.h"

namespace allshader {
class WaveformRendererRGBGpu;
} // namespace allshader

/// The RGB waveform drawn by the GPU (QML only): waveform data uploaded
/// once into a texture, vertex shader (waveformrgb.vert) gives the pixel
/// columns (geometry built once) with the same formulas as
/// WaveformRendererRGB. Each frame changes only a few uniforms: no geometry
/// is built or uploaded, no scene graph re-sorting. On slow devices
/// (e.g. Adreno 610), building thousands of rectangles per frame stuttered.
///
/// Not for the slip renderer nor split stereo signal: WaveformRendererRGB
/// draws those.
class allshader::WaveformRendererRGBGpu final
        : public allshader::WaveformRendererSignalBase,
          public rendergraph::GeometryNode {
  public:
    explicit WaveformRendererRGBGpu(WaveformWidgetRenderer* waveformWidget,
            ::WaveformRendererSignalBase::Options options =
                    ::WaveformRendererSignalBase::Option::None);

    // Pure virtual from WaveformRendererSignalBase, not used
    void onSetup(const QDomNode& node) override;

    // Virtuals for rendergraph::Node
    void preprocess() override;

  private:
    bool preprocessInner();
    /// The rectangles of the pixel columns and of the axis
    void buildGeometry(int pixelLength);
    /// Uploads the waveform data if it changed (analysis in progress)
    bool updateTexture(const ConstWaveformPointer& pWaveform);

    int m_pixelLength;
    // The waveform data in the texture
    const Waveform* m_pTextureWaveform;
    int m_textureCompletion;
    int m_textureDataSize;
    int m_textureWidth;
    int m_textureHeight;
    QElapsedTimer m_textureAge;

    DISALLOW_COPY_AND_ASSIGN(WaveformRendererRGBGpu);
};
