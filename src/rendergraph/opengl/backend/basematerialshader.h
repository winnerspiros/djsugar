#pragma once

#include "rendergraph/materialshader.h"
#include <QString>
#include <vector>

namespace rendergraph {
class BaseMaterial;
class MaterialShader;
class LLGLShaderProgram;
class BaseMaterialShader;
} // namespace rendergraph

class rendergraph::BaseMaterialShader : public MaterialShader {
  protected:
    BaseMaterialShader(const char* vertexShaderFile,
            const char* fragmentShaderFile,
            const UniformSet& uniforms,
            const AttributeSet& attributeSet)
        : MaterialShader(vertexShaderFile, fragmentShaderFile, uniforms, attributeSet) {
    }

  public:
    virtual ~BaseMaterialShader() = default;

    virtual int attributeLocation(int attributeIndex) const;
    virtual int uniformLocation(int uniformIndex) const;

    BaseMaterial* lastModifiedByMaterial() const;
    void setLastModifiedByMaterial(BaseMaterial* pMaterial);

    virtual bool bind() = 0;
    virtual void release() = 0;

  protected:
    std::vector<int> m_attributeLocations;
    std::vector<int> m_uniformLocations;
    BaseMaterial* m_pLastModifiedByMaterial{};
};
