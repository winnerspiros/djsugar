#include "waveform/renderers/allshader/waveformrenderbackground.h"

#include "waveform/renderers/waveformwidgetrenderer.h"

#ifdef MIXXX_USE_LLGL
#include "rendergraph/llgl/rendergraph/context.h"
#endif

namespace allshader {

WaveformRenderBackground::WaveformRenderBackground(
        WaveformWidgetRenderer* waveformWidgetRenderer)
        : WaveformRenderer(waveformWidgetRenderer),
          m_backgroundColor(0, 0, 0) {
}

void WaveformRenderBackground::setup(const QDomNode& node,
        const SkinContext& skinContext) {
    m_backgroundColor = m_waveformRenderer->getWaveformSignalColors()->getBgColor();

    QString backgroundPixmapPath = skinContext.selectString(node, "BgPixmap");
    if (!backgroundPixmapPath.isEmpty()) {
        qWarning() << "WaveformView BgPixmap is not supported by "
                      "allshader::WaveformRenderBackground";
    }
}

void WaveformRenderBackground::paintGL() {
#ifdef MIXXX_USE_LLGL
    // LLGL: clear is handled by the widget's beginFrame
    Q_UNUSED(m_backgroundColor);
#else
    glClearColor(static_cast<float>(m_backgroundColor.redF()),
            static_cast<float>(m_backgroundColor.greenF()),
            static_cast<float>(m_backgroundColor.blueF()),
            1.f);
    glClear(GL_COLOR_BUFFER_BIT);
#endif
}

} // namespace allshader
