#pragma once

#include <QtGlobal>

// This define is checked in wglwidgetqglwidget.h and wglwidgetqopengl.h
// to make sure they are only included from this header, to enforce that
// all code includes this header wglwidget.h.
#define WGLWIDGET_H

#ifdef Q_OS_ANDROID
// Android: use stub to avoid QOpenGLWindow crash on devices where
// OpenGL context creation fails. Provides inline implementations of
// all virtual methods so the linker can resolve symbols from
// derived classes (WSpinnyBase, WVuMeterBase) that reference them.
#include <QWidget>

class QPaintDevice;
class TrackDropTarget;

class WGLWidget : public QWidget {
  public:
    WGLWidget(QWidget* parent)
            : QWidget(parent) {
    }
    ~WGLWidget() = default;

    bool isContextValid() const {
        return false;
    }
    bool shouldRender() const {
        return false;
    }
    void makeCurrentIfNeeded() {
    }
    void doneCurrent() {
    }
    void swapBuffers() {
    }

    virtual void paintGL() {
    }
    virtual void resizeGL(int w, int h) {
        Q_UNUSED(w);
        Q_UNUSED(h);
    }
    virtual void initializeGL() {
    }

    void setTrackDropTarget(TrackDropTarget* pTarget) {
        Q_UNUSED(pTarget);
    }
    TrackDropTarget* trackDropTarget() const {
        return nullptr;
    }

    void* getOpenGLWindow() const {
        return nullptr;
    }

  protected:
    void showEvent(QShowEvent* event) override {
        QWidget::showEvent(event);
    }
    void resizeEvent(QResizeEvent* event) override {
        QWidget::resizeEvent(event);
    }

    QPaintDevice* paintDevice() {
        return nullptr;
    }
};
#elif MIXXX_USE_QOPENGL
#include "widget/wglwidgetqopengl.h"
#elif QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
// QGLWidget was removed in Qt6 — only include Qt5 fallback
#include "widget/wglwidgetqglwidget.h"
#else
// Qt6 without QOPENGL: use QOpenGLWindow-based fallback
#include "widget/wglwidgetqopengl.h"
#endif

#undef WGLWIDGET_H
