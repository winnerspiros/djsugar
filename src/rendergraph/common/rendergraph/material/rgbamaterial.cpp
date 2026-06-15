#include "rgbamaterial.h"

#include <QVector2D>

#include "rendergraph/materialshader.h"
#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
#include "../opengl/backend/shadercache.h"
#endif
#include "rendergraph/materialtype.h"
#include "rendergraph/uniformset.h"

using namespace rendergraph;

RGBAMaterial::RGBAMaterial()
        : Material(uniforms()) {
}

// static
const AttributeSet& RGBAMaterial::attributes() {
    static AttributeSet set = makeAttributeSet<QVector2D, QVector4D>({"position", "color"});
    return set;
}

// static
const UniformSet& RGBAMaterial::uniforms() {
    static UniformSet set = makeUniformSet<QMatrix4x4>({"ubuf.matrix"});
    return set;
}

MaterialType* RGBAMaterial::type() const {
    static MaterialType type;
    return &type;
}

#ifndef MIXXX_USE_LLGL
std::unique_ptr<MaterialShader> RGBAMaterial::createShader() const {
    return std::make_unique<MaterialShader>(
            "rgba.vert", "rgba.frag", uniforms(), attributes());
}
#endif

#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
std::unique_ptr<LLGLMaterialShader> RGBAMaterial::createLLGLShader() const {
    return std::make_unique<LLGLMaterialShader>(
            "rgba.vert", "rgba.frag", uniforms(), attributes());
}
#endif
