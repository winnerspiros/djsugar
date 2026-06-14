#pragma once

#ifdef MIXXX_USE_LLGL

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
#include <LLGL/Surface.h>
#include <LLGL/VertexAttribute.h>
#include <LLGL/Format.h>

#include <QWidget>
#include <QWindow>
#include <QMutex>
#include <memory>

/// LLGLContext manages the LLGL rendering infrastructure for a QWidget.
///
/// It creates and owns:
/// - LLGL::RenderSystem (the backend, e.g. OpenGL, Vulkan, Metal)
/// - LLGL::SwapChain (presentation surface tied to the widget's native window)
/// - LLGL::CommandBuffer (for recording draw commands)
/// - LLGL::CommandQueue (for submission)
///
/// Platform-specific surface creation:
/// - Android: Uses ANativeWindow from Qt's native window handle
/// - Windows: Uses HWND
/// - macOS: Uses NSView
/// - Linux/X11: Uses XWindow
///
/// Thread safety: All public methods lock m_mutex. LLGL requires that
/// command buffer recording and submission happen on the same thread.

class LLGLContext {
  public:
    LLGLContext();
    ~LLGLContext();

    /// Initialize LLGL for the given widget. Must be called after the widget
    /// has been shown and has a valid native window handle.
    bool initialize(QWidget* pWidget);

    /// Release all LLGL resources.
    void shutdown();

    /// Resize the swap chain to match the widget's current size.
    void resize(int width, int height);

    /// Begin a new frame: start command buffer recording.
    void beginFrame();

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

    /// Create a vertex buffer.
    LLGL::Buffer* createBuffer(size_t size,
            LLGL::BindFlags::Bits bindFlags,
            const void* initialData = nullptr);

    /// Create a texture.
    LLGL::Texture* createTexture(const LLGL::TextureDescriptor& desc,
            const void* initialData = nullptr);

    /// Create a sampler.
    LLGL::Sampler* createSampler(const LLGL::SamplerDescriptor& desc);

    /// Create a pipeline state.
    LLGL::PipelineState* createPipelineState(const LLGL::GraphicsPipelineDescriptor& desc);

  private:
    bool createRenderSystem();
    bool createSwapChain(QWidget* pWidget);
    void destroySwapChain();

    mutable QMutex m_mutex;

    LLGL::RenderSystemPtr m_pRenderSystem;
    LLGL::SwapChain* m_pSwapChain;
    LLGL::CommandBuffer* m_pCommandBuffer;
    LLGL::CommandQueue* m_pCommandQueue;

    QWidget* m_pWidget;
    bool m_initialized;

    // Keep surface alive for the lifetime of the swap chain
    std::shared_ptr<LLGL::Surface> m_pSurface;
};

#endif // MIXXX_USE_LLGL
