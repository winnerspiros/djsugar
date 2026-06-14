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
          m_pContext(std::make_unique<rendergraph::LLGLContext>()),
          m_pRenderTimer(new QTimer(this)),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);

    // Always add QPainter-based renderers as fallback.
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
        renderFallback();
        return;
    }

    render();
}

void LLGLWaveformWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    if (m_initOk && m_pContext) {
        m_pContext->resize(width(), height());
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
    if (!m_pContext) {
        return false;
    }

    if (!m_pContext->initialize(this)) {
        qWarning() << "LLGLWaveformWidget: failed to initialize LLGL context";
        return false;
    }

    qDebug() << "LLGLWaveformWidget: LLGL context initialized";
    return true;
}

void LLGLWaveformWidget::shutdownLLGL() {
    m_pRenderTimer->stop();
    if (m_pContext) {
        m_pContext->shutdown();
    }
    m_initOk = false;
}

void LLGLWaveformWidget::render() {
    if (!m_pContext || !m_pContext->isValid()) {
        renderFallback();
        return;
    }

    renderLLGL();
}

void LLGLWaveformWidget::renderLLGL() {
    // Clear the background using LLGL
    m_pContext->beginFrame(QColor(0, 0, 0, 255));

    // TODO: Render waveform using LLGL command buffer
    // For now, we clear the screen and present

    m_pContext->endFrame();
}

void LLGLWaveformWidget::renderFallback() {
    QPainter painter(this);
    draw(&painter, nullptr);
}
