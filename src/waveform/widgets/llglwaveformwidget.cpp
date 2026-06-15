// LLGLWaveformWidget - Full GPU-accelerated waveform rendering using LLGL
//
// Architecture:
// - Uses LLGL swap chain for native surface rendering
// - GPU-side vertex buffers with dynamic upload
// - Pipeline state caching: PSOs created once at init, reused every frame
// - No QPainter fallback — LLGL is the only rendering path

#include "llglwaveformwidget.h"

#include <QDebug>
#include <QTimer>
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

    // Create pipeline state cache (shaders, PSO)
    if (!createPipelineCache()) {
        qWarning() << "LLGLWaveformWidget: failed to create pipeline cache";
        return false;
    }

    // Create GPU vertex buffer
    m_vertexBuffer.capacity = 65536;
    m_vertexBuffer.count = 0;

    auto* device = m_pContext->renderSystem();
    if (device) {
        LLGL::BufferDescriptor bufDesc;
        bufDesc.size = static_cast<std::uint32_t>(m_vertexBuffer.capacity * sizeof(WaveformVertex));
        bufDesc.bindFlags = LLGL::BindFlags::VertexBuffer;
        bufDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;
        bufDesc.miscFlags = LLGL::MiscFlags::DynamicUsage;
        m_vertexBuffer.buffer = device->CreateBuffer(bufDesc);
        if (!m_vertexBuffer.buffer) {
            qWarning() << "LLGLWaveformWidget: failed to create vertex buffer";
            return false;
        }
    }

    qDebug() << "LLGLWaveformWidget: initialized with backend:"
             << m_pContext->backendName();
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_pRenderTimer->stop();

    if (!m_pContext) {
        return;
    }

    auto* device = m_pContext->renderSystem();
    if (device) {
        m_pContext->waitIdle();

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
    }

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
        qWarning() << "LLGLWaveformWidget: failed to create vertex shader";
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
        qWarning() << "LLGLWaveformWidget: failed to create fragment shader";
        return false;
    }

    return true;
}

void LLGLWaveformWidget::setupVertexFormat() {
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

    // Blend state
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

// ============================================================================
// Per-Frame Rendering
// ============================================================================

void LLGLWaveformWidget::renderLLGL() {
    if (!m_initOk || !m_pContext || !m_pContext->isValid()) {
        renderFallback();
        return;
    }

    // Update vertex buffer if data changed
    if (!m_vertices.empty()) {
        updateVertexBuffer(m_vertices.data(), static_cast<std::uint32_t>(m_vertices.size()));
    }

    // Begin frame (handles render pass, clear, etc.)
    m_pContext->beginFrame(Qt::black);

    // Bind pipeline state
    auto* cmdBuf = m_pContext->commandBuffer();
    if (cmdBuf && m_pipelineCache.pipelineState) {
        cmdBuf->SetPipelineState(*m_pipelineCache.pipelineState);
    }

    // Draw waveform
    if (m_vertexBuffer.buffer && m_vertexBuffer.count > 0 && cmdBuf) {
        cmdBuf->SetVertexBuffer(*m_vertexBuffer.buffer);
        cmdBuf->Draw(static_cast<std::uint32_t>(m_vertexBuffer.count), 0);
    }

    // End frame (handles present)
    m_pContext->endFrame();
}

// ============================================================================
// GPU Buffer Management
// ============================================================================

void LLGLWaveformWidget::updateVertexBuffer(const WaveformVertex* data, std::uint32_t count) {
    if (!data || count == 0 || !m_vertexBuffer.buffer) {
        return;
    }

    ensureBufferCapacity(count);

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

    std::uint32_t newCapacity = m_vertexBuffer.capacity;
    while (newCapacity < required) {
        newCapacity *= 2;
    }

    if (m_vertexBuffer.buffer) {
        device->Release(*m_vertexBuffer.buffer);
    }

    LLGL::BufferDescriptor bufDesc;
    bufDesc.size = static_cast<std::uint32_t>(newCapacity * sizeof(WaveformVertex));
    bufDesc.bindFlags = LLGL::BindFlags::VertexBuffer;
    bufDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;
    bufDesc.miscFlags = LLGL::MiscFlags::DynamicUsage;
    m_vertexBuffer.buffer = device->CreateBuffer(bufDesc);
    m_vertexBuffer.capacity = newCapacity;
    m_vertexBuffer.count = 0;
}

// ============================================================================
// Fallback
// ============================================================================

void LLGLWaveformWidget::renderFallback() {
    // No fallback — if LLGL fails, we show nothing
}
