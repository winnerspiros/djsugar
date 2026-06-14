#pragma once

#ifdef MIXXX_USE_LLGL

#include <LLGL/LLGL.h>
#include <LLGL/PipelineState.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/Shader.h>

#include <QMatrix4x4>
#include <QString>

#include "util/class.h"

namespace mixxx {
class LLGLShader;
} // namespace mixxx

class mixxx::LLGLShader {
  public:
    LLGLShader();
    ~LLGLShader();

    bool load(const QString& vertexShaderCode, const QString& fragmentShaderCode);

    LLGL::Shader* vertexShader() const {
        return m_pVertexShader;
    }

    LLGL::Shader* fragmentShader() const {
        return m_pFragmentShader;
    }

    LLGL::PipelineState* pipelineState() const {
        return m_pPipelineState;
    }

    void setPipelineState(LLGL::PipelineState* pPipelineState) {
        m_pPipelineState = pPipelineState;
    }

    bool isValid() const {
        return m_pVertexShader != nullptr && m_pFragmentShader != nullptr;
    }

  private:
    LLGL::Shader* m_pVertexShader;
    LLGL::Shader* m_pFragmentShader;
    LLGL::PipelineState* m_pPipelineState;

    DISALLOW_COPY_AND_ASSIGN(LLGLShader);
};

#endif // MIXXX_USE_LLGL
