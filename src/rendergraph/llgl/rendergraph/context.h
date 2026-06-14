#pragma once

#include <LLGL/Buffer.h>
#include <LLGL/CommandBuffer.h>
#include <LLGL/CommandQueue.h>
#include <LLGL/Device.h>
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

namespace rendergraph {

/// LLGLContext manages the LLGL rendering infrastructure for a QWidget.
/// It creates and owns the RenderSystem, Device, SwapChain, and CommandBuffer.
class LLGLContext : public QObject {
    Q_OBJECT
  public:
    explicit LLGLContext(QObject* parent = nullptr);
    ~LLGLContext() override;

    /// Initialize LLGL for the given widget. Must be called after the widget
    /// has been shown and has a valid native window handle.
    bool initialize(QWidget* pWidget);

    /// Release all LLGL resources.
    void shutdown();

    /// Resize the swap chain to match the widget's current size.
    void resize(int width, int height);

    /// Begin a new frame: clear the back buffer.
    void beginFrame(const QColor& clearColor = QColor(0, 0, 0, 255));

    /// End the frame: submit command buffer and present.
    void endFrame();

    /// Returns true if the context is fully initialized and ready to render.
    bool isValid() const {
        return m_pRenderSystem != nullptr && m_pSwapChain != nullptr;
    }

    /// Accessors for LLGL objects (valid after initialize()).
    LLGL::RenderSystem* renderSystem() const {
        return m_pRenderSystem.get();
    }
    LLGL::SwapChain* swapChain() const {
        return m_pSwapChain;
    }
    LLGL::CommandBuffer* commandBuffer() const {
        return m_pCommandBuffer;
    }
    LLGL::CommandQueue* commandQueue() const {
        return m_pCommandQueue;
    }

    /// Create an LLGL shader from GLSL source code.
    LLGL::Shader* createShader(LLGL::ShaderType type,
            const char* source,
            size_t sourceSize);

    /// Create a vertex/index/uniform buffer.
    LLGL::Buffer* createBuffer(size_t size,
            LLGL::BindFlags::Bits bindFlags,
            const void* initialData = nullptr);

    /// Create a texture.
    LLGL::Texture* createTexture(const LLGL::TextureDescriptor& desc,
            const void* initialData = nullptr);

    /// Create a sampler.
    LLGL::Sampler* createSampler(const LLGL::SamplerDescriptor& desc);

    /// Create a pipeline state.
    LLGL::PipelineState* createPipelineState(
            const LLGL::GraphicsPipelineDescriptor& desc);

    /// Create a pipeline layout.
    LLGL::PipelineLayout* createPipelineLayout(
            const LLGL::PipelineLayoutDescriptor& desc);

    /// Get the native surface for the current platform.
    LLGL::Surface* surface() const {
        return m_pSurface.get();
    }

  private:
    bool createRenderSystem();
    bool createSwapChain(QWidget* pWidget);
    void destroySwapChain();

    mutable QMutex m_mutex;

    LLGL::RenderSystemPtr m_pRenderSystem;
    LLGL::SwapChain* m_pSwapChain = nullptr;
    LLGL::CommandBuffer* m_pCommandBuffer = nullptr;
    LLGL::CommandQueue* m_pCommandQueue = nullptr;

    QWidget* m_pWidget = nullptr;
    bool m_initialized = false;

    std::shared_ptr<LLGL::Surface> m_pSurface;
};

} // namespace rendergraph
