#include "llglwglwidget.h"

#include <QDebug>
#include <QWindow>

#include "widget/trackdroptarget.h"
#include "rendergraph/llgl/rendergraph/context.h"

LLGLWGLWidget::LLGLWGLWidget(QWidget* parent)
        : QWidget(parent),
          m_pContext(std::make_unique<rendergraph::LLGLContext>()),
          m_pRenderTimer(new QTimer(this)),
          m_pTrackDropTarget(nullptr),
          m_initialized(false),
          m_initOk(false) {
    setAttribute(Qt::WA_NoSystemBackground);
    setAttribute(Qt::WA_OpaquePaintEvent);
    setAttribute(Qt::WA_NativeWindow);

    connect(m_pRenderTimer, &QTimer::timeout, this, &LLGLWGLWidget::onRenderTimer);
}

LLGLWGLWidget::~LLGLWGLWidget() {
    m_pRenderTimer->stop();
    if (m_pContext) {
        m_pContext->shutdown();
    }
}

bool LLGLWGLWidget::isContextValid() const {
    return m_pContext && m_pContext->isValid();
}

bool LLGLWGLWidget::shouldRender() const {
    return m_initOk && isVisible();
}

void LLGLWGLWidget::makeCurrentIfNeeded() {
    // LLGL manages the context internally
}

void LLGLWGLWidget::doneCurrent() {
    // LLGL manages the context internally
}

void LLGLWGLWidget::swapBuffers() {
    if (m_pContext && m_pContext->isValid()) {
        m_pContext->endFrame();
    }
}

void LLGLWGLWidget::paintGL() {
    // Base class does nothing — subclasses override with engine->render() etc.
}

void LLGLWGLWidget::resizeGL(int w, int h) {
    Q_UNUSED(w);
    Q_UNUSED(h);
}

void LLGLWGLWidget::initializeGL() {
    // Base class does nothing — subclasses override
}

void LLGLWGLWidget::setTrackDropTarget(TrackDropTarget* pTarget) {
    m_pTrackDropTarget = pTarget;
}

TrackDropTarget* LLGLWGLWidget::trackDropTarget() const {
    return m_pTrackDropTarget;
}

void LLGLWGLWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);

    if (!m_initialized) {
        winId(); // Ensure native window handle exists

        if (m_pContext->initialize(this)) {
            m_initOk = true;
            m_initialized = true;
            initializeGL();
            m_pRenderTimer->start(16); // ~60 FPS
        } else {
            qWarning() << "LLGLWGLWidget: failed to initialize LLGL context";
        }
    } else if (m_initOk) {
        m_pRenderTimer->start(16);
    }
}

void LLGLWGLWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
    if (m_initOk && m_pContext) {
        m_pContext->resize(width(), height());
        resizeGL(width(), height());
    }
}

void LLGLWGLWidget::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event);
    if (!m_initOk) {
        return;
    }

    // Begin LLGL frame — clear to black
    m_pContext->beginFrame(QColor(0, 0, 0, 255));

    // Set viewport
    auto* pCmdBuf = m_pContext->commandBuffer();
    if (pCmdBuf) {
        LLGL::Viewport viewport;
        viewport.x = 0.0f;
        viewport.y = 0.0f;
        viewport.width = static_cast<float>(width());
        viewport.height = static_cast<float>(height());
        viewport.minDepth = 0.0f;
        viewport.maxDepth = 1.0f;
        pCmdBuf->SetViewport(viewport);
    }

    // Call subclass rendering
    paintGL();

    // End frame and present
    m_pContext->endFrame();
}

void LLGLWGLWidget::onRenderTimer() {
    if (isVisible() && m_initOk) {
        update();
    }
}
