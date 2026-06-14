#include "llglwaveformwidget.h"

#include <QDebug>
#include <QPainter>

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
          m_pRenderTimer(new QTimer(this)) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

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
    if (!m_initOk) {
        QPainter painter(this);
        draw(&painter, event);
        return;
    }

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
        m_pRenderTimer->start(16);
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
    QPainter painter(this);
    paintEvent(nullptr);
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
