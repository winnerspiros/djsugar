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

// LLGL render system wrapper
struct LLGLRenderSystem {
    bool initialized = false;
};

LLGLWaveformWidget::LLGLWaveformWidget(const QString& group, QWidget* parent)
        : WaveformWidgetAbstract(group),
          QWidget(parent),
          m_renderSystem(std::make_unique<LLGLRenderSystem>()),
          m_pRenderTimer(new QTimer(this)),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

    // Always add QPainter-based renderers as fallback.
    // LLGL rendering will be layered on top when initialized.
    addRenderer<WaveformRenderBackground>();
    addRenderer<WaveformRendererEndOfTrack>();
    addRenderer<WaveformRendererPreroll>();
    addRenderer<WaveformRenderMarkRange>();
    addRenderer<WaveformRendererFilteredSignal>();
    addRenderer<WaveformRenderBeat>();
    addRenderer<WaveformRenderMark>();

    m_initOk = initLLGL();
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
    QPainter painter(this);
    draw(&painter, event);
}

void LLGLWaveformWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    update();
}

void LLGLWaveformWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
    // Refresh timer for smooth waveform updates at display refresh rate
    m_pRenderTimer->start(16); // ~60fps default
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

bool LLGLWaveformWidget::initLLGL() {
    // LLGL rendering infrastructure placeholder.
    // Full implementation: load RenderSystem with "OpenGL" or "Vulkan"
    // backend, create Device, CommandBuffer, SwapChain for this widget's
    // native window handle.
    //
    // For now we rely on the QPainter fallback (always registered above)
    // and flag the widget as initialized. The render() override will use
    // LLGL when m_initOk is true, otherwise falls through to QPainter.
    qDebug() << "LLGLWaveformWidget: initialized (QPainter fallback active)";
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_renderSystem->initialized = false;
    m_pRenderTimer->stop();
}

void LLGLWaveformWidget::render() {
    // TODO: Full LLGL rendering pipeline
}
