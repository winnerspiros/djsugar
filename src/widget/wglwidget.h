#pragma once

// LLGL is the default rendering backend on all platforms.
// This header routes to the LLGL widget implementation.
#define WGLWIDGET_H

#ifdef RENDERGRAPH_SG
// SG target: WGLWidget is a stub that provides the GL virtual methods
// but doesn't actually do any GL rendering. This allows WaveformWidget
// to compile for both targets.
#include <QWidget>
class WGLWidget : public QWidget {
    Q_OBJECT
  public:
    WGLWidget(QWidget* parent = nullptr)
            : QWidget(parent) {
    }
    virtual ~WGLWidget() = default;
    virtual void paintGL() {
    }
    virtual void initializeGL() {
    }
    virtual void resizeGL(int, int) {
    }
    virtual bool isContextValid() const {
        return false;
    }
    virtual bool shouldRender() const {
        return false;
    }
    virtual void makeCurrentIfNeeded() {
    }
    virtual void doneCurrent() {
    }
    virtual void swapBuffers() {
    }
};
#else
#include "widget/llglwglwidget.h"
#endif

#undef WGLWIDGET_H
