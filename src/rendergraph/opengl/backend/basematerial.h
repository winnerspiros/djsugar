#pragma once

#include "rendergraph/materialshader.h"
#include "rendergraph/materialtype.h"

#ifdef MIXXX_USE_LLGL
#include "rendergraph/llgl/backend/shadercache.h"
#endif

namespace rendergraph {
class BaseMaterial;
} // namespace rendergraph

class rendergraph::BaseMaterial {
  protected:
    BaseMaterial() = default;

  public:
    virtual MaterialType* type() const = 0;

    int compare(const BaseMaterial* other) const;

#ifdef MIXXX_USE_LLGL
    void setShader(std::shared_ptr<LLGLMaterialShader> pShader) {
        m_pLLGLShader = pShader;
    }
    LLGLMaterialShader& shader() const {
        return *m_pLLGLShader;
    }
    int uniformLocation(int uniformIndex) const {
        return m_pLLGLShader ? m_pLLGLShader->uniformLocation(uniformIndex) : -1;
    }
    int attributeLocation(int attributeIndex) const {
        return m_pLLGLShader ? m_pLLGLShader->attributeLocation(attributeIndex) : -1;
    }
    void modifyShader() {
        m_pLastModifier = this;
    }
    bool isLastModifierOfShader() const {
        return m_pLastModifier == this;
    }
  private:
    std::shared_ptr<LLGLMaterialShader> m_pLLGLShader;
    BaseMaterial* m_pLastModifier = nullptr;
#else
    void setShader(std::shared_ptr<MaterialShader> pShader);
    MaterialShader& shader() const;
    int uniformLocation(int uniformIndex) const;
    int attributeLocation(int attributeIndex) const;
    void modifyShader();
    bool isLastModifierOfShader() const;
  private:
    std::shared_ptr<MaterialShader> m_pShader;
#endif
};
