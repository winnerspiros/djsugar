#include "rendergraph/attributeset.h"
#include "rendergraph/material.h"
#include "rendergraph/materialtype.h"
#include "rendergraph/uniformset.h"

#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
#include "../opengl/backend/shadercache.h"
#endif

namespace rendergraph {
class RGBMaterial;
} // namespace rendergraph

class rendergraph::RGBMaterial : public rendergraph::Material {
  public:
    RGBMaterial();

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
