#include "context.h"

#include <QDebug>
#include <QGuiApplication>
#include <QScreen>
#include <QWindow>

#if defined(Q_OS_ANDROID)
#include <android/native_window.h>
#include <android/native_window_jni.h>
#include <QJniObject>
#include <QNativeInterface>
#elif defined(Q_OS_WIN32)
#include <windows.h>
#elif defined(Q_OS_MACOS)
#include <Cocoa/Cocoa.h>
#elif defined(Q_OS_LINUX)
#include <X11/Xlib.h>
#endif

namespace rendergraph {

LLGLContext::LLGLContext(QObject* parent)
        : QObject(parent) {
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

    m_initialized = true;
    qDebug() << "LLGLContext: initialized (render system:\""
             << m_pRenderSystem->GetName() << "\")";
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
        return false;
    }

    if (!createSwapChain(m_pWidget)) {
        return false;
    }

    m_pCommandBuffer = m_pRenderSystem->CreateCommandBuffer();
    m_pCommandQueue = m_pRenderSystem->GetCommandQueue();

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

void LLGLContext::beginFrame(const QColor& clearColor) {
    QMutexLocker lock(&m_mutex);

    if (!ensureSwapChain()) {
        return;
    }

    m_pCommandBuffer->Begin();

    LLGL::ClearValue clearValue;
    clearValue.color = {static_cast<float>(clearColor.redF()),
            static_cast<float>(clearColor.greenF()),
            static_cast<float>(clearColor.blueF()),
            static_cast<float>(clearColor.alphaF())};

    m_pCommandBuffer->BeginRenderPass(*m_pSwapChain, &clearValue);
}

void LLGLContext::endFrame() {
    QMutexLocker lock(&m_mutex);

    if (!m_pCommandBuffer || !m_pSwapChain || !m_pCommandQueue) {
        return;
    }

    m_pCommandBuffer->EndRenderPass();
    m_pCommandBuffer->End();

    m_pCommandQueue->Submit(*m_pCommandBuffer);
    m_pSwapChain->Present();
}

bool LLGLContext::createRenderSystem() {
    // Use OpenGL backend on all platforms.
    // LLGL's OpenGL backend works everywhere:
    // - Linux: native OpenGL/EGL
    // - Android: OpenGL ES (via EGL)  
    // - Windows: native OpenGL (WGL)
    // - macOS: NSOpenGLContext (deprecated but functional on macOS 15)
    //
    // All shader code is GLSL which LLGL compiles for OpenGL.
    // This avoids the need for per-backend shader translation.

    const char* backends[] = {"OpenGL"};

    LLGL::Log::RegisterCallbackStd();

    for (const char* backendName : backends) {
        LLGL::RenderSystemDescriptor desc;
        desc.moduleName = backendName;

        m_pRenderSystem = LLGL::RenderSystem::Load(desc);
        if (m_pRenderSystem) {
            qDebug() << "LLGLContext: loaded" << backendName;
            return true;
        }
        qWarning() << "LLGLContext: failed to load" << backendName;
    }

    return false;
}

bool LLGLContext::createSwapChain(QWidget* pWidget) {
    QWindow* pWindow = pWidget->windowHandle();
    if (!pWindow) {
        return false;
    }

    LLGL::SwapChainDescriptor swapChainDesc;
    swapChainDesc.resolution.width = static_cast<std::uint32_t>(
            pWidget->width() * pWindow->devicePixelRatio());
    swapChainDesc.resolution.height = static_cast<std::uint32_t>(
            pWidget->height() * pWindow->devicePixelRatio());
    swapChainDesc.colorFormat = LLGL::Format::RGBA8UNorm;
    swapChainDesc.depthStencilFormat = LLGL::Format::D24UNormS8UInt;
    swapChainDesc.samples = 1;
    swapChainDesc.swapBuffers = 2;

    m_pSwapChain = m_pRenderSystem->CreateSwapChain(swapChainDesc);

    if (!m_pSwapChain) {
        qWarning() << "LLGLContext: failed to create swap chain";
        return false;
    }

    qDebug() << "LLGLContext: swap chain"
             << swapChainDesc.resolution.width << "x"
             << swapChainDesc.resolution.height;
    return true;
}

void LLGLContext::destroySwapChain() {
    if (m_pSwapChain && m_pRenderSystem) {
        m_pRenderSystem->Release(*m_pSwapChain);
        m_pSwapChain = nullptr;
    }
}

LLGL::Shader* LLGLContext::createShader(
        LLGL::ShaderType type, const char* source, size_t sourceSize,
        const char* profile) {
    QMutexLocker lock(&m_mutex);
    if (!m_pRenderSystem) {
        return nullptr;
    }

    LLGL::ShaderDescriptor desc;
    desc.type = type;
    desc.source = source;
    desc.sourceSize = sourceSize;
    desc.entryPoint = "main";
    desc.profile = profile;

    return m_pRenderSystem->CreateShader(desc);
}

LLGL::Buffer* LLGLContext::createBuffer(
        size_t size, LLGL::BindFlags::Bits bindFlags, const void* initialData) {
    QMutexLocker lock(&m_mutex);
    if (!m_pRenderSystem) {
        return nullptr;
    }

    LLGL::BufferDescriptor desc;
    desc.size = size;
    desc.bindFlags = bindFlags;

    return m_pRenderSystem->CreateBuffer(desc, initialData);
}

LLGL::Texture* LLGLContext::createTexture(
        const LLGL::TextureDescriptor& desc, const void* initialData) {
    QMutexLocker lock(&m_mutex);
    if (!m_pRenderSystem) {
        return nullptr;
    }
    return m_pRenderSystem->CreateTexture(desc, initialData);
}

LLGL::Sampler* LLGLContext::createSampler(const LLGL::SamplerDescriptor& desc) {
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

LLGL::PipelineLayout* LLGLContext::createPipelineLayout(
        const LLGL::PipelineLayoutDescriptor& desc) {
    QMutexLocker lock(&m_mutex);
    if (!m_pRenderSystem) {
        return nullptr;
    }
    return m_pRenderSystem->CreatePipelineLayout(desc);
}

QString LLGLContext::backendName() const {
    return m_pRenderSystem ? QString::fromUtf8(m_pRenderSystem->GetName()) : QString();
}

bool LLGLContext::useGLSL() const {
    if (!m_pRenderSystem) return true;
    const char* name = m_pRenderSystem->GetName();
    return (strcmp(name, "OpenGL") == 0 || strcmp(name, "OpenGLES") == 0);
}

} // namespace rendergraph
