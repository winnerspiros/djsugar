#pragma once

#include <QTimer>
#include <QWidget>
#include <memory>
#include <vector>

#include "rendergraph/llgl/rendergraph/context.h"
#include "waveform/widgets/waveformwidgetabstract.h"

// Vertex format: position (x, y) + color (r, g, b)
struct WaveformVertex {
    float x, y;
    float r, g, b;
};

/// GPU-side vertex buffer with dynamic capacity.
struct LLGLVertexBuffer {
    LLGL::Buffer* buffer = nullptr;
    std::uint32_t capacity = 0;
    std::uint32_t count = 0;
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
/// - Uses LLGL swap chain for native surface rendering
/// - GPU vertex buffers: waveform data uploaded once, reused across frames
/// - Pipeline state caching: PSOs created at init, reused every frame
/// - No QPainter fallback — LLGL is the only rendering path
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
    bool initializeLLGL();
    void shutdownLLGL();
    bool createPipelineCache();
    bool createShaders();
    bool createPipelineState();
    void setupVertexFormat();

    void renderLLGL();
    void updateVertexBuffer(const WaveformVertex* data, std::uint32_t count);
    void ensureBufferCapacity(std::uint32_t required);

    void renderFallback();

    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    LLGLPipelineCache m_pipelineCache;
    LLGLVertexBuffer m_vertexBuffer;
    std::vector<WaveformVertex> m_vertices;
    QTimer* m_pRenderTimer = nullptr;
    bool m_initOk = false;
};
