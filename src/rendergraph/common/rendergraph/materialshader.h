#pragma once

#include "rendergraph/attributeset.h"
#include "rendergraph/uniformset.h"

namespace rendergraph {
class MaterialShader;
} // namespace rendergraph

#ifdef RENDERGRAPH_SG
// SG target: MaterialShader inherits from QSGMaterialShader
#include <QSGMaterialShader>
class rendergraph::MaterialShader : public QSGMaterialShader {
#else
// GL target: MaterialShader is standalone
class rendergraph::MaterialShader {
#endif
  public:
    virtual ~MaterialShader() = default;

    MaterialShader(const char* vertexShaderFile,
            const char* fragmentShaderFile,
            const UniformSet& uniforms,
            const AttributeSet& attributeSet);

    virtual bool bind() {
        return true;
    }
    virtual void release() {
    }

    virtual int uniformLocation(int index) const {
        return -1;
    }
    virtual int attributeLocation(int index) const {
        return -1;
    }
};
