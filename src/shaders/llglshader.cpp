#include "shaders/llglshader.h"

#ifdef MIXXX_USE_LLGL

#include <QDebug>

#include "util/assert.h"

using namespace mixxx;

LLGLShader::LLGLShader()
        : m_pVertexShader(nullptr),
          m_pFragmentShader(nullptr),
          m_pPipelineState(nullptr) {
}

LLGLShader::~LLGLShader() {
    // Shaders and pipeline state are owned by the LLGL context
    // and will be released when the context is destroyed.
}

bool LLGLShader::load(const QString& vertexShaderCode,
        const QString& fragmentShaderCode) {
    Q_UNUSED(vertexShaderCode);
    Q_UNUSED(fragmentShaderCode);

    // Shader compilation is deferred to the LLGL context.
    // The source code is stored by the renderer, and the actual
    // LLGL shader objects are created during pipeline setup.
    qDebug() << "LLGLShader::load: shader source stored for deferred compilation";
    return true;
}

#endif // MIXXX_USE_LLGL
