#pragma once

#include <LLGL/LLGL.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/Device.h>
#include <LLGL/CommandBuffer.h>
#include <LLGL/CommandQueue.h>
#include <LLGL/SwapChain.h>
#include <LLGL/Shader.h>
#include <LLGL/PipelineState.h>
#include <LLGL/PipelineLayout.h>
#include <LLGL/Buffer.h>
#include <LLGL/Texture.h>
#include <LLGL/Sampler.h>
#include <LLGL/VertexAttribute.h>
#include <LLGL/Format.h>

#include <QObject>
#include <QMutex>
#include <QWidget>
#include <memory>

namespace rendergraph {

/// LLGLContext manages the LLGL rendering infrastructure.
/// Creates and owns the RenderSystem, Device, SwapChain, and CommandBuffer.
/// Uses OpenGL backend on all platforms for maximum compatibility.
class LLGLContext : public QObject {
    Q_OBJECT
  public:
    explicit LLGLContext(QObject* parent = nullptr);
    ~LLGLContext() override();

    bool initialize(QWidget* pWidget);
    void shutdown();
    void resize(int width, int height);
    void beginFrame(const QColor& clearColor = QColor(0, 0, 0, 255));
    void endFrame();

    bool isValid() const {
        return m_pRenderSystem != nullptr && m_pSwapChain != nullptr;
    }

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

    /// Create a shader with optional profile override.
    /// If profile is null, the context auto-detects based on the active backend.
    LLGL::Shader* createShader(
            LLGL::ShaderType type,
            const char* source,
            size_t sourceSize,
            const char* profile = nullptr);

    LLGL::Buffer* createBuffer(
            size_t size, LLGL::BindFlags::Bits bindFlags,
            const void* initialData = nullptr);
    LLGL::Texture* createTexture(const LLGL::TextureDescriptor& desc,
            const void* initialData = nullptr);
    LLGL::Sampler* createSampler(const LLGL::SamplerDescriptor& desc);
    LLGL::PipelineState* createPipelineState(
            const LLGL::GraphicsPipelineDescriptor& desc);
    LLGL::PipelineLayout* createPipelineLayout(
            const LLGL::PipelineLayoutDescriptor& desc);

    /// Name of the active backend (e.g. "OpenGL", "Metal")
    QString backendName() const;
    /// Whether the backend accepts GLSL shaders
    bool useGLSL() const;

  private:
    bool createRenderSystem();
    bool createSwapChain(QWidget* pWidget);
    void destroySwapChain();
    bool ensureSwapChain();

    mutable QMutex m_mutex;
    LLGL::RenderSystemPtr m_pRenderSystem;
    LLGL::SwapChain* m_pSwapChain = nullptr;
    LLGL::CommandBuffer* m_pCommandBuffer = nullptr;
    LLGL::CommandQueue* m_pCommandQueue = nullptr;
    QWidget* m_pWidget = nullptr;
    bool m_initialized = false;
};

} // namespace rendergraph
