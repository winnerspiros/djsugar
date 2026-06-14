#include "llglcontext.h"

#ifdef MIXXX_USE_LLGL

#include <QDebug>
#include <QWindow>

LLGLContext::LLGLContext()
        : m_pSwapChain(nullptr),
          m_pCommandBuffer(nullptr),
          m_pCommandQueue(nullptr),
          m_pWidget(nullptr),
          m_initialized(false) {
}

LLGLContext::~LLGLContext() {
    shutdown();
}

bool LLGLContext::initialize(QWidget* pWidget) {
    QMutexLocker lock(&m_mutex);

    if (m_initialized) {
        return true;
    }

    m_pWidget = pWidget;

    if (!createRenderSystem()) {
        qWarning() << "LLGLContext: failed to create render system";
        return false;
    }

    // Don't create swap chain yet — wait until the widget is shown
    // and has a valid native window handle.
    m_initialized = true;
    qDebug() << "LLGLContext: initialized (render system loaded)";
    return true;
}

bool LLGLContext::ensureSwapChain() {
    QMutexLocker lock(&m_mutex);

    if (!m_initialized || !m_pRenderSystem) {
        return false;
    }

    if (m_pSwapChain) {
        return true;
    }

    if (!m_pWidget) {
        return false;
    }

    QWindow* pWindow = m_pWidget->windowHandle();
    if (!pWindow) {
        // Widget doesn't have a native window yet — defer
        return false;
    }

    // Create swap chain with the widget's native window
    LLGL::SwapChainDescriptor swapChainDesc;
    swapChainDesc.resolution.width = static_cast<std::uint32_t>(
            m_pWidget->width() * pWindow->devicePixelRatio());
    swapChainDesc.resolution.height = static_cast<std::uint32_t>(
            m_pWidget->height() * pWindow->devicePixelRatio());
    swapChainDesc.colorFormat = LLGL::Format::RGBA8UNorm;
    swapChainDesc.depthStencilFormat = LLGL::Format::D24UNormS8UInt;
    swapChainDesc.samples = 1;
    swapChainDesc.swapBuffers = 2;

    // Let LLGL create its own surface from the native window
    m_pSwapChain = m_pRenderSystem->CreateSwapChain(swapChainDesc);

    if (!m_pSwapChain) {
        qWarning() << "LLGLContext: failed to create swap chain";
        return false;
    }

    qDebug() << "LLGLContext: swap chain created"
             << swapChainDesc.resolution.width << "x"
             << swapChainDesc.resolution.height;
    return true;
}

void LLGLContext::shutdown() {
    QMutexLocker lock(&m_mutex);

    if (!m_initialized) {
        return;
    }

    if (m_pCommandQueue) {
        m_pCommandQueue->WaitIdle();
    }

    destroySwapChain();

    m_pCommandBuffer = nullptr;
    m_pCommandQueue = nullptr;

    if (m_pRenderSystem) {
        LLGL::RenderSystem::Unload(std::move(m_pRenderSystem));
    }

    m_pWidget = nullptr;
    m_initialized = false;

    qDebug() << "LLGLContext: shut down";
}

void LLGLContext::resize(int width, int height) {
    QMutexLocker lock(&m_mutex);

    if (!m_pSwapChain) {
        return;
    }

    QWindow* pWindow = m_pWidget ? m_pWidget->windowHandle() : nullptr;
    qreal dpr = pWindow ? pWindow->devicePixelRatio() : 1.0;

    m_pSwapChain->ResizeBuffers(
            static_cast<std::uint32_t>(width * dpr),
            static_cast<std::uint32_t>(height * dpr));
}

void LLGLContext::beginFrame() {
    // Command buffer recording is done per-draw in endFrame()
}

void LLGLContext::endFrame() {
    QMutexLocker lock(&m_mutex);

    if (!m_pCommandQueue || !m_pSwapChain) {
        return;
    }

    m_pCommandQueue->Submit(*m_pCommandBuffer);
    m_pSwapChain->Present();
}

bool LLGLContext::createRenderSystem() {
    // Determine the best backend for this platform
    const char* backendName = nullptr;

#if defined(Q_OS_ANDROID)
    backendName = "OpenGL"; // OpenGL ES via EGL
#elif defined(Q_OS_MACOS)
    backendName = "Metal";
#elif defined(Q_OS_WIN32)
    backendName = "Direct3D11";
#else
    // Linux: prefer OpenGL
    backendName = "OpenGL";
#endif

    LLGL::Log::RegisterCallbackStd();

    m_pRenderSystem = LLGL::RenderSystem::Load(backendName);

    if (!m_pRenderSystem) {
        qWarning() << "LLGLContext: failed to load render system:"
                   << backendName;
        // Try OpenGL as fallback
        if (strcmp(backendName, "OpenGL") != 0) {
            qDebug() << "LLGLContext: trying OpenGL as fallback";
            m_pRenderSystem = LLGL::RenderSystem::Load("OpenGL");
        }
        if (!m_pRenderSystem) {
            qWarning() << "LLGLContext: all backends failed";
            return false;
        }
    }

    qDebug() << "LLGLContext: render system loaded:"
             << m_pRenderSystem->GetName();
    return true;
}

void LLGLContext::destroySwapChain() {
    if (m_pSwapChain && m_pRenderSystem) {
        m_pRenderSystem->Release(*m_pSwapChain);
        m_pSwapChain = nullptr;
    }
}

LLGL::Shader* LLGLContext::createShader(
        LLGL::ShaderType type, const char* source, size_t sourceSize) {
    QMutexLocker lock(&m_mutex);

    if (!m_pRenderSystem) {
        return nullptr;
    }

    LLGL::ShaderDescriptor shaderDesc;
    shaderDesc.type = type;
    shaderDesc.source = source;
    shaderDesc.sourceSize = sourceSize;
    shaderDesc.entryPoint = "main";
    shaderDesc.profile = nullptr;

    return m_pRenderSystem->CreateShader(shaderDesc);
}

LLGL::Buffer* LLGLContext::createBuffer(
        size_t size, LLGL::BindFlags::Bits bindFlags, const void* initialData) {
    QMutexLocker lock(&m_mutex);

    if (!m_pRenderSystem) {
        return nullptr;
    }

    LLGL::BufferDescriptor bufferDesc;
    bufferDesc.size = size;
    bufferDesc.bindFlags = bindFlags;

    return m_pRenderSystem->CreateBuffer(bufferDesc, initialData);
}

LLGL::Texture* LLGLContext::createTexture(
        const LLGL::TextureDescriptor& desc, const void* initialData) {
    QMutexLocker lock(&m_mutex);

    if (!m_pRenderSystem) {
        return nullptr;
    }

    return m_pRenderSystem->CreateTexture(desc, initialData);
}

LLGL::Sampler* LLGLContext::createSampler(
        const LLGL::SamplerDescriptor& desc) {
    QMutexLocker lock(&m_mutex);

    if (!m_pRenderSystem) {
        return nullptr;
    }

    return m_pRenderSystem->CreateSampler(desc);
}

LLGL::PipelineState* LLGLContext::createPipelineState(
        const LLGL::GraphicsPipelineDescriptor& desc) {
    QMutexLocker lock(&m_mutex);

    if (!m_pRenderSystem) {
        return nullptr;
    }

    return m_pRenderSystem->CreatePipelineState(desc);
}

#endif // MIXXX_USE_LLGL
