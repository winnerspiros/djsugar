#pragma once

#include <LLGL/Buffer.h>
#include <LLGL/CommandBuffer.h>
#include <LLGL/CommandQueue.h>
#include <LLGL/Format.h>
#include <LLGL/LLGL.h>
#include <LLGL/PipelineLayout.h>
#include <LLGL/PipelineState.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/RenderTarget.h>
#include <LLGL/Sampler.h>
#include <LLGL/Shader.h>
#include <LLGL/ShaderReflection.h>
#include <LLGL/SwapChain.h>
#include <LLGL/Texture.h>
#include <LLGL/VertexAttribute.h>

#include <QMutex>
#include <QString>
#include <QTimer>
#include <QWidget>
#include <QWindow>
#include <memory>

#include "llgl/rendergraph/openglnode.h"

namespace rendergraph {

/// LLGLNode is the base class for all LLGL-based render graph nodes.
/// It replaces BaseOpenGLNode (which uses QOpenGLFunctions) with
/// LLGL's CommandBuffer-based rendering.
class LLGLNode {
  public:
    explicit LLGLNode(LLGLContext* context);
    virtual ~LLGLNode();

    /// Initialize the node: create shaders, pipeline state, buffers.
    virtual void initialize();

    /// Render the node using the LLGL command buffer.
    virtual void render() = 0;

    /// Resize the node.
    virtual void resize(int w, int h);

    /// Preprocess (update vertex data from waveform).
    virtual void preprocess();

    /// Get the LLGL context.
    LLGLContext* context() const {
        return m_pContext;
    }

    /// Get the command buffer.
    LLGL::CommandBuffer* commandBuffer() const;

    /// Get the render system.
    LLGL::RenderSystem* renderSystem() const;

  private:
    LLGLContext* m_pContext;
};

} // namespace rendergraph
