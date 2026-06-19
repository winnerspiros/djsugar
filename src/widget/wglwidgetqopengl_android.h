#pragma once

#ifndef WGLWIDGET_H
#error "Do not include this file, include wglwidget.h instead"
#endif

#include <QWidget>

////////////////////////////////
// Stub WGLWidget for Android (QOPENGL=OFF)
// Prevents QOpenGLWindow crash on devices where OpenGL context creation fails
////////////////////////////////

class QPaintDevice;
class TrackDropTarget;

class WGLWidget : public QWidget {
  public:
    WGLWidget(QWidget* parent) : QWidget(parent) {}
    ~WGLWidget() = default;

    bool isContextValid() const { return false; }
    bool shouldRender() const { return false; }
    void makeCurrentIfNeeded() {}
    void doneCurrent() {}
    void swapBuffers() {}

    virtual void paintGL() {}
    virtual void resizeGL(int w, int h) {}
    virtual void initializeGL() {}

    void setTrackDropTarget(TrackDropTarget* pTarget) {}
    TrackDropTarget* trackDropTarget() const { return nullptr; }

    void* getOpenGLWindow() const { return nullptr; }

  protected:
    void showEvent(QShowEvent* event) override { QWidget::showEvent(event); }
    void resizeEvent(QResizeEvent* event) override { QWidget::resizeEvent(event); }

    QPaintDevice* paintDevice() { return nullptr; }
};
