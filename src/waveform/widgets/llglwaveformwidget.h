#pragma once

#include <QWidget>
#include <QTimer>
#include <QOpenGLContext>
#include <QOffscreenSurface>
#include <memory>

#include "waveform/widgets/waveformwidgetabstract.h"
#include "waveform/renderers/waveformwidgetrenderer.h"

struct LLGLRenderState;

/// LLGLWaveformWidget - Hardware-accelerated waveform rendering using LLGL.
///
/// This widget uses LLGL's OpenGL backend to render waveforms with
/// hardware acceleration on ALL platforms (Windows, macOS, Linux, Android).
/// It replaces both the QOpenGLWindow-based GL path and the QPainter
/// software fallback with a single, unified rendering backend.
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
    void renderWaveform();
    void renderBackground();
    void renderSignal();
    void renderBeatMarkers();
    void renderEndOfTrack();
    void renderMarks();

    std::unique_ptr<LLGLRenderState> m_renderState;
    QTimer* m_pRenderTimer;
    bool m_initOk;
};
