#pragma once

#include "rendergraph/geometrynode.h"
#include "util/class.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"

namespace llgl {
class LLGLRendererHSV;
} // namespace llgl

class llgl::LLGLRendererHSV final
        : public allshader::WaveformRendererSignalBase,
          public rendergraph::GeometryNode {
  public:
    explicit LLGLRendererHSV(WaveformWidgetRenderer* waveformWidget,
            ::WaveformRendererSignalBase::Options options);

    // Pure virtual from WaveformRendererSignalBase, not used
    void onSetup(const QDomNode& node) override;

    // Virtuals for rendergraph::Node
    void preprocess() override;

  private:
    bool preprocessInner();

    DISALLOW_COPY_AND_ASSIGN(LLGLRendererHSV);
};
