#pragma once

#include "rendergraph/attributeset.h"
#include "rendergraph/uniformset.h"

namespace rendergraph {
class MaterialShader;
} // namespace rendergraph

class rendergraph::MaterialShader {
  public:
    virtual ~MaterialShader() = default;

    MaterialShader(const char* vertexShaderFile,
            const char* fragmentShaderFile,
            const UniformSet& uniforms,
            const AttributeSet& attributeSet) {
        Q_UNUSED(vertexShaderFile);
        Q_UNUSED(fragmentShaderFile);
        Q_UNUSED(uniforms);
        Q_UNUSED(attributeSet);
    }

    virtual bool bind() = 0;
    virtual void release() = 0;

    virtual int uniformLocation(int index) const = 0;
    virtual int attributeLocation(int index) const = 0;
};