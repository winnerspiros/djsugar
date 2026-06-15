#pragma once

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

// Define GL types that would normally come from Qt OpenGL or GLEW
typedef unsigned int GLenum;
typedef unsigned int GLuint;
typedef float GLfloat;
typedef int GLint;
typedef int GLsizei;
typedef void GLvoid;
typedef char GLchar;
typedef unsigned short GLushort;
typedef unsigned char GLubyte;

// Common GL constants needed for draw calls
static const GLenum GL_TRIANGLES = 0x0004;
static const GLenum GL_TRIANGLE_STRIP = 0x0005;
static const GLenum GL_LINES = 0x0001;
static const GLenum GL_LINE_STRIP = 0x0003;
static const GLenum GL_POINTS = 0x0000;
static const GLenum GL_TEXTURE_2D = 0x0DE1;
static const GLenum GL_TEXTURE0 = 0x84C0;
static const GLenum GL_RGBA = 0x1908;
static const GLenum GL_UNSIGNED_BYTE = 0x1401;
static const GLenum GL_LINEAR = 0x2601;
static const GLenum GL_TEXTURE_MIN_FILTER = 0x2801;
static const GLenum GL_TEXTURE_MAG_FILTER = 0x2800;

#include <QMatrix4x4>
#include <QVector2D>
#include <QVector3D>
#include <QVector4D>
#include <QString>
#include <QDebug>
#include <vector>

#include "util/class.h"

namespace rendergraph {
class LLGLContext;

/// LLGLShaderProgram wraps LLGL shader objects and pipeline state,
/// providing the same interface as QOpenGLShaderProgram.
/// Auto-detects the active backend (OpenGL/Metal/D3D11) and uses
/// the appropriate shader profile.
class LLGLShaderProgram {
  public:
    LLGLShaderProgram();
    ~LLGLShaderProgram();

    bool addShaderFromSourceCode(const QString& vertexShader, const QString& fragmentShader);
    bool link();

    bool bind();
    void release();

    void setUniformValue(int location, GLfloat value);
    void setUniformValue(int location, const QVector2D& value);
    void setUniformValue(int location, const QVector3D& value);
    void setUniformValue(int location, const QVector4D& value);
    void setUniformValue(int location, const QMatrix4x4& value);
    void setUniformValue(int location, GLuint value);

    void enableAttributeArray(int location);
    void disableAttributeArray(int location);
    void setAttributeArray(int location, const float* data, int tupleSize, int stride);

    void setUniformValue(int location, LLGL::Texture* texture) { Q_UNUSED(location); Q_UNUSED(texture); }

    int uniformLocation(const char* name) const;
    int attributeLocation(const char* name) const;

    bool isValid() const {
        return m_pPipelineState != nullptr;
    }

    // Direct LLGL access
    LLGL::PipelineState* pipelineState() const { return m_pPipelineState; }
    LLGL::Buffer* vertexBuffer() const { return m_pVertexBuffer; }
    LLGL::Buffer* uniformBuffer() const { return m_pUniformBuffer; }
    LLGL::RenderSystem* device() const { return m_pDevice; }

    void setContext(LLGLContext* pContext);
    void setCommandBuffer(LLGL::CommandBuffer* pCmdBuf) { m_pCmdBuf = pCmdBuf; }

    // Create/update vertex buffer from data
    void updateVertexBuffer(const float* data, std::uint32_t vertexCount, std::uint32_t stride);

    void drawArrays(GLenum mode, int first, int count);

  private:
    bool createPipelineState(const QString& vertexShader, const QString& fragmentShader);
    void destroyResources();
    void detectProfiles(QString& vsProfile, QString& fsProfile);

    LLGLContext* m_pContext;
    LLGL::RenderSystem* m_pDevice;
    LLGL::CommandBuffer* m_pCmdBuf;

    LLGL::Shader* m_pVertexShader;
    LLGL::Shader* m_pFragmentShader;
    LLGL::PipelineState* m_pPipelineState;
    LLGL::PipelineLayout* m_pPipelineLayout;
    LLGL::Buffer* m_pVertexBuffer;
    LLGL::Buffer* m_pUniformBuffer;

    QString m_vertexShaderSource;
    QString m_fragmentShaderSource;

    std::vector<int> m_attributeLocations;
    std::vector<int> m_uniformLocations;

    bool m_pipelineCreated;
    bool m_bound;
    std::uint32_t m_vertexStride;
    std::uint32_t m_vertexCount;

    static std::uint32_t s_nextShaderId;
    std::uint32_t m_shaderId;

    DISALLOW_COPY_AND_ASSIGN(LLGLShaderProgram);
};

} // namespace rendergraph
