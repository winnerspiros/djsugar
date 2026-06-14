#pragma once

#include <QWidget>
#include <QTimer>
#include <QOffscreenSurface>
#include <QOpenGLContext>
#include <memory>

#include "waveform/widgets/waveformwidgetabstract.h"
#include "rendergraph/llgl/rendergraph/context.h"

/// LLGL rendering state for a single waveform widget.
/// Manages LLGL buffers, shaders, and pipeline state.
struct LLGLWaveformRenderState {
    LLGL::Buffer* pVertexBuffer = nullptr;
    LLGL::Buffer* pIndexBuffer = nullptr;
    LLGL::Shader* pVertexShader = nullptr;
    LLGL::Shader* pFragmentShader = nullptr;
    LLGL::PipelineState* pPipelineState = nullptr;
    LLGL::PipelineLayout* pPipelineLayout = nullptr;
    uint32_t vertexCount = 0;
    uint32_t indexCount = 0;
};

/// LLGLWaveformWidget renders waveforms using LLGL (Low Level Graphics Library)
/// as the rendering backend on ALL platforms.
///
/// The widget creates its own LLGL context and swap chain, then renders
/// the waveform by:
/// 1. Computing vertex data from waveform samples (CPU-side preprocess)
/// 2. Uploading to LLGL vertex buffers
/// 3. Drawing with LLGL command buffer (shaders + pipeline state)
///
/// Falls back to QPainter if LLGL initialization fails.
class LLGLWaveformWidget : public WaveformWidgetAbstract, public QWidget {
    Q_OBJECT
  public:
    explicit LLGLWaveformWidget(const QString& group, QWidget* parent = nullptr);
    ~LLGLWaveformWidget() override;

    void castToQWidget() override;
    QWidget* widget() override;

    // Override WaveformWidgetAbstract
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
    void renderLLGL();
    void renderFallback();

    // LLGL rendering pipeline
    bool createShaders();
    bool createPipelineState();
    bool createBuffers();
    void updateVertexData();
    void drawWaveform();

    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    std::unique_ptr<LLGLWaveformRenderState> m_renderState;

    // Waveform data cache
    std::vector<float> m_vertices;

    QTimer* m_pRenderTimer;
    bool m_initOk;
};
