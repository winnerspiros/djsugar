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
    uniform mat4 uProjection;
    out vec3 vColor;
    void main() {
        gl_Position = uProjection * vec4(aPosition, 0.0, 1.0);
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
struct WaveformVertex {
    float x, y;
    float r, g, b;
};

LLGLWaveformWidget::LLGLWaveformWidget(const QString& group, QWidget* parent)
        : WaveformWidgetAbstract(group),
          QWidget(parent),
          m_pContext(std::make_unique<rendergraph::LLGLContext>()),
          m_renderState(std::make_unique<LLGLWaveformRenderState>()),
          m_pRenderTimer(new QTimer(this)),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

    // Add QPainter-based renderers as fallback
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

void LLGLWaveformWidget::hideEvent(QHideEvent* event) {
    QWidget::hideEvent(event);
    m_pRenderTimer->stop();
}

void LLGLWaveformWidget::onRenderTimer() {
    if (isVisible()) {
        update();
    }
}

bool LLGLWaveformWidget::initializeLLGL() {
    if (!m_pContext) {
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

    if (!createPipelineState()) {
        qWarning() << "LLGLWaveformWidget: failed to create pipeline state";
        return false;
    }

    if (!createBuffers()) {
        qWarning() << "LLGLWaveformWidget: failed to create buffers";
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
        if (m_renderState->pPipelineState) {
            pDevice->Release(*m_renderState->pPipelineState);
        }
        if (m_renderState->pPipelineLayout) {
            pDevice->Release(*m_renderState->pPipelineLayout);
        }
        if (m_renderState->pVertexBuffer) {
            pDevice->Release(*m_renderState->pVertexBuffer);
        }
        if (m_renderState->pIndexBuffer) {
            pDevice->Release(*m_renderState->pIndexBuffer);
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

    // Create vertex shader
    LLGL::ShaderDescriptor vsDesc;
    vsDesc.type = LLGL::ShaderType::Vertex;
    vsDesc.source = kVertexShaderSource;
    vsDesc.sourceSize = strlen(kVertexShaderSource);
    vsDesc.entryPoint = "main";
    vsDesc.profile = nullptr;

    m_renderState->pVertexShader = pDevice->CreateShader(vsDesc);
    if (!m_renderState->pVertexShader) {
        return false;
    }

    // Create fragment shader
    LLGL::ShaderDescriptor fsDesc;
    fsDesc.type = LLGL::ShaderType::Fragment;
    fsDesc.source = kFragmentShaderSource;
    fsDesc.sourceSize = strlen(kFragmentShaderSource);
    fsDesc.entryPoint = "main";
    fsDesc.profile = nullptr;

    m_renderState->pFragmentShader = pDevice->CreateShader(fsDesc);
    if (!m_renderState->pFragmentShader) {
        return false;
    }

    return true;
}

bool LLGLWaveformWidget::createPipelineState() {
    auto* pDevice = m_pContext->renderSystem();
    if (!pDevice) {
        return false;
    }

    // Pipeline layout (empty - no uniform buffers needed for now)
    LLGL::PipelineLayoutDescriptor layoutDesc;
    m_renderState->pPipelineLayout = pDevice->CreatePipelineLayout(layoutDesc);

    // Graphics pipeline descriptor
    LLGL::GraphicsPipelineDescriptor pipelineDesc;
    pipelineDesc.pipelineLayout = m_renderState->pPipelineLayout;
    pipelineDesc.vertexShader = m_renderState->pVertexShader;
    pipelineDesc.fragmentShader = m_renderState->pFragmentShader;

    // Vertex attributes
    LLGL::VertexAttribute vertexAttributes[2];
    vertexAttributes[0].name = "aPosition";
    vertexAttributes[0].format = LLGL::Format::RG32Float;
    vertexAttributes[0].location = 0;
    vertexAttributes[0].offset = 0;

    vertexAttributes[1].name = "aColor";
    vertexAttributes[1].format = LLGL::Format::RGB32Float;
    vertexAttributes[1].location = 1;
    vertexAttributes[1].offset = sizeof(float) * 2;

    pipelineDesc.vertexAttributes = vertexAttributes;
    pipelineDesc.numVertexAttributes = 2;

    // Primitive topology
    pipelineDesc.primitiveTopology = LLGL::PrimitiveTopology::TriangleList;

    // Blend state
    pipelineDesc.blend.targets[0].blendEnable = true;
    pipelineDesc.blend.targets[0].srcColor = LLGL::BlendOp::SrcAlpha;
    pipelineDesc.blend.targets[0].dstColor = LLGL::BlendOp::InvSrcAlpha;

    // Rasterizer state
    pipelineDesc.rasterizer.cullMode = LLGL::CullMode::None;

    m_renderState->pPipelineState = pDevice->CreatePipelineState(pipelineDesc);
    return m_renderState->pPipelineState != nullptr;
}

bool LLGLWaveformWidget::createBuffers() {
    auto* pDevice = m_pContext->renderSystem();
    if (!pDevice) {
        return false;
    }

    // Create dynamic vertex buffer (will be updated each frame)
    const size_t maxVertices = 65536;
    LLGL::BufferDescriptor vbDesc;
    vbDesc.size = maxVertices * sizeof(WaveformVertex);
    vbDesc.bindFlags = LLGL::BindFlags::VertexBuffer;
    vbDesc.cpuAccessFlags = LLGL::CPUAccessFlags::Write;

    m_renderState->pVertexBuffer = pDevice->CreateBuffer(vbDesc);
    if (!m_renderState->pVertexBuffer) {
        return false;
    }

    return true;
}

void LLGLWaveformWidget::renderLLGL() {
    if (!m_pContext || !m_pContext->isValid() || !m_renderState) {
        renderFallback();
        return;
    }

    // Update vertex data from waveform
    updateVertexData();

    if (m_renderState->vertexCount == 0) {
        // Nothing to draw, just clear
        m_pContext->beginFrame(QColor(0, 0, 0, 255));
        m_pContext->endFrame();
        return;
    }

    // Render using LLGL
    m_pContext->beginFrame(QColor(0, 0, 0, 255));

    auto* pCmdBuf = m_pContext->commandBuffer();
    if (pCmdBuf && m_renderState->pPipelineState) {
        // Set pipeline state
        pCmdBuf->SetPipelineState(*m_renderState->pPipelineState);

        // Set vertex buffer
        LLGL::VertexBufferStream vertexStream;
        vertexStream.buffer = m_renderState->pVertexBuffer;
        vertexStream.stride = sizeof(WaveformVertex);
        vertexStream.offset = 0;
        pCmdBuf->SetVertexBuffer(vertexStream);

        // Draw
        pCmdBuf->Draw(static_cast<std::uint32_t>(m_renderState->vertexCount), 0);
    }

    m_pContext->endFrame();
}

void LLGLWaveformWidget::updateVertexData() {
    if (!m_renderState || !m_renderState->pVertexBuffer) {
        m_renderState->vertexCount = 0;
        return;
    }

    // Get waveform data from the track
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
    const double visualIncrementPerPixel = (lastVisualFrame - firstVisualFrame) / static_cast<double>(pixelLength);
    const double maxSamplingRange = visualIncrementPerPixel / 2.0;

    const float breadth = static_cast<float>(getBreadth());
    const float halfBreadth = breadth / 2.0f;
    const float heightFactor = halfBreadth / 255.0f;

    // Waveform color (green)
    const float signalR = 0.0f;
    const float signalG = 1.0f;
    const float signalB = 0.3f;

    m_vertices.clear();

    double xVisualFrame = firstVisualFrame;
    for (int pos = 0; pos < pixelLength; ++pos) {
        const int visualFrameStart = std::lround(xVisualFrame - maxSamplingRange);
        const int visualFrameStop = std::lround(xVisualFrame + maxSamplingRange);
        const int visualIndexStart = std::max(visualFrameStart * 2, 0);
        const int visualIndexStop = std::min(std::max(visualFrameStop, visualFrameStart + 1) * 2, dataSize - 1);

        float maxSample = 0.0f;
        for (int i = visualIndexStart; i < visualIndexStop; i++) {
            maxSample = math_max(maxSample, static_cast<float>(data[i].filtered.all));
        }

        const float fpos = static_cast<float>(pos);
        const float halfPixelSize = 0.5f;
        const float top = halfBreadth - heightFactor * maxSample;
        const float bottom = halfBreadth + heightFactor * maxSample;

        // Two triangles (6 vertices) for a rectangle
        // Triangle 1
        m_vertices.push_back({fpos - halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos - halfPixelSize, bottom, signalR, signalG, signalB});
        // Triangle 2
        m_vertices.push_back({fpos - halfPixelSize, bottom, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, top, signalR, signalG, signalB});
        m_vertices.push_back({fpos + halfPixelSize, bottom, signalR, signalG, signalB});

        xVisualFrame += visualIncrementPerPixel;
    }

    m_renderState->vertexCount = static_cast<uint32_t>(m_vertices.size());

    // Upload to LLGL buffer
    if (m_renderState->vertexCount > 0 && m_renderState->pVertexBuffer) {
        auto* pCmdBuf = m_pContext->commandBuffer();
        if (pCmdBuf) {
            const size_t dataSize = m_vertices.size() * sizeof(WaveformVertex);
            pCmdBuf->UpdateBuffer(*m_renderState->pVertexBuffer, 0, m_vertices.data(), dataSize);
        }
    }
}

void LLGLWaveformWidget::renderFallback() {
    QPainter painter(this);
    draw(&painter, nullptr);
}
