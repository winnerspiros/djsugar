#pragma once

#ifdef MIXXX_USE_LLGL

#include "../rendergraph/context.h"

namespace rendergraph {

/// LGLOpenGLNode - redirects GL calls to LLGL.
/// This allows existing allshader renderers (which inherit from OpenGLNode)
/// to work with LLGL without modification.
class LGLOpenGLNode {
  public:
    explicit LGLOpenGLNode(LLGLContext* context);
    ~LGLOpenGLNode();

    void initialize();

    void glClearColor(GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha) const;
    void glClear(GLbitfield mask) const;
    void glViewport(GLint x, GLint y, GLsizei width, GLsizei height) const;
    void glUseProgram(GLuint program) const;
    void glUniform1f(GLint location, GLfloat v0) const;
    void glUniform2f(GLint location, GLfloat v0, GLfloat v1) const;
    void glUniform3f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2) const;
    void glUniform4f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2, GLfloat v3) const;
    void glUniform1i(GLint location, GLint v0) const;
    void glUniformMatrix4fv(GLint location, GLboolean transpose, const GLfloat* value) const;
    void glBindBuffer(GLenum target, GLuint buffer) const;
    void glBufferData(GLenum target, GLsizeiptr size, const GLvoid* data, GLenum usage) const;
    void glBufferSubData(GLenum target,
            GLintptr offset,
            GLsizeiptr size,
            const GLvoid* data) const;
    void glGenBuffers(GLsizei n, GLuint* buffers) const;
    void glDeleteBuffers(GLsizei n, const GLuint* buffers) const;
    void glEnableVertexAttribArray(GLuint index) const;
    void glDisableVertexAttribArray(GLuint index) const;
    void glVertexAttribPointer(GLuint index,
            GLint size,
            GLenum type,
            GLboolean normalized,
            GLsizei stride,
            const GLvoid* pointer) const;
    void glDrawArrays(GLenum mode, GLint first, GLsizei count) const;
    void glDrawElements(GLenum mode, GLsizei count, GLenum type, const GLvoid* indices) const;
    void glEnable(GLenum cap) const;
    void glDisable(GLenum cap) const;
    void glBlendFunc(GLenum sfactor, GLenum dfactor) const;
    void glScissor(GLint x, GLint y, GLsizei width, GLsizei height) const;
    GLint glGetUniformLocation(GLuint program, const GLchar* name) const;
    GLuint glCreateShader(GLenum type) const;
    void glShaderSource(GLuint shader,
            GLsizei count,
            const GLchar** string,
            const GLint* length) const;
    void glCompileShader(GLuint shader) const;
    void glGetShaderiv(GLuint shader, GLenum pname, GLint* params) const;
    void glGetShaderInfoLog(GLuint shader,
            GLsizei bufSize,
            GLsizei* length,
            GLchar* infoLog) const;
    GLuint glCreateProgram() const;
    void glAttachShader(GLuint program, GLuint shader) const;
    void glLinkProgram(GLuint program) const;
    void glGetProgramiv(GLuint program, GLenum pname, GLint* params) const;
    void glDeleteProgram(GLuint program) const;
    void glDeleteShader(GLuint shader) const;
    void glActiveTexture(GLenum texture) const;
    void glBindTexture(GLenum target, GLuint texture) const;
    void glGenTextures(GLsizei n, GLuint* textures) const;
    void glDeleteTextures(GLsizei n, const GLuint* textures) const;
    void glTexParameteri(GLenum target, GLenum pname, GLint param) const;
    void glTexImage2D(GLenum target,
            GLint level,
            GLint internalformat,
            GLsizei width,
            GLint height,
            GLint border,
            GLenum format,
            GLenum type,
            const GLvoid* pixels) const;

    LLGLContext* context() const {
        return m_pContext;
    }

  private:
    LLGLContext* m_pContext;
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
