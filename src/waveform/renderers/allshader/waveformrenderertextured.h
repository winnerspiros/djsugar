#pragma once

#include "rendergraph/openglnode.h"
#include "track/track_decl.h"
#include "waveform/renderers/allshader/waveformrenderersignalbase.h"
#include "waveform/waveform.h"
#include "waveform/widgets/waveformwidgettype.h"

namespace allshader {
class WaveformRendererTextured;
} // namespace allshader

// High-detail textured waveform renderer.
// In LLGL mode, this falls back to the base class (simple colored rectangles).
// Full LLGL port of the framebuffer/shader pipeline is a future task.
class allshader::WaveformRendererTextured final : public allshader::WaveformRendererSignalBase,
                                                  public rendergraph::OpenGLNode {
    Q_OBJECT
  public:
    explicit WaveformRendererTextured(WaveformWidgetRenderer* waveformWidget,
            WaveformWidgetType::Type t,
            ::WaveformRendererAbstract::PositionSource type =
                    ::WaveformRendererAbstract::Play,
            ::WaveformRendererSignalBase::Options options =
                    ::WaveformRendererSignalBase::Option::None);
    ~WaveformRendererTextured() override;

    void onSetup(const QDomNode& node) override;

    void initializeGL() override;
    void paintGL() override;
    void resizeGL(int w, int h) override;

    bool supportsSlip() const override {
        return true;
    }

    void onSetTrack() override;

  public slots:
    void slotWaveformUpdated();

  private:
    void createGeometry();

    bool m_isSlipRenderer;
    ::WaveformRendererSignalBase::Options m_options;
    WaveformWidgetType::Type m_type;
    const QString m_fragShader;
};
