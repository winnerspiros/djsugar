#include "backend/basegeometrynode.h"

#include <stdexcept>

#include "backend/shadercache.h"
#include "rendergraph/engine.h"
#include "rendergraph/geometrynode.h"
#include "rendergraph/texture.h"
#include "rendergraph/llgl/rendergraph/context.h"

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
#endif // MIXXX_USE_LLGL

void BaseGeometryNode::renderGL(GeometryNode*, Geometry&, Material&) {
    // Not used when LLGL is enabled — kept for link compatibility
}

void BaseGeometryNode::resize(int, int) {
    VERIFY_OR_DEBUG_ASSERT(engine() != nullptr) {
        return;
    }
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    pThis->material().setUniform(0, engine()->matrix());
}
