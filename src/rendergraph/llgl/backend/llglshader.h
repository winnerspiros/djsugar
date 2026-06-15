#pragma once

#ifdef MIXXX_USE_LLGL

#include <LLGL/LLGL.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/Shader.h>
#include <LLGL/PipelineState.h>
#include <LLGL/PipelineLayout.h>
#include <LLGL/Buffer.h>
#include <LLGL/Texture.h>
#include <LLGL/Sampler.h>
#include <LLGL/VertexAttribute.h>
#include <LLGL/Format.h>
#include <LLGL/CommandBuffer.h>

#include <QMatrix4x4>
#include <QVector2D>
#include <QVector3D>
#include <QVector4D>
#include <QString>
#include <QDebug>

#include "util/class.h"

namespace rendergraph {

/// LLGLShader wraps LLGL shader objects and pipeline state,
/// replacing QOpenGLShaderProgram in the LLGL backend.
class LLGLShader {
  public:
    LLGLShader() = default;
    ~LLGLShader() = default;

    bool load(LLGL::RenderSystem* pDevice,
            const QString& vertexShaderCode,
            const QString& fragmentShaderCode,
            const LLGL::VertexAttribute* pAttribs = nullptr,
            std::uint32_t numAttribs = 0);

    LLGL::PipelineState* pipelineState() const {
        return m_pPipelineState;
    }

    LLGL::PipelineLayout* pipelineLayout() const {
        return m_pPipelineLayout;
    }

    LLGL::Shader* vertexShader() const {
        return m_pVertexShader;
    }

    LLGL::Shader* fragmentShader() const {
        return m_pFragmentShader;
    }

    bool isValid() const {
        return m_pPipelineState != nullptr;
    }

    // Uniform setting (called before draw)
    void setUniform(LLGL::CommandBuffer* pCmdBuf,
            int location, float value);
    void setUniform(LLGL::CommandBuffer* pCmdBuf,
            int location, const QVector2D& value);
    void setUniform(LLGL::CommandBuffer* pCmdBuf,
            int location, const QVector3D& value);
    void setUniform(LLGL::CommandBuffer* pCmdBuf,
            int location, const QVector4D& value);
    void setUniform(LLGL::CommandBuffer* pCmdBuf,
            int location, const QMatrix4x4& value);

    // Attribute location query
    int attributeLocation(int attributeIndex) const {
        if (attributeIndex >= 0 && attributeIndex < static_cast<int>(m_attributeLocations.size())) {
            return m_attributeLocations[attributeIndex];
        }
        return -1;
    }

    int uniformLocation(int uniformIndex) const {
        if (uniformIndex >= 0 && uniformIndex < static_cast<int>(m_uniformLocations.size())) {
            return m_uniformLocations[uniformIndex];
        }
        return -1;
    }

  private:
    LLGL::Shader* m_pVertexShader = nullptr;
    LLGL::Shader* m_pFragmentShader = nullptr;
    LLGL::PipelineState* m_pPipelineState = nullptr;
    LLGL::PipelineLayout* m_pPipelineLayout = nullptr;
    std::vector<int> m_attributeLocations;
    std::vector<int> m_uniformLocations;

    DISALLOW_COPY_AND_ASSIGN(LLGLShader);
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
