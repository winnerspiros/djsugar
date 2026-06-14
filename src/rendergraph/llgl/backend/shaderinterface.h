#pragma once

#include <QMatrix4x4>
#include <QVector2D>
#include <QVector3D>
#include <QVector4D>
#include <QOpenGLTexture>

namespace rendergraph {

/// Abstract shader interface that works with both OpenGL and LLGL backends.
/// This replaces direct use of QOpenGLShaderProgram in the rendering code.
class ShaderInterface {
  public:
    virtual ~ShaderInterface() = default;

    virtual bool bind() = 0;
    virtual void release() = 0;

    virtual void setUniformValue(int location, GLfloat value) = 0;
    virtual void setUniformValue(int location, const QVector2D& value) = 0;
    virtual void setUniformValue(int location, const QVector3D& value) = 0;
    virtual void setUniformValue(int location, const QVector4D& value) = 0;
    virtual void setUniformValue(int location, const QMatrix4x4& value) = 0;
    virtual void setUniformValue(int location, GLuint value) = 0;

    virtual void enableAttributeArray(int location) = 0;
    virtual void disableAttributeArray(int location) = 0;
    virtual void setAttributeArray(int location, const float* data,
            int tupleSize, int stride) = 0;

    virtual int uniformLocation(int index) const = 0;
    virtual int attributeLocation(int index) const = 0;

    virtual void drawArrays(GLenum mode, int first, int count) = 0;

    virtual bool isValid() const = 0;
};

} // namespace rendergraph
