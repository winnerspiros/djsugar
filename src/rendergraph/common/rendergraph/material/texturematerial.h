#include "rendergraph/attributeset.h"
#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
#include "../opengl/backend/shadercache.h"
#endif
#include "rendergraph/material.h"
#include "rendergraph/texture.h"
#include "rendergraph/uniformset.h"

namespace rendergraph {
class TextureMaterial;
}

class rendergraph::TextureMaterial : public rendergraph::Material {
  public:
    TextureMaterial();

    static const AttributeSet& attributes();

    static const UniformSet& uniforms();

    MaterialType* type() const override;

    std::unique_ptr<MaterialShader> createShader() const override;

#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
    std::unique_ptr<LLGLMaterialShader> createLLGLShader() const override;
#endif

    Texture* texture(int /*binding*/) const override {
        return m_pTexture.get();
    }

    void setTexture(std::unique_ptr<Texture> texture) {
        m_pTexture = std::move(texture);
    }

  private:
    std::unique_ptr<Texture> m_pTexture;
};
