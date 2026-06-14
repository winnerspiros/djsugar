#include "backend/basegeometrynode.h"

#ifdef MIXXX_USE_LLGL

#include <QOpenGLTexture>

#include "backend/shadercache.h"
#include "rendergraph/engine.h"
#include "rendergraph/geometrynode.h"
#include "rendergraph/texture.h"
#include "rendergraph/llgl/rendergraph/context.h"

using namespace rendergraph;

void BaseLLGLGeometryNode::initialize() {
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    pThis->material().setShader(ShaderCache::getShaderForMaterial(&pThis->material()));
    pThis->material().setUniform(0, engine()->matrix());
}

void BaseLLGLGeometryNode::render() {
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    Geometry& geometry = pThis->geometry();
    Material& material = pThis->material();

    if (geometry.vertexCount() == 0) {
        return;
    }

    // Get the LLGL context from the widget
    auto* pLLGLNode = dynamic_cast<BaseLLGLNode*>(this);
    if (!pLLGLNode) {
        return;
    }

    auto* pContext = pLLGLNode->context();
    auto* pCmdBuf = pContext ? pContext->commandBuffer() : nullptr;
    if (!pCmdBuf) {
        return;
    }

    // Bind shader pipeline state
    QOpenGLShaderProgram& shader = material.shader();
    // For LLGL, we need to set the pipeline state instead of binding a shader program
    // TODO: Replace with LLGL pipeline state binding

    // Set blend state via LLGL
    // LLGL handles blend state through pipeline state, not individual calls

    // Set uniforms
    if (material.clearUniformsCacheDirty() || !material.isLastModifierOfShader()) {
        material.modifyShader();
        const UniformsCache& cache = material.uniformsCache();
        for (int i = 0; i < cache.count(); i++) {
            int location = material.uniformLocation(i);
            switch (cache.type(i)) {
            case Type::Float:
                // TODO: Set uniform via LLGL
                break;
            case Type::Matrix4x4:
                // TODO: Set uniform via LLGL
                break;
            default:
                break;
            }
        }
    }

    // Set vertex attributes and draw
    // TODO: Use LLGL command buffer for drawing
    Q_UNUSED(geometry);
}

void BaseLLGLGeometryNode::resize(int, int) {
    VERIFY_OR_DEBUG_ASSERT(engine() != nullptr) {
        return;
    }
    GeometryNode* pThis = static_cast<GeometryNode*>(this);
    pThis->material().setUniform(0, engine()->matrix());
}

#endif // MIXXX_USE_LLGL
