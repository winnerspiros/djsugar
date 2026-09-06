#pragma once

#include "rendergraph/geometrynode.h"
#include "util/class.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"

namespace llgl {
class LLGLRendererFiltered;
} // namespace llgl

class llgl::LLGLRendererFiltered final
        : public allshader::WaveformRendererSignalBase,
          public rendergraph::GeometryNode {
  public:
    explicit LLGLRendererFiltered(WaveformWidgetRenderer* waveformWidget,
            bool rgbStacked,
            ::WaveformRendererSignalBase::Options options);

    // Pure virtual from WaveformRendererSignalBase, not used
    void onSetup(const QDomNode& node) override;

    // Virtuals for rendergraph::Node
    void preprocess() override;

  private:
    const bool m_bRgbStacked;
    bool preprocessInner();

    DISALLOW_COPY_AND_ASSIGN(LLGLRendererFiltered);
};
