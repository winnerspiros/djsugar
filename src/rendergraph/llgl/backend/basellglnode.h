#pragma once

#include <LLGL/LLGL.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/Device.h>
#include <LLGL/CommandBuffer.h>
#include <LLGL/CommandQueue.h>
#include <LLGL/SwapChain.h>
#include <LLGL/Shader.h>
#include <LLGL/PipelineState.h>
#include <LLGL/Buffer.h>
#include <LLGL/Texture.h>
#include <LLGL/Sampler.h>
#include <LLGL/RenderTarget.h>
#include <LLGL/VertexAttribute.h>
#include <LLGL/Format.h>
#include <LLGL/PipelineLayout.h>
#include <LLGL/ShaderReflection.h>

#include <QWidget>
#include <QWindow>
#include <QMutex>
#include <QTimer>
#include <QHash>
#include <QString>
#include <memory>

#include "rendergraph/llgl/rendergraph/context.h"

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
    LLGLContext* context() const { return m_pContext; }

    /// Get the command buffer.
    LLGL::CommandBuffer* commandBuffer() const;

    /// Get the render system.
    LLGL::RenderSystem* renderSystem() const;

  protected:
    LLGLContext* m_pContext;
};

} // namespace rendergraph
