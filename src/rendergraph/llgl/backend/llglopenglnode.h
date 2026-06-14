#pragma once

#ifdef MIXXX_USE_LLGL

#include <QOpenGLFunctions>
#include "rendergraph/llgl/rendergraph/context.h"

namespace rendergraph {

/// LGLOpenGLNode - QOpenGLFunctions subclass that redirects GL calls to LLGL.
/// This allows existing allshader renderers (which inherit from OpenGLNode
/// = QOpenGLFunctions) to work with LLGL without modification.
class LGLOpenGLNode : public QOpenGLFunctions {
  public:
    explicit LGLOpenGLNode(LLGLContext* context);
    ~LGLOpenGLNode() override;

    void initialize() override;

    void glClearColor(GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha) override;
    void glClear(GLbitfield mask) override;
    void glViewport(GLint x, GLint y, GLsizei width, GLsizei height) override;
    void glUseProgram(GLuint program) override;
    void glUniform1f(GLint location, GLfloat v0) override;
    void glUniform2f(GLint location, GLfloat v0, GLfloat v1) override;
    void glUniform3f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2) override;
    void glUniform4f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2, GLfloat v3) override;
    void glUniform1i(GLint location, GLint v0) override;
    void glUniformMatrix4fv(GLint location, GLboolean transpose, const GLfloat* value) override;
    void glBindBuffer(GLenum target, GLuint buffer) override;
    void glBufferData(GLenum target, GLsizeiptr size, const GLvoid* data, GLenum usage) override;
    void glBufferSubData(GLenum target, GLintptr offset, GLsizeiptr size, const GLvoid* data) override;
    void glGenBuffers(GLsizei n, GLuint* buffers) override;
    void glDeleteBuffers(GLsizei n, const GLuint* buffers) override;
    void glEnableVertexAttribArray(GLuint index) override;
    void glDisableVertexAttribArray(GLuint index) override;
    void glVertexAttribPointer(GLuint index, GLint size, GLenum type, GLboolean normalized, GLsizei stride, const GLvoid* pointer) override;
    void glDrawArrays(GLenum mode, GLint first, GLsizei count) override;
    void glDrawElements(GLenum mode, GLsizei count, GLenum type, const GLvoid* indices) override;
    void glEnable(GLenum cap) override;
    void glDisable(GLenum cap) override;
    void glBlendFunc(GLenum sfactor, GLenum dfactor) override;
    void glScissor(GLint x, GLint y, GLsizei width, GLsizei height) override;
    GLint glGetUniformLocation(GLuint program, const GLchar* name) override;
    GLuint glCreateShader(GLenum type) override;
    void glShaderSource(GLuint shader, GLsizei count, const GLchar** string, const GLint* length) override;
    void glCompileShader(GLuint shader) override;
    void glGetShaderiv(GLuint shader, GLenum pname, GLint* params) override;
    void glGetShaderInfoLog(GLuint shader, GLsizei bufSize, GLsizei* length, GLchar* infoLog) override;
    GLuint glCreateProgram() override;
    void glAttachShader(GLuint program, GLuint shader) override;
    void glLinkProgram(GLuint program) override;
    void glGetProgramiv(GLuint program, GLenum pname, GLint* params) override;
    void glDeleteProgram(GLuint program) override;
    void glDeleteShader(GLuint shader) override;
    void glActiveTexture(GLenum texture) override;
    void glBindTexture(GLenum target, GLuint texture) override;
    void glGenTextures(GLsizei n, GLuint* textures) override;
    void glDeleteTextures(GLsizei n, const GLuint* textures) override;
    void glTexParameteri(GLenum target, GLenum pname, GLint param) override;
    void glTexImage2D(GLenum target, GLint level, GLint internalformat,
                      GLsizei width, GLint height, GLint border,
                      GLenum format, GLenum type, const GLvoid* pixels) override;

    LLGLContext* context() const { return m_pContext; }

  private:
    LLGLContext* m_pContext;
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
