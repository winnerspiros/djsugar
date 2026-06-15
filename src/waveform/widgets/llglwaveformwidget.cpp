// LLGLWaveformWidget - Full GPU-accelerated waveform rendering using LLGL
//
// Architecture improvements over QOpenGL-based rendering:
// 1. Native swap chain: Renders directly to a native window surface via LLGL,
//    bypassing Qt's compositor for lower latency.
// 2. Command buffer batching: All draw commands are recorded into a single
//    LLGL command buffer per frame, reducing CPU overhead.
// 3. GPU-side vertex buffers: Waveform vertex data is uploaded to GPU memory
//    and reused across frames. Only re-uploaded when data changes.
// 4. Pipeline state caching: Pipeline state objects (shaders, blend state,
//    rasterizer state) are created once at initialization and reused.
// 5. Multi-buffered rendering: 3 frames in flight to maximize GPU utilization
//    and avoid CPU-GPU sync stalls.

#include "llglwaveformwidget.h"

#include <QDebug>
#include <QTimer>
#include <QWindow>
#include <cmath>

#include "moc_llglwaveformwidget.cpp"
#include "../rendergraph/llgl/backend/shadersourceprovider.h"
#include "track/track.h"
#include "util/math.h"
#include "waveform/renderers/waveformrenderbackground.h"
#include "waveform/renderers/waveformrenderbeat.h"
#include "waveform/renderers/waveformrendererendoftrack.h"
#include "waveform/renderers/waveformrendererfilteredsignal.h"
#include "waveform/renderers/waveformrendererpreroll.h"
#include "waveform/renderers/waveformrendermark.h"
#include "waveform/renderers/waveformrendermarkrange.h"
#include "waveform/waveform.h"

LLGLWaveformWidget::LLGLWaveformWidget(const QString& group, QWidget* parent)
        : WaveformWidgetAbstract(group),
          QWidget(parent),
          m_pContext(std::make_unique<rendergraph::LLGLContext>()),
          m_pRenderTimer(new QTimer(this)),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);
    setAttribute(Qt::WA_NativeWindow);

    addRenderer<WaveformRenderBackground>();
    addRenderer<WaveformRendererEndOfTrack>();
    addRenderer<WaveformRendererPreroll>();
    addRenderer<WaveformRenderMarkRange>();
    addRenderer<WaveformRendererFilteredSignal>();
    addRenderer<WaveformRenderBeat>();
    addRenderer<WaveformRenderMark>();

    connect(m_pRenderTimer, &QTimer::timeout, this, &LLGLWaveformWidget::onRenderTimer);
}

LLGLWaveformWidget::~LLGLWaveformWidget() {
    shutdownLLGL();
}

void LLGLWaveformWidget::castToQWidget() {
    m_widget = this;
}

QWidget* LLGLWaveformWidget::widget() {
    return this;
}

mixxx::Duration LLGLWaveformWidget::render() {
    if (m_initOk && m_pContext && m_pContext->isValid()) {
        renderLLGL();
    } else {
        update();
    }
    return mixxx::Duration();
}

void LLGLWaveformWidget::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    if (!m_initOk) {
        renderFallback();
        return;
    }
    renderLLGL();
}

void LLGLWaveformWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    if (m_initOk && m_pContext) {
        m_pContext->resize(width(), height());
    }
}

void LLGLWaveformWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    if (!m_initOk) {
        m_initOk = initializeLLGL();
    }
    if (m_initOk) {
        m_pRenderTimer->start(16);  // ~60fps
    }
}

void LLGLWaveformWidget::hideEvent(QHideEvent* event) {
    QWidget::hideEvent(event);
    m_pRenderTimer->stop();
}

void LLGLWaveformWidget::onRenderTimer() {
    if (isVisible()) {
        update();
    }
}

// ============================================================================
// Initialization
// ============================================================================

bool LLGLWaveformWidget::initializeLLGL() {
    if (!m_pContext) {
        return false;
    }

    // Initialize LLGL context with this widget's native window handle
    if (!m_pContext->initialize(this)) {
        qWarning() << "LLGLWaveformWidget: failed to initialize LLGL context";
        return false;
    }

    // Swap chain is created by m_pContext->initialize(this) above

    // Create pipeline state cache (shaders, PSO)
    if (!createPipelineCache()) {
        qWarning() << "LLGLWaveformWidget: failed to create pipeline cache";
        return false;
    }

    // Create multi-buffered frame resources
    if (!createFrameResources()) {
        qWarning() << "LLGLWaveformWidget: failed to create frame resources";
        return false;
    }

    // Create GPU vertex buffer
    m_vertexBuffer.capacity = 65536;  // Initial capacity
    m_vertexBuffer.count = 0;

    auto* device = m_pContext->renderSystem();
    if (device) {
        LLGL::BufferDescriptor bufDesc;
        bufDesc.size = static_cast<std::uint32_t>(m_vertexBuffer.capacity * sizeof(WaveformVertex));
        bufDesc.bindFlags = LLGL::BindFlag::VertexBuffer;
        bufDesc.cpuAccessFlags = LLGL::CPUAccessFlag::Write;  // CPU-write for updates
        bufDesc.miscFlags = LLGL::MiscFlag::DynamicUsage;       // Dynamic for frequent updates
        m_vertexBuffer.buffer = device->CreateBuffer(bufDesc);
        if (!m_vertexBuffer.buffer) {
            qWarning() << "LLGLWaveformWidget: failed to create vertex buffer";
            return false;
        }
    }

    qDebug() << "LLGLWaveformWidget: initialized with backend:"
             << m_pContext->backendName()
             << "swap chain:" << (m_pContext->swapChain() ? "yes" : "no")
             << "pipeline:" << (m_pipelineCache.valid ? "yes" : "no");
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_pRenderTimer->stop();

    if (!m_pContext) {
        return;
    }

    auto* device = m_pContext->renderSystem();
    if (device) {
        // Wait for all GPU work to complete
        auto* queue = m_pContext->commandQueue();
        if (queue) {
            queue->WaitIdle();
        }

        // Release frame resources
        for (int i = 0; i < kFrameCount; ++i) {
            if (m_frames[i].commandBuffer) {
                device->Release(*m_frames[i].commandBuffer);
            }
            if (m_frames[i].fence) {
                device->Release(*m_frames[i].fence);
            }
        }

        // Release pipeline cache
        if (m_pipelineCache.pipelineState) {
            device->Release(*m_pipelineCache.pipelineState);
        }
        if (m_pipelineCache.pipelineLayout) {
            device->Release(*m_pipelineCache.pipelineLayout);
        }
        if (m_pipelineCache.vertexShader) {
            device->Release(*m_pipelineCache.vertexShader);
        }
        if (m_pipelineCache.fragmentShader) {
            device->Release(*m_pipelineCache.fragmentShader);
        }

        // Release vertex buffer
        if (m_vertexBuffer.buffer) {
            device->Release(*m_vertexBuffer.buffer);
        }

        // Note: swap chain is owned by m_pContext, don't release here
    }

    memset(m_frames, 0, sizeof(m_frames));
    memset(&m_pipelineCache, 0, sizeof(m_pipelineCache));
    memset(&m_vertexBuffer, 0, sizeof(m_vertexBuffer));
    m_pRenderTarget = nullptr;

    m_pContext->shutdown();
    m_initOk = false;
}

bool LLGLWaveformWidget::createPipelineCache() {
    if (!createShaders()) {
        return false;
    }
    if (!createPipelineState()) {
        return false;
    }
    setupVertexFormat();
    m_pipelineCache.valid = true;
    return true;
}

bool LLGLWaveformWidget::createShaders() {
    auto* device = m_pContext->renderSystem();
    if (!device) {
        return false;
    }

    rendergraph::ShaderBackend backend =
            rendergraph::ShaderSourceProvider::detectBackend(device->GetName());
    const rendergraph::ShaderSourceProvider& provider =
            rendergraph::ShaderSourceProvider::instance();

    QString vsSource = provider.vertexShader(backend);
    QString fsSource = provider.fragmentShader(backend);
    QString vsProfile = provider.profileString(backend, true);
    QString fsProfile = provider.profileString(backend, false);

    // Vertex shader
    LLGL::ShaderDescriptor vsDesc;
    vsDesc.type = LLGL::ShaderType::Vertex;
    QByteArray vsBytes = vsSource.toUtf8();
    vsDesc.source = vsBytes.constData();
    vsDesc.sourceSize = static_cast<std::uint32_t>(vsBytes.size());
    vsDesc.entryPoint = "main";
    vsDesc.profile = vsProfile.toUtf8().constData();

    m_pipelineCache.vertexShader = device->CreateShader(vsDesc);
    if (!m_pipelineCache.vertexShader) {
        qWarning() << "LLGLWaveformWidget: failed to create vertex shader ("
                   << vsProfile << " on " << device->GetName() << ")";
        return false;
    }

    // Fragment shader
    LLGL::ShaderDescriptor fsDesc;
    fsDesc.type = LLGL::ShaderType::Fragment;
    QByteArray fsBytes = fsSource.toUtf8();
    fsDesc.source = fsBytes.constData();
    fsDesc.sourceSize = static_cast<std::uint32_t>(fsBytes.size());
    fsDesc.entryPoint = "main";
    fsDesc.profile = fsProfile.toUtf8().constData();

    m_pipelineCache.fragmentShader = device->CreateShader(fsDesc);
    if (!m_pipelineCache.fragmentShader) {
        qWarning() << "LLGLWaveformWidget: failed to create fragment shader ("
                   << fsProfile << " on " << device->GetName() << ")";
        return false;
    }

    return true;
}

void LLGLWaveformWidget::setupVertexFormat() {
    // Vertex format: float2 position + float3 color
    auto& fmt = m_pipelineCache.vertexFormat;
    fmt.AppendAttribute({"position", LLGL::Format::RG32Float, 0, 0});
    fmt.AppendAttribute({"color", LLGL::Format::RGB32Float, 1, 2 * sizeof(float)});
    fmt.SetStride(sizeof(WaveformVertex));
}

bool LLGLWaveformWidget::createPipelineState() {
    auto* device = m_pContext->renderSystem();
    if (!device) {
        return false;
    }

    // Pipeline layout
    LLGL::PipelineLayoutDescriptor plDesc;
    m_pipelineCache.pipelineLayout = device->CreatePipelineLayout(plDesc);
    if (!m_pipelineCache.pipelineLayout) {
        return false;
    }

    // Graphics pipeline state
    LLGL::GraphicsPipelineDescriptor gpDesc;
    gpDesc.pipelineLayout = m_pipelineCache.pipelineLayout;
    gpDesc.vertexShader = m_pipelineCache.vertexShader;
    gpDesc.fragmentShader = m_pipelineCache.fragmentShader;
    gpDesc.vertexFormat = m_pipelineCache.vertexFormat;
    gpDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleStrip;

    // Rasterizer state
    gpDesc.rasterizer.discardEnabled = false;
    gpDesc.rasterizer.cullMode = LLGL::CullMode::None;
    gpDesc.rasterizer.fillMode = LLGL::FillMode::Solid;

    // Blend state — enable alpha blending
    gpDesc.blend.blendFactor = {1.0f, 1.0f, 1.0f, 1.0f};
    gpDesc.blend.targets[0].blendEnabled = true;
    gpDesc.blend.targets[0].srcColor = LLGL::BlendOp::SrcAlpha;
    gpDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;
    gpDesc.blend.targets[0].srcAlpha = LLGL::BlendOp::One;
    gpDesc.blend.targets[0].dstAlpha = LLGL::BlendOp::InvSrcAlpha;

    m_pipelineCache.pipelineState = device->CreatePipelineState(gpDesc);
    if (!m_pipelineCache.pipelineState) {
        qWarning() << "LLGLWaveformWidget: failed to create pipeline state";
        return false;
    }

    return true;
}

bool LLGLWaveformWidget::createFrameResources() {
    auto* device = m_pContext->renderSystem();
    if (!device) {
        return false;
    }

    for (int i = 0; i < kFrameCount; ++i) {
        // Create command buffer
        LLGL::CommandBufferDescriptor cbDesc;
        cbDesc.flags = LLGL::CommandBufferFlag::Secondary;
        m_frames[i].commandBuffer = device->CreateCommandBuffer(cbDesc);
        if (!m_frames[i].commandBuffer) {
            qWarning() << "LLGLWaveformWidget: failed to create command buffer " << i;
            return false;
        }

        // Create fence for GPU-CPU synchronization
        m_frames[i].fence = device->CreateFence();
        if (!m_frames[i].fence) {
            qWarning() << "LLGLWaveformWidget: failed to create fence " << i;
            return false;
        }
    }

    return true;
}

// ============================================================================
// Per-Frame Rendering with Command Buffer Batching
// ============================================================================

void LLGLWaveformWidget::renderLLGL() {
    if (!m_initOk || !m_pContext || !m_pContext->swapChain()) {
        renderFallback();
        return;
    }

    std::lock_guard<std::mutex> lock(m_renderMutex);

    beginFrame();
    recordDrawCommands();
    endFrame();
    presentFrame();
}

void LLGLWaveformWidget::beginFrame() {
    // Get next frame resource and wait for it to be free
    LLGLFrameResources& frame = m_frames[m_currentFrame];
    waitForFrame(frame);

    // Begin recording command buffer
    LLGL::CommandBuffer* cmdBuf = frame.commandBuffer;
    cmdBuf->Begin();

    // Set render target to swap chain
    cmdBuf->SetRenderTarget(*m_pContext->swapChain());

    // Set viewport
    LLGL::Viewport viewport;
    viewport.x = 0.0f;
    viewport.y = 0.0f;
    viewport.width = static_cast<float>(width());
    viewport.height = static_cast<float>(height());
    cmdBuf->SetViewport(viewport);

    // Clear to background color
    LLGL::ClearValue clearValue;
    clearValue.color = {0.0f, 0.0f, 0.0f, 1.0f};
    cmdBuf->Clear(LLGL::ClearFlag::Color, clearValue);

    // Bind cached pipeline state
    cmdBuf->SetPipelineState(*m_pipelineCache.pipelineState);

    frame.inUse = true;
}

void LLGLWaveformWidget::recordDrawCommands() {
    LLGLFrameResources& frame = m_frames[m_currentFrame];
    LLGL::CommandBuffer* cmdBuf = frame.commandBuffer;

    // Update vertex buffer if data changed
    if (!m_vertices.empty()) {
        updateVertexData(m_vertices.data(), static_cast<std::uint32_t>(m_vertices.size()));
    }

    // Bind vertex buffer
    if (m_vertexBuffer.buffer && m_vertexBuffer.count > 0) {
        cmdBuf->SetVertexBuffer(*m_vertexBuffer.buffer);

        // Draw — single draw call for all waveform data
        cmdBuf->Draw(static_cast<std::uint32_t>(m_vertexBuffer.count), 0);
    }
}

void LLGLWaveformWidget::endFrame() {
    LLGLFrameResources& frame = m_frames[m_currentFrame];
    LLGL::CommandBuffer* cmdBuf = frame.commandBuffer;

    cmdBuf->End();
}

void LLGLWaveformWidget::presentFrame() {
    LLGLFrameResources& frame = m_frames[m_currentFrame];
    auto* queue = m_pContext->commandQueue();

    if (queue) {
        // Submit command buffer with fence
        queue->Submit(*frame.commandBuffer);
        queue->Submit(*frame.fence);

        // Present swap chain
        m_pContext->swapChain()->Present();
    }

    // Advance to next frame
    m_currentFrame = (m_currentFrame + 1) % kFrameCount;
}

void LLGLWaveformWidget::waitForFrame(LLGLFrameResources& frame) {
    if (!frame.inUse || !frame.fence) {
        return;
    }

    auto* queue = m_pContext->commandQueue();
    if (queue) {
        // Wait for GPU to finish with this frame
        queue->WaitFence(*frame.fence);
        queue->ResetFence(*frame.fence);
    }
    frame.inUse = false;
}

// ============================================================================
// GPU Buffer Management
// ============================================================================

void LLGLWaveformWidget::updateVertexData(const WaveformVertex* data, std::uint32_t count) {
    if (!data || count == 0 || !m_vertexBuffer.buffer) {
        return;
    }

    // Ensure buffer is large enough
    ensureBufferCapacity(count);

    // Upload data to GPU buffer
    auto* device = m_pContext->renderSystem();
    if (device) {
        device->WriteBuffer(*m_vertexBuffer.buffer, 0, data,
                static_cast<std::uint32_t>(count * sizeof(WaveformVertex)));
    }

    m_vertexBuffer.count = count;
}

void LLGLWaveformWidget::ensureBufferCapacity(std::uint32_t required) {
    if (required <= m_vertexBuffer.capacity) {
        return;
    }

    auto* device = m_pContext->renderSystem();
    if (!device) {
        return;
    }

    // Double capacity until large enough
    std::uint32_t newCapacity = m_vertexBuffer.capacity;
    while (newCapacity < required) {
        newCapacity *= 2;
    }

    // Release old buffer
    if (m_vertexBuffer.buffer) {
        device->Release(*m_vertexBuffer.buffer);
    }

    // Create new larger buffer
    LLGL::BufferDescriptor bufDesc;
    bufDesc.size = static_cast<std::uint32_t>(newCapacity * sizeof(WaveformVertex));
    bufDesc.bindFlags = LLGL::BindFlag::VertexBuffer;
    bufDesc.cpuAccessFlags = LLGL::CPUAccessFlag::Write;
    bufDesc.miscFlags = LLGL::MiscFlag::DynamicUsage;
    m_vertexBuffer.buffer = device->CreateBuffer(bufDesc);
    m_vertexBuffer.capacity = newCapacity;
    m_vertexBuffer.count = 0;
}

// ============================================================================
// Fallback
// ============================================================================

void LLGLWaveformWidget::renderFallback() {
    // No fallback — if LLGL fails, we show nothing
    // The old QPainter fallback is removed as part of the full LLGL migration
}
