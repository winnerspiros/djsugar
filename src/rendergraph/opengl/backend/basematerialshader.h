#pragma once

#include <QString>
#include <vector>

namespace rendergraph {
class BaseMaterial;
class LLGLShaderProgram;
} // namespace rendergraph

class rendergraph::BaseMaterialShader {
  protected:
    BaseMaterialShader() = default;

  public:
    virtual ~BaseMaterialShader() = default;

    int attributeLocation(int attributeIndex) const;
    int uniformLocation(int uniformIndex) const;

    BaseMaterial* lastModifiedByMaterial() const;
    void setLastModifiedByMaterial(BaseMaterial* pMaterial);

    virtual bool bind() = 0;
    virtual void release() = 0;

  protected:
    std::vector<int> m_attributeLocations;
    std::vector<int> m_uniformLocations;
    BaseMaterial* m_pLastModifiedByMaterial{};
};
