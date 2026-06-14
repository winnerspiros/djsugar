#include "basellglnode.h"

#include "rendergraph/llgl/rendergraph/context.h"

namespace rendergraph {

LLGLNode::LLGLNode(LLGLContext* context)
        : m_pContext(context) {
}

LLGLNode::~LLGLNode() {
}

void LLGLNode::initialize() {
    // Base initialization — subclasses override
}

void LLGLNode::resize(int w, int h) {
    Q_UNUSED(w);
    Q_UNUSED(h);
}

void LLGLNode::preprocess() {
    // Base preprocess — subclasses override
}

LLGL::CommandBuffer* LLGLNode::commandBuffer() const {
    return m_pContext ? m_pContext->commandBuffer() : nullptr;
}

LLGL::RenderSystem* LLGLNode::renderSystem() const {
    return m_pContext ? m_pContext->renderSystem() : nullptr;
}

} // namespace rendergraph
