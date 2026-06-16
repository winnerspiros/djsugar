#pragma once

#include "llglmaterialshader.h"
#include "rendergraph/materialshader.h"
#include "rendergraph/materialtype.h"

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
#endif

  private:
    BaseMaterial* m_pLastModifier = nullptr;
};
