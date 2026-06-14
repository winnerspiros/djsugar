#pragma once

#include <QOffscreenSurface>
#include <QOpenGLContext>
#include <QTimer>
#include <QWidget>
#include <memory>

#include "rendergraph/llgl/rendergraph/context.h"
#include "waveform/widgets/waveformwidgetabstract.h"

/// LLGLWaveformWidget uses LLGL (Low Level Graphics Library) to render
/// waveforms with hardware acceleration on ALL platforms.
///
/// This widget uses LLGL's rendering backend to draw waveforms.
/// It creates its own LLGL context and swap chain, and renders
/// the waveform using LLGL command buffers.
///
/// When MIXXX_USE_LLGL is enabled, this becomes the primary waveform widget.
class LLGLWaveformWidget : public WaveformWidgetAbstract, public QWidget {
    Q_OBJECT
  public:
    explicit LLGLWaveformWidget(const QString& group, QWidget* parent = nullptr);
    ~LLGLWaveformWidget() override;

    void castToQWidget() override;
    QWidget* widget() override;

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
    void render();
    void renderLLGL();
    void renderFallback();

    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    QTimer* m_pRenderTimer;
    bool m_initOk;
};
