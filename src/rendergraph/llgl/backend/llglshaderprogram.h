#pragma once

#ifdef MIXXX_USE_LLGL

#include <LLGL/LLGL.h>
#include <LLGL/RenderSystem.h>
#include <LLGL/Device.h>
#include <LLGL/Shader.h>
#include <LLGL/PipelineState.h>
#include <LLGL/PipelineLayout.h>
#include <LLGL/Buffer.h>
#include <LLGL/Texture.h>
#include <LLGL/Sampler.h>
#include <LLGL/VertexAttribute.h>
#include <LLGL/Format.h>
#include <LLGL/CommandBuffer.h>
#include <LLGL/RenderPass.h>
#include <LLGL/QueryHeap.h>

#include <QOpenGLTexture>
#include <QMatrix4x4>
#include <QVector2D>
#include <QVector3D>
#include <QVector4D>
#include <QString>
#include <QDebug>
#include <vector>
#include <memory>

namespace rendergraph {

/// LLGLShaderProgram wraps LLGL shader objects and pipeline state,
/// providing the same interface as QOpenGLShaderProgram.
///
/// This allows existing allshader renderers to work with LLGL
/// without modification — they call bind(), setUniformValue(),
/// enableAttributeArray(), etc. as usual.
class LLGLShaderProgram {
  public:
    LLGLShaderProgram();
    ~LLGLShaderProgram();

    // Load shaders from source code
    bool addShaderFromSourceCode(const QString& vertexShader, const QString& fragmentShader);
    bool link();

    // Bind/unbind (set pipeline state)
    bool bind();
    void release();

    // Uniform setting
    void setUniformValue(int location, GLfloat value);
    void setUniformValue(int location, const QVector2D& value);
    void setUniformValue(int location, const QVector3D& value);
    void setUniformValue(int location, const QVector4D& value);
    void setUniformValue(int location, const QMatrix4x4& value);
    void setUniformValue(int location, GLuint value);

    // Attribute management
    void enableAttributeArray(int location);
    void disableAttributeArray(int location);
    void setAttributeArray(int location, const float* data, int tupleSize, int stride);

    // Texture binding
    void setUniformValue(int location, QOpenGLTexture* texture);

    // Location queries
    int uniformLocation(const char* name) const;
    int attributeLocation(const char* name) const;

    // State
    bool isLinked() const {
        return m_pPipelineState != nullptr;
    }

    // Access to underlying LLGL objects
    LLGL::PipelineState* pipelineState() const {
        return m_pPipelineState;
    }
    LLGL::Buffer* vertexBuffer() const {
        return m_pVertexBuffer;
    }
    LLGL::RenderSystem* device() const {
        return m_pDevice;
    }

    // Set the LLGL context (called by the widget)
    void setContext(LLGLContext* pContext) {
        m_pContext = pContext;
    }

    // Set the command buffer for recording
    void setCommandBuffer(LLGL::CommandBuffer* pCmdBuf) {
        m_pCmdBuf = pCmdBuf;
    }

    // Draw (called after setting up vertex attributes and uniforms)
    void drawArrays(GLenum mode, int first, int count);

  private:
    LLGLContext* m_pContext;
    LLGL::RenderSystem* m_pDevice;
    LLGL::CommandBuffer* m_pCmdBuf;

    LLGL::Shader* m_pVertexShader;
    LLGL::Shader* m_pFragmentShader;
    LLGL::PipelineState* m_pPipelineState;
    LLGL::PipelineLayout* m_pPipelineLayout;

    // Vertex buffer management
    LLGL::Buffer* m_pVertexBuffer;
    std::vector<LLGL::VertexAttribute> m_vertexAttribs;
    uint32_t m_vertexStride;

    // Uniform data (staged for upload)
    std::vector<uint8_t> m_uniformData;
    LLGL::Buffer* m_pUniformBuffer;

    // Texture management
    struct TextureBinding {
        QOpenGLTexture* pTexture;
        int bindingPoint;
    };
    std::vector<TextureBinding> m_textures;

    // Attribute state
    struct AttributeState {
        bool enabled;
        int location;
        const float* data;
        int tupleSize;
        int stride;
    };
    std::vector<AttributeState> m_attribState;

    bool m_bound;

    static std::uint32_t s_nextShaderId;
    std::uint32_t m_shaderId;
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
