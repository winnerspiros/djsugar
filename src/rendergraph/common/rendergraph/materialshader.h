#pragma once

#include "rendergraph/attributeset.h"
#include "rendergraph/uniformset.h"

namespace rendergraph {
class MaterialShader;
} // namespace rendergraph

class rendergraph::MaterialShader {
  public:
    virtual ~MaterialShader() = default;

#ifndef MIXXX_USE_LLGL
    MaterialShader(const char* vertexShaderFile,
            const char* fragmentShaderFile,
            const UniformSet& uniforms,
            const AttributeSet& attributeSet);
#endif

    virtual bool bind() = 0;
    virtual void release() = 0;

    virtual int uniformLocation(int index) const = 0;
    virtual int attributeLocation(int index) const = 0;
};