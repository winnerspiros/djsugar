#pragma once

// LLGLWaveformWidget - Hardware-accelerated waveform rendering using LLGL
// (Low Level Graphics Library) as a cross-platform rendering backend.
//
// LLGL provides a unified API for OpenGL, Vulkan, Direct3D, and Metal.
// This widget replaces both the QOpenGLWindow-based GL path and the
// QPainter software fallback with a single LLGL-powered backend.
//
// CMake option MIXXX_USE_LLGL controls inclusion.

#include <QWidget>
#include <QTimer>
#include <memory>

#include "waveform/widgets/waveformwidgetabstract.h"

struct LLGLRenderSystem;

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
    bool initLLGL();
    void shutdownLLGL();
    void render();

    std::unique_ptr<LLGLRenderSystem> m_renderSystem;
    QTimer* m_pRenderTimer;
    bool m_initOk;
};
