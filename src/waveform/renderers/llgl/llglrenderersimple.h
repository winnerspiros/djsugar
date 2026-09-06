#pragma once

#include "rendergraph/geometrynode.h"
#include "util/class.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"

namespace llgl {
class LLGLRendererSimple;
} // namespace llgl

class llgl::LLGLRendererSimple final
        : public allshader::WaveformRendererSignalBase,
          public rendergraph::GeometryNode {
  public:
    explicit LLGLRendererSimple(WaveformWidgetRenderer* waveformWidget,
            ::WaveformRendererSignalBase::Options options);

    // Pure virtual from WaveformRendererSignalBase, not used
    void onSetup(const QDomNode& node) override;

    // Virtuals for rendergraph::Node
    void preprocess() override;

  private:
    bool preprocessInner();

    DISALLOW_COPY_AND_ASSIGN(LLGLRendererSimple);
};
