#pragma once

#include <QTimer>
#include <QWidget>
#include <memory>
#include <vector>
#include <mutex>

#include "rendergraph/llgl/rendergraph/context.h"
#include "waveform/widgets/waveformwidgetabstract.h"

// Vertex format: position (x, y) + color (r, g, b)
// Matches WaveformVertex in llglwaveformwidget.h for compatibility
struct WaveformVertex {
    float x, y;
    float r, g, b;
};

/// Per-frame rendering resources using LLGL command buffers.
/// Each frame gets its own command buffer for multi-buffering.
struct LLGLFrameResources {
    LLGL::CommandBuffer* commandBuffer = nullptr;
    LLGL::Fence* fence = nullptr;
    bool inUse = false;
};

/// GPU-side vertex buffer with persistent mapping.
/// Uploads waveform data once, reuses across frames.
struct LLGLVertexBuffer {
    LLGL::Buffer* buffer = nullptr;
    std::uint32_t capacity = 0;  // max vertices
    std::uint32_t count = 0;     // current vertices
};

/// Cached pipeline state objects — created once at init, reused every frame.
struct LLGLPipelineCache {
    LLGL::Shader* vertexShader = nullptr;
    LLGL::Shader* fragmentShader = nullptr;
    LLGL::PipelineState* pipelineState = nullptr;
    LLGL::PipelineLayout* pipelineLayout = nullptr;
    LLGL::VertexFormat vertexFormat;
    bool valid = false;
};

/// LLGLWaveformWidget renders waveforms using LLGL with full GPU acceleration.
///
/// Architecture:
/// - Creates an LLGL swap chain on a native window surface
/// - Uses command buffer batching: all draw commands recorded into one buffer per frame
/// - GPU vertex buffers: waveform data uploaded once, reused across frames
/// - Pipeline state caching: PSOs created at init, reused every frame
/// - Multi-buffered: 2-3 frames in flight to maximize GPU utilization
/// - Falls back to QPainter if LLGL initialization fails
class LLGLWaveformWidget : public WaveformWidgetAbstract, public QWidget {
    Q_OBJECT
  public:
    explicit LLGLWaveformWidget(const QString& group, QWidget* parent = nullptr);
    ~LLGLWaveformWidget() override;

    void castToQWidget() override;
    QWidget* widget() override;
    mixxx::Duration render() override;

  protected:
    void paintEvent(QPaintEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void showEvent(QShowEvent* event) override;
    void hideEvent(QHideEvent* event) override;

  private slots:
    void onRenderTimer();

  private:
    // Initialization
    bool initializeLLGL();
    void shutdownLLGL();
    bool createSwapChain();
    bool createPipelineCache();
    bool createFrameResources();
    bool createShaders();
    bool createPipelineState();
    void setupVertexFormat();

    // Per-frame rendering with command buffer batching
    void renderLLGL();
    void beginFrame();
    void recordDrawCommands();
    void endFrame();
    void presentFrame();
    void waitForFrame(LLGLFrameResources& frame);

    // GPU buffer management
    void updateVertexBuffer(const WaveformVertex* data, std::uint32_t count);
    void ensureBufferCapacity(std::uint32_t required);

    // Fallback
    void renderFallback();

    // Core LLGL resources
    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    LLGL::RenderTarget* m_pRenderTarget = nullptr;

    // Pipeline state cache (created once, reused every frame)
    LLGLPipelineCache m_pipelineCache;

    // GPU vertex buffer (persistent, uploaded once per data change)
    LLGLVertexBuffer m_vertexBuffer;

    // Multi-buffered frame resources
    static constexpr int kFrameCount = 3;
    LLGLFrameResources m_frames[kFrameCount];
    int m_currentFrame = 0;

    // Render state
    std::vector<WaveformVertex> m_vertices;
    QTimer* m_pRenderTimer = nullptr;
    bool m_initOk = false;
    std::mutex m_renderMutex;
};
