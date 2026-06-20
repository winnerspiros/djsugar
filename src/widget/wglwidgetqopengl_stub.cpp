// Stub implementations for WGLWidget on Android (QOPENGL=OFF)
// Provides out-of-line virtual method symbols for the linker.
// On Android, WGLWidget is defined inline in wglwidgetqopengl_android.h
// but we still need this file to emit the vtable and typeinfo symbols
// when LTO (CMAKE_INTERPROCEDURAL_OPTIMIZATION=ON) is enabled.
#include "widget/wglwidget.h"

// Out-of-line definitions to force vtable/typeinfo emission under LTO.
// These match the inline definitions in wglwidgetqopengl_android.h.

WGLWidget::WGLWidget(QWidget* parent)
        : QWidget(parent) {
}

WGLWidget::~WGLWidget() = default;

bool WGLWidget::isContextValid() const {
    return false;
}

bool WGLWidget::shouldRender() const {
    return false;
}

void WGLWidget::makeCurrentIfNeeded() {
}

void WGLWidget::doneCurrent() {
}

void WGLWidget::swapBuffers() {
}

void WGLWidget::paintGL() {
}

void WGLWidget::resizeGL(int w, int h) {
    Q_UNUSED(w);
    Q_UNUSED(h);
}

void WGLWidget::initializeGL() {
}

void WGLWidget::setTrackDropTarget(TrackDropTarget* pTarget) {
    Q_UNUSED(pTarget);
}

TrackDropTarget* WGLWidget::trackDropTarget() const {
    return nullptr;
}

void* WGLWidget::getOpenGLWindow() const {
    return nullptr;
}

void WGLWidget::showEvent(QShowEvent* event) {
    QWidget::showEvent(event);
}

void WGLWidget::resizeEvent(QResizeEvent* event) {
    QWidget::resizeEvent(event);
}

QPaintDevice* WGLWidget::paintDevice() {
    return nullptr;
}
