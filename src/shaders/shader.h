#pragma once

#include <QString>

#include "util/class.h"

namespace rendergraph {
class LLGLShaderProgram;
}

namespace mixxx {
class Shader;
}

class mixxx::Shader {
  public:
    Shader();
    ~Shader();

    void load(const QString& vertexShader, const QString& fragmentShader);
    bool bind();
    void release();

  private:
    std::unique_ptr<rendergraph::LLGLShaderProgram> m_pShaderProgram;
    DISALLOW_COPY_AND_ASSIGN(Shader)
};
