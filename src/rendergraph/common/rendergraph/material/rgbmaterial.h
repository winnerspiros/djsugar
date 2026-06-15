#include "rendergraph/attributeset.h"
#include "rendergraph/material.h"
#include "rendergraph/materialtype.h"
#include "rendergraph/uniformset.h"

#ifdef MIXXX_USE_LLGL
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

#ifdef MIXXX_USE_LLGL
    std::unique_ptr<LLGLMaterialShader> createLLGLShader() const override;
#endif

};
