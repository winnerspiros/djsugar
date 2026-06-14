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

    LLGL::Shader* createShader(
            LLGL::ShaderType type, const char* source, std::uint32_t sourceSize);
    LLGL::Buffer* createBuffer(
            std::uint64_t size, LLGL::BindFlags::Bits bindFlags,
            const void* initialData = nullptr,
            const LLGL::VertexAttribute* attribs = nullptr,
            std::uint32_t numAttribs = 0);
    LLGL::Texture* createTexture(const LLGL::TextureDescriptor& desc,
            const void* initialData = nullptr);
    LLGL::Sampler* createSampler(const LLGL::SamplerDescriptor& desc);
    LLGL::PipelineState* createPipelineState(
            const LLGL::GraphicsPipelineDescriptor& desc);
    LLGL::PipelineLayout* createPipelineLayout(
            const LLGL::PipelineLayoutDescriptor& desc);

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
    std::shared_ptr<LLGL::Surface> m_pSurface;
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
