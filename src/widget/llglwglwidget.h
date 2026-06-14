#pragma once

#ifdef MIXXX_USE_LLGL

#include <QWidget>
#include <QTimer>
#include <memory>

#include "rendergraph/llgl/rendergraph/context.h"

class TrackDropTarget;

/// LLGLWGLWidget replaces WGLWidget when LLGL is the rendering backend.
/// It creates an LLGL swap chain for the widget's native window and
/// drives the rendering loop through LLGL command buffers.
///
/// Existing allshader renderers (which override paintGL/initializeGL/resizeGL)
/// work unchanged because this widget calls them at the right times.
class LLGLWGLWidget : public QWidget {
    Q_OBJECT
  public:
    LLGLWGLWidget(QWidget* parent);
    ~LLGLWGLWidget() override;

    bool isContextValid() const;
    bool shouldRender() const;

    void makeCurrentIfNeeded();
    void doneCurrent();
    void swapBuffers();

    virtual void paintGL();
    virtual void resizeGL(int w, int h);
    virtual void initializeGL();

    void setTrackDropTarget(TrackDropTarget* pTarget);
    TrackDropTarget* trackDropTarget() const;

  protected:
    void showEvent(QShowEvent* event) override;
    void resizeEvent(QResizeEvent* event) override;
    void paintEvent(QPaintEvent* event) override;

  private slots:
    void onRenderTimer();

  private:
    std::unique_ptr<rendergraph::LLGLContext> m_pContext;
    QTimer* m_pRenderTimer;
    TrackDropTarget* m_pTrackDropTarget;
    bool m_initialized;
    bool m_initOk;
};

#endif // MIXXX_USE_LLGL
