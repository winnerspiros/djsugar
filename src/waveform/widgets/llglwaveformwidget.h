#pragma once

#include <QOffscreenSurface>
#include <QOpenGLContext>
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

/// LLGL rendering state for a single waveform widget.
struct LLGLWaveformRenderState {
    LLGL::Buffer* pVertexBuffer = nullptr;
    LLGL::Shader* pVertexShader = nullptr;
    LLGL::Shader* pFragmentShader = nullptr;
    LLGL::PipelineState* pPipelineState = nullptr;
    LLGL::PipelineLayout* pPipelineLayout = nullptr;
    uint32_t vertexCount = 0;
};

/// LLGLWaveformWidget renders waveforms using LLGL (Low Level Graphics Library).
/// Falls back to QPainter if LLGL initialization fails.
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
    void hideEvent(QEvent* event) override;

  private slots:
    void onRenderTimer();

  private:
    bool initializeLLGL();
    void shutdownLLGL();
    void renderLLGL();
    void renderFallback();
    bool createShaders();
    bool createPipelineState();
    bool createBuffers();
    void updateVertexData();

    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    std::unique_ptr<LLGLWaveformRenderState> m_renderState;
    std::vector<WaveformVertex> m_vertices;
    QTimer* m_pRenderTimer;
    bool m_initOk;
};
