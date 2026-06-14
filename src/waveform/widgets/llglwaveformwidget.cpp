#include "llglwaveformwidget.h"

#include <QDebug>
#include <QPainter>
#include <QWindow>
#include <cmath>

#include "moc_llglwaveformwidget.cpp"
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

// Simple vertex shader for waveform rendering
static const char* kVertexShaderSource = R"(
    #version 330 core
    layout(location = 0) in vec2 aPosition;
    layout(location = 1) in vec3 aColor;
    out vec3 vColor;
    void main() {
        gl_Position = vec4(aPosition, 0.0, 1.0);
        vColor = aColor;
    }
)";

// Simple fragment shader
static const char* kFragmentShaderSource = R"(
    #version 330 core
    in vec3 vColor;
    out vec4 fragColor;
    void main() {
        fragColor = vec4(vColor, 1.0);
    }
)";

// Vertex format: position (x, y) + color (r, g, b)
// (defined in header)

LLGLWaveformWidget::LLGLWaveformWidget(const QString& group, QWidget* parent)
        : WaveformWidgetAbstract(group),
          QWidget(parent),
          m_pContext(std::make_unique<rendergraph::LLGLContext>()),
          m_renderState(std::make_unique<LLGLWaveformRenderState>()),
          m_pRenderTimer(new QTimer(this)),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

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
    update();
}

void LLGLWaveformWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    if (!m_initOk) {
        m_initOk = initializeLLGL();
    }
    if (m_initOk) {
        m_pRenderTimer->start(16);
    }
}

void LLGLWaveformWidget::hideEvent(QEvent* event) {
    QWidget::hideEvent(event);
    m_pRenderTimer->stop();
}

void LLGLWaveformWidget::onRenderTimer() {
    if (isVisible()) {
        update();
    }
}

bool LLGLWaveformWidget::initializeLLGL() {
    if (!m_pContext || !m_renderState) {
        return false;
    }

    if (!m_pContext->initialize(this)) {
        qWarning() << "LLGLWaveformWidget: failed to initialize LLGL context";
        return false;
    }

    if (!createShaders()) {
        qWarning() << "LLGLWaveformWidget: failed to create shaders";
        return false;
    }

    if (!createBuffers()) {
        qWarning() << "LLGLWaveformWidget: failed to create buffers";
        return false;
    }

    if (!createPipelineState()) {
        qWarning() << "LLGLWaveformWidget: failed to create pipeline state";
        return false;
    }

    qDebug() << "LLGLWaveformWidget: fully initialized with LLGL rendering";
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_pRenderTimer->stop();

    if (!m_pContext || !m_renderState) {
        return;
    }

    auto* pDevice = m_pContext->renderSystem();
    if (pDevice) {
        auto* pQueue = m_pContext->commandQueue();
        if (pQueue) {
            pQueue->WaitIdle();
        }
        if (m_renderState->pPipelineState) {
            pDevice->Release(*m_renderState->pPipelineState);
        }
        if (m_renderState->pPipelineLayout) {
            pDevice->Release(*m_renderState->pPipelineLayout);
        }
        if (m_renderState->pVertexBuffer) {
            pDevice->Release(*m_renderState->pVertexBuffer);
        }
        if (m_renderState->pVertexShader) {
            pDevice->Release(*m_renderState->pVertexShader);
        }
        if (m_renderState->pFragmentShader) {
            pDevice->Release(*m_renderState->pFragmentShader);
        }
    }

    m_renderState = std::make_unique<LLGLWaveformRenderState>();
    m_pContext->shutdown();
    m_initOk = false;
}

bool LLGLWaveformWidget::createShaders() {
    auto* pDevice = m_pContext->renderSystem();
    if (!pDevice) {
        return false;
    }

    LLGL::ShaderDescriptor vsDesc;
    vsDesc.type = LLGL::ShaderType::Vertex;
    vsDesc.source = kVertexShaderSource;
    vsDesc.sourceSize = static_cast<std::uint32_t>(strlen(kVertexShaderSource));
    vsDesc.entryPoint = "main";

    m_renderState->pVertexShader = pDevice->CreateShader(vsDesc);
    if (!m_renderState->pVertexShader) {
        return false;
    }

    LLGL::ShaderDescriptor fsDesc;
    fsDesc.type = LLGL::ShaderType::Fragment;
    fsDesc.source = kFragmentShaderSource;
    fsDesc.sourceSize = static_cast<std::uint32_t>(strlen(kFragmentShaderSource));
    fsDesc.entryPoint = "main";

    m_renderState->pFragmentShader = pDevice->CreateShader(fsDesc);
    return m_renderState->pFragmentShader != nullptr;
}

bool LLGLWaveformWidget::createBuffers() {
    auto* pDevice = m_pContext->renderSystem();
    if (!pDevice) {
        return false;
    }

    // Define vertex attributes
    LLGL::VertexAttribute vertexAttribs[2];
    vertexAttribs[0].name = "aPosition";
    vertexAttribs[0].format = LLGL::Format::RG32Float;
    vertexAttribs[0].location = 0;
    vertexAttribs[0].offset = 0;
    vertexAttribs[0].stride = sizeof(WaveformVertex);

    vertexAttribs[1].name = "aColor";
    vertexAttribs[1].format = LLGL::Format::RGB32Float;
    vertexAttribs[1].location = 1;
    vertexAttribs[1].offset = sizeof(float) * 2;
    vertexAttribs[1].stride = sizeof(WaveformVertex);

    // Create vertex buffer with vertex attributes
    const std::uint64_t bufferSize = 65536 * sizeof(WaveformVertex);

    LLGL::BufferDescriptor vbDesc;
    vbDesc.size = bufferSize;
    vbDesc.bindFlags = LLGL::BindFlags::VertexBuffer;
    vbDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;
    vbDesc.vertexAttribs = vertexAttribs;
    vbDesc.miscFlags = LLGL::MiscFlags::DynamicUsage;

    m_renderState->pVertexBuffer = pDevice->CreateBuffer(vbDesc);
    return m_renderState->pVertexBuffer != nullptr;
}

bool LLGLWaveformWidget::createPipelineState() {
    auto* pDevice = m_pContext->renderSystem();
    if (!pDevice) {
        return false;
    }

    // Pipeline layout (empty - no binding points)
    LLGL::PipelineLayoutDescriptor layoutDesc;
    m_renderState->pPipelineLayout = pDevice->CreatePipelineLayout(layoutDesc);

    LLGL::GraphicsPipelineDescriptor pipelineDesc;
    pipelineDesc.pipelineLayout = m_renderState->pPipelineLayout;
    pipelineDesc.vertexShader = m_renderState->pVertexShader;
    pipelineDesc.fragmentShader = m_renderState->pFragmentShader;
    pipelineDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleList;

    // Rasterizer: no culling
    pipelineDesc.rasterizer.cullMode = LLGL::CullMode::Disabled;

    // Blend: standard alpha blending
    pipelineDesc.blend.targets[0].blendEnabled = true;
    pipelineDesc.blend.targets[0].srcColor = LLGL::BlendOp::SrcAlpha;
    pipelineDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;

    m_renderState->pPipelineState = pDevice->CreatePipelineState(pipelineDesc);
    return m_renderState->pPipelineState != nullptr;
}

void LLGLWaveformWidget::renderLLGL() {
    if (!m_pContext || !m_pContext->isValid() || !m_renderState) {
        renderFallback();
        return;
    }

    updateVertexData();

    m_pContext->beginFrame(QColor(0, 0, 0, 255));

    auto* pCmdBuf = m_pContext->commandBuffer();
    if (pCmdBuf && m_renderState->pPipelineState && m_renderState->vertexCount > 0) {
        pCmdBuf->SetPipelineState(*m_renderState->pPipelineState);
        pCmdBuf->SetVertexBuffer(*m_renderState->pVertexBuffer);

        LLGL::Viewport viewport;
        viewport.x = 0.0f;
        viewport.y = 0.0f;
        viewport.width = static_cast<float>(width());
        viewport.height = static_cast<float>(height());
        viewport.minDepth = 0.0f;
        viewport.maxDepth = 1.0f;
        pCmdBuf->SetViewport(viewport);

        pCmdBuf->Draw(static_cast<std::uint32_t>(m_renderState->vertexCount), 0);
    }

    m_pContext->endFrame();
}

void LLGLWaveformWidget::updateVertexData() {
    if (!m_renderState || !m_renderState->pVertexBuffer) {
        m_renderState->vertexCount = 0;
        return;
    }

    TrackPointer pTrack = getTrackInfo();
    if (!pTrack) {
        m_renderState->vertexCount = 0;
        return;
    }

    ConstWaveformPointer waveform = pTrack->getWaveform();
    if (waveform.isNull() || waveform->getDataSize() <= 1) {
        m_renderState->vertexCount = 0;
        return;
    }

    const int dataSize = waveform->getDataSize();
    const WaveformData* data = waveform->data();
    if (!data) {
        m_renderState->vertexCount = 0;
        return;
    }

    const int visualFramesSize = dataSize / 2;
    const double firstVisualFrame = getFirstDisplayedPosition() * visualFramesSize;
    const double lastVisualFrame = getLastDisplayedPosition() * visualFramesSize;
    const int pixelLength = width();

    if (pixelLength <= 0) {
        m_renderState->vertexCount = 0;
        return;
    }

    const double visualIncrementPerPixel =
            (lastVisualFrame - firstVisualFrame) / static_cast<double>(pixelLength);
    const double maxSamplingRange = visualIncrementPerPixel / 2.0;

    const float breadth = static_cast<float>(getBreadth());
    const float halfBreadth = breadth / 2.0f;
    const float heightFactor = halfBreadth / 255.0f;

    // Waveform color (green-ish)
    constexpr float signalR = 0.0f;
    constexpr float signalG = 1.0f;
    constexpr float signalB = 0.3f;

    m_vertices.clear();

    double xVisualFrame = firstVisualFrame;
    for (int pos = 0; pos < pixelLength; ++pos) {
        const int visualFrameStart = std::lround(xVisualFrame - maxSamplingRange);
        const int visualFrameStop = std::lround(xVisualFrame + maxSamplingRange);
        const int visualIndexStart = std::max(visualFrameStart * 2, 0);
        const int visualIndexStop =
                std::min(std::max(visualFrameStop, visualFrameStart + 1) * 2, dataSize - 1);

        float maxSample = 0.0f;
        for (int i = visualIndexStart; i < visualIndexStop; i++) {
            maxSample = math_max(maxSample, static_cast<float>(data[i].filtered.all));
        }

        const float fpos = static_cast<float>(pos);
        const float halfPixelSize = 0.5f;
        const float top = halfBreadth - heightFactor * maxSample;
        const float bottom = halfBreadth + heightFactor * maxSample;

        // Two triangles (6 vertices) for a rectangle
        m_vertices.push_back({fpos - halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos - halfPixelSize, bottom, signalR, signalG, signalB});

        m_vertices.push_back({fpos - halfPixelSize, bottom, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, bottom, signalR, signalG, signalB});

        xVisualFrame += visualIncrementPerPixel;
    }

    m_renderState->vertexCount = static_cast<std::uint32_t>(m_vertices.size());

    // Upload vertex data to LLGL buffer
    if (m_renderState->vertexCount > 0) {
        auto* pCmdBuf = m_pContext->commandBuffer();
        if (pCmdBuf) {
            const size_t dataSize = m_vertices.size() * sizeof(WaveformVertex);
            pCmdBuf->UpdateBuffer(
                    *m_renderState->pVertexBuffer,
                    0,
                    m_vertices.data(),
                    static_cast<std::uint64_t>(dataSize));
        }
    }
}

void LLGLWaveformWidget::renderFallback() {
    QPainter painter(this);
    draw(&painter, nullptr);
}
