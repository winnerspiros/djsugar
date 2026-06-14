#include "backend/basegeometrynode.h"

#include <QOpenGLTexture>
#include <stdexcept>

#include "backend/shadercache.h"
#include "rendergraph/engine.h"
#include "rendergraph/geometrynode.h"
#include "rendergraph/texture.h"

#ifdef MIXXX_USE_LLGL
#include "rendergraph/llgl/rendergraph/context.h"
#endif

using namespace rendergraph;

namespace {
GLenum toGlDrawingMode(DrawingMode mode) {
    switch (mode) {
    case DrawingMode::Triangles:
        return GL_TRIANGLES;
    case DrawingMode::TriangleStrip:
        return GL_TRIANGLE_STRIP;
    default:
        throw std::runtime_error("not implemented");
    }
}
} // namespace

void BaseGeometryNode::initialize() {
#ifndef MIXXX_USE_LLGL
    initializeOpenGLFunctions();
#endif
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    pThis->material().setShader(ShaderCache::getShaderForMaterial(&pThis->material()));
    pThis->material().setUniform(0, engine()->matrix());
}

void BaseGeometryNode::render() {
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    Geometry& geometry = pThis->geometry();
    Material& material = pThis->material();

    if (geometry.vertexCount() == 0) {
        return;
    }

#ifdef MIXXX_USE_LLGL
    renderLLGL(pThis, geometry, material);
#else
    renderGL(pThis, geometry, material);
#endif
}

#ifdef MIXXX_USE_LLGL

void BaseGeometryNode::renderLLGL(GeometryNode* pThis,
        Geometry& geometry,
        Material& material) {
    Q_UNUSED(pThis);

    auto* pLLGLNode = dynamic_cast<BaseLLGLNode*>(this);
    if (!pLLGLNode) return;
    auto* pContext = pLLGLNode->context();
    if (!pContext || !pContext->isValid()) return;
    auto* pCmdBuf = pContext->commandBuffer();
    if (!pCmdBuf) return;

    LLGLMaterialShader* pMatShader = nullptr;
    if (material.isLLGL()) {
        pMatShader = &static_cast<LLGLMaterialShader&>(material.shader());
    }
    if (!pMatShader || !pMatShader->isValid()) return;

    pMatShader->setContext(pContext);
    pMatShader->setCommandBuffer(pCmdBuf);
    if (!pMatShader->bind()) return;

    if (geometry.vertexCount() > 0 && geometry.sizeOfVertex() > 0) {
        pMatShader->updateVertexBuffer(
                geometry.vertexDataAs<float>(),
                static_cast<std::uint32_t>(geometry.vertexCount()),
                static_cast<std::uint32_t>(geometry.sizeOfVertex()));
    }

    if (material.clearUniformsCacheDirty() || !material.isLastModifierOfShader()) {
        material.modifyShader();
        const UniformsCache& cache = material.uniformsCache();
        for (int i = 0; i < cache.count(); i++) {
            int location = material.uniformLocation(i);
            switch (cache.type(i)) {
            case Type::Float:
                pMatShader->setUniformValue(location, cache.get<GLfloat>(i));
                break;
            case Type::Vector2D:
                pMatShader->setUniformValue(location, cache.get<QVector2D>(i));
                break;
            case Type::Vector3D:
                pMatShader->setUniformValue(location, cache.get<QVector3D>(i));
                break;
            case Type::Vector4D:
                pMatShader->setUniformValue(location, cache.get<QVector4D>(i));
                break;
            case Type::Matrix4x4:
                pMatShader->setUniformValue(location, cache.get<QMatrix4x4>(i));
                break;
            case Type::UInt:
                pMatShader->setUniformValue(location, cache.get<GLuint>(i));
                break;
            }
        }
    }

    pMatShader->drawArrays(GL_TRIANGLES, 0, static_cast<int>(geometry.vertexCount()));
    pMatShader->release();
}

#endif

void BaseGeometryNode::renderGL(GeometryNode* pThis,
        Geometry& geometry,
        Material& material) {
    Q_UNUSED(pThis);

#ifndef MIXXX_USE_LLGL
    QOpenGLShaderProgram& shader = material.shader();
    VERIFY_OR_DEBUG_ASSERT(shader.bind()) {
        return;
    }

    glEnable(GL_BLEND);
    glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);

    if (material.clearUniformsCacheDirty() || !material.isLastModifierOfShader()) {
        material.modifyShader();
        const UniformsCache& cache = material.uniformsCache();
        for (int i = 0; i < cache.count(); i++) {
            int location = material.uniformLocation(i);
            switch (cache.type(i)) {
            case Type::UInt:
                shader.setUniformValue(location, cache.get<GLuint>(i));
                break;
            case Type::Float:
                shader.setUniformValue(location, cache.get<GLfloat>(i));
                break;
            case Type::Vector2D:
                shader.setUniformValue(location, cache.get<QVector2D>(i));
                break;
            case Type::Vector3D:
                shader.setUniformValue(location, cache.get<QVector3D>(i));
                break;
            case Type::Vector4D:
                shader.setUniformValue(location, cache.get<QVector4D>(i));
                break;
            case Type::Matrix4x4:
                shader.setUniformValue(location, cache.get<QMatrix4x4>(i));
                break;
            }
        }
    }

    int vertexOffset = 0;
    for (int i = 0; i < geometry.attributeCount(); i++) {
        const Geometry::Attribute& attribute = geometry.attributes()[i];
        int location = material.attributeLocation(i);
        shader.enableAttributeArray(location);
        shader.setAttributeArray(location,
                geometry.vertexDataAs<float>() + vertexOffset,
                attribute.m_tupleSize,
                geometry.sizeOfVertex());
        vertexOffset += attribute.m_tupleSize;
    }

    auto* pTexture = material.texture(1);
    if (pTexture) {
        pTexture->backendTexture()->bind();
    }

    glDrawArrays(toGlDrawingMode(geometry.drawingMode()), 0, geometry.vertexCount());

    if (pTexture) {
        pTexture->backendTexture()->release();
    }

    for (int i = 0; i < geometry.attributeCount(); i++) {
        int location = material.attributeLocation(i);
        shader.disableAttributeArray(location);
    }

    shader.release();
#endif
}

void BaseGeometryNode::resize(int, int) {
    VERIFY_OR_DEBUG_ASSERT(engine() != nullptr) {
        return;
    }
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    pThis->material().setUniform(0, engine()->matrix());
}
