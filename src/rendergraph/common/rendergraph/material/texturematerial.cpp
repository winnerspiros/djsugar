#include "texturematerial.h"

#include <QMatrix4x4>
#include <QVector2D>

#include "rendergraph/materialshader.h"
#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
#include "../opengl/backend/shadercache.h"
#endif
#include "rendergraph/materialtype.h"

using namespace rendergraph;

TextureMaterial::TextureMaterial()
        : Material(uniforms()) {
}

/* static */ const AttributeSet& TextureMaterial::attributes() {
    static AttributeSet set = makeAttributeSet<QVector2D, QVector2D>({"position", "texcoord"});
    return set;
}

/* static */ const UniformSet& TextureMaterial::uniforms() {
    static UniformSet set = makeUniformSet<QMatrix4x4, float>({"ubuf.matrix", "ubuf.alpha"});
    return set;
}

MaterialType* TextureMaterial::type() const {
    static MaterialType type;
    return &type;
}

#ifndef MIXXX_USE_LLGL
std::unique_ptr<MaterialShader> TextureMaterial::createShader() const {
    return std::make_unique<MaterialShader>(
            "texture.vert", "texture.frag", uniforms(), attributes());
}
#endif

#if defined(MIXXX_USE_LLGL) && !defined(RENDERGRAPH_SG)
std::unique_ptr<LLGLMaterialShader> TextureMaterial::createLLGLShader() const {
    return std::make_unique<LLGLMaterialShader>(
            "texture.vert", "texture.frag", uniforms(), attributes());
}
#endif
