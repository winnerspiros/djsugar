#include "shaders/shader.h"

#include <memory>

#include "rendergraph/llgl/backend/llglshaderprogram.h"
#include "util/assert.h"

using namespace mixxx;

Shader::Shader()
        : m_pShaderProgram(std::make_unique<rendergraph::LLGLShaderProgram>()) {
}

Shader::~Shader() = default;

bool Shader::bind() {
    return m_pShaderProgram->bind();
}

void Shader::release() {
    m_pShaderProgram->release();
}

void Shader::load(const QString& vertexShaderCode, const QString& fragmentShaderCode) {
    VERIFY_OR_DEBUG_ASSERT(m_pShaderProgram->addShaderFromSourceCode(
            vertexShaderCode, fragmentShaderCode)) {
        return;
    }

    VERIFY_OR_DEBUG_ASSERT(m_pShaderProgram->link()) {
        return;
    }
}
