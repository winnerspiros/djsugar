#include "rendergraph/attributeset.h"
#include "../opengl/backend/shadercache.h"
#include "rendergraph/material.h"

namespace rendergraph {
class EndOfTrackMaterial;
}

class rendergraph::EndOfTrackMaterial : public rendergraph::Material {
  public:
    EndOfTrackMaterial();

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
