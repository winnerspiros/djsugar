#pragma once

#include <QWidget>
#include <QTimer>
#include <QOpenGLContext>
#include <QOffscreenSurface>
#include <memory>

#include "waveform/widgets/waveformwidgetabstract.h"

/// LLGLWaveformWidget uses LLGL (Low Level Graphics Library) to render
/// waveforms with hardware acceleration on ALL platforms.
///
/// Unlike the allshader renderers which use Qt's QOpenGLFunctions,
/// this widget uses LLGL's RenderSystem and CommandBuffer directly.
/// This provides a single, unified rendering backend for all platforms:
/// - Windows: Direct3D 11 or OpenGL
/// - macOS: Metal or OpenGL
/// - Linux: OpenGL or Vulkan
/// - Android: OpenGL ES
///
/// When MIXXX_USE_LLGL is enabled, this replaces both the QOpenGLWindow-based
/// GL path and the QPainter software fallback.
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
    void renderBackground();
    void renderSignal();
    void renderBeatMarkers();
    void renderEndOfTrack();
    void renderMarks();

    // LLGL rendering state
    bool m_initOk;
    QTimer* m_pRenderTimer;

    // QPainter fallback state
    bool m_useFallback;
};
