#include "rendergraph/attributeset.h"
#include "rendergraph/material.h"

namespace rendergraph {
class RGBAMaterial;
}

class rendergraph::RGBAMaterial : public rendergraph::Material {
  public:
    RGBAMaterial();

    static const AttributeSet& attributes();

    static const UniformSet& uniforms();

    MaterialType* type() const override;

#ifndef MIXXX_USE_LLGL
    std::unique_ptr<MaterialShader> createShader() const override;
#endif

#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
    std::unique_ptr<LLGLMaterialShader> createLLGLShader() const override;
#endif

};
