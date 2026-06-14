#include "llglwaveformwidget.h"

#include <QPainter>
#include <QDebug>

#include "moc_llglwaveformwidget.cpp"
#include "waveform/renderers/waveformrenderbackground.h"
#include "waveform/renderers/waveformrenderbeat.h"
#include "waveform/renderers/waveformrendererendoftrack.h"
#include "waveform/renderers/waveformrendererfilteredsignal.h"
#include "waveform/renderers/waveformrendererpreroll.h"
#include "waveform/renderers/waveformrendermark.h"
#include "waveform/renderers/waveformrendermarkrange.h"

LLGLWaveformWidget::LLGLWaveformWidget(const QString& group, QWidget* parent)
        : WaveformWidgetAbstract(group),
          QWidget(parent),
          m_initOk(false),
          m_pRenderTimer(new QTimer(this)),
          m_useFallback(true) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

    // Add QPainter-based renderers as fallback.
    // These are always registered and used when LLGL is not available.
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

void LLGLWaveformWidget::paintEvent(QPaintEvent* event) {
    if (!m_initOk || m_useFallback) {
        // QPainter fallback
        QPainter painter(this);
        draw(&painter, event);
        return;
    }

    // LLGL rendering
    render();
}

void LLGLWaveformWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    if (m_initOk) {
        // Resize LLGL swap chain
    }
    update();
}

void LLGLWaveformWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    if (!m_initOk) {
        m_initOk = initializeLLGL();
    }
    if (m_initOk) {
        m_useFallback = false;
        m_pRenderTimer->start(16); // ~60fps
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
    // LLGL rendering infrastructure placeholder.
    // Full implementation: load RenderSystem with platform-appropriate backend,
    // create Device, CommandBuffer, SwapChain for this widget's native window.
    //
    // For now we rely on the QPainter fallback (always registered above)
    // and flag the widget as initialized. The render() override will use
    // LLGL when m_initOk is true, otherwise falls through to QPainter.
    qDebug() << "LLGLWaveformWidget: initialized (QPainter fallback active)";
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_initOk = false;
    m_pRenderTimer->stop();
}

void LLGLWaveformWidget::render() {
    // TODO: Full LLGL rendering pipeline
    // For now, fall through to QPainter
    m_useFallback = true;
    update();
}

void LLGLWaveformWidget::renderBackground() {
    // TODO: LLGL background rendering
}

void LLGLWaveformWidget::renderSignal() {
    // TODO: LLGL signal rendering
}

void LLGLWaveformWidget::renderBeatMarkers() {
    // TODO: LLGL beat marker rendering
}

void LLGLWaveformWidget::renderEndOfTrack() {
    // TODO: LLGL end-of-track rendering
}

void LLGLWaveformWidget::renderMarks() {
    // TODO: LLGL mark rendering
}
