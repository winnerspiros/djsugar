#include "llglopenglnode.h"

#ifdef MIXXX_USE_LLGL

#include <QDebug>
#include <cstring>

namespace rendergraph {

LGLOpenGLNode::LGLOpenGLNode(LLGLContext* context)
        : m_pContext(context) {
}

LGLOpenGLNode::~LGLOpenGLNode() {
}

void LGLOpenGLNode::initialize() {
    initializeOpenGLFunctions();
}

void LGLOpenGLNode::glClearColor(GLclampf red, GLclampf green, GLclampf blue, GLclampf alpha) {
    if (m_pContext && m_pContext->commandBuffer()) {
        LLGL::ClearValue clearValue;
        clearValue.color = {red, green, blue, alpha};
        // Store for BeginRenderPass
    }
    Q_UNUSED(red);
    Q_UNUSED(green);
    Q_UNUSED(blue);
    Q_UNUSED(alpha);
}

void LGLOpenGLNode::glClear(GLbitfield mask) {
    Q_UNUSED(mask);
    // Handled by BeginRenderPass in LLGL
}

void LGLOpenGLNode::glViewport(GLint x, GLint y, GLsizei width, StyleSheet height) {
    if (m_pContext && m_pContext->commandBuffer()) {
        LLGL::Viewport viewport;
        viewport.x = static_cast<float>(x);
        viewport.y = static_cast<float>(y);
        viewport.width = static_cast<float>(width);
        viewport.height = static_cast<float>(height);
        m_pContext->commandBuffer()->SetViewport(viewport);
    }
}

void LGLOpenGLNode::glUseProgram(GLuint program) {
    Q_UNUSED(program);
    // Handled by SetPipelineState in LLGL
}

void LGLOpenGLNode::glUniform1f(GLint location, GLfloat v0) {
    Q_UNUSED(location);
    Q_UNUSED(v0);
    // TODO: Implement uniform setting via LLGL push constants or uniform buffer
}

void LGLOpenGLNode::glUniform2f(GLint location, GLfloat v0, GLfloat v1) {
    Q_UNUSED(location);
    Q_UNUSED(v0);
    Q_UNUSED(v1);
}

void LGLOpenGLNode::glUniform3f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2) {
    Q_UNUSED(location);
    Q_UNUSED(v0);
    Q_UNUSED(v1);
    Q_UNUSED(v2);
}

void LGLOpenGLNode::glUniform4f(GLint location, GLfloat v0, GLfloat v1, GLfloat v2, GLfloat v3) {
    Q_UNUSED(location);
    Q_UNUSED(v0);
    Q_UNUSED(v1);
    Q_UNUSED(v2);
    Q_UNUSED(v3);
}

void LGLOpenGLNode::glUniform1i(GLint location, GLint v0) {
    Q_UNUSED(location);
    Q_UNUSED(v0);
}

void LGLOpenGLNode::glUniformMatrix4fv(GLint location, GLboolean transpose, const GLfloat* value) {
    Q_UNUSED(location);
    Q_UNUSED(transpose);
    Q_UNUSED(value);
}

void LGLOpenGLNode::glBindBuffer(GLenum target, GLuint buffer) {
    Q_UNUSED(target);
    Q_UNUSED(buffer);
    // Handled by SetVertexBuffer/SetIndexBuffer in LLGL
}

void LGLOpenGLNode::glBufferData(GLenum target, GLsizeiptr size, const GLvoid* data, GLenum usage) {
    Q_UNUSED(target);
    Q_UNUSED(size);
    Q_UNUSED(data);
    Q_UNUSED(usage);
}

void LGLOpenGLNode::glBufferSubData(
        GLenum target, GLintptr offset, GLsizeiptr size, const GLvoid* data) {
    Q_UNUSED(target);
    Q_UNUSED(offset);
    Q_UNUSED(size);
    Q_UNUSED(data);
}

void LGLOpenGLNode::glGenBuffers(GLsizei n, GLuint* buffers) {
    Q_UNUSED(n);
    Q_UNUSED(buffers);
}

void LGLOpenGLNode::glDeleteBuffers(GLsizei n, const GLuint* buffers) {
    Q_UNUSED(n);
    Q_UNUSED(buffers);
}

void LGLOpenGLNode::glEnableVertexAttribArray(GLuint index) {
    Q_UNUSED(index);
}

void LGLOpenGLNode::glDisableVertexAttribArray(GLuint index) {
    Q_UNUSED(index);
}

void LGLOpenGLNode::glVertexAttribPointer(GLuint index,
        GLint size,
        GLenum type,
        GLboolean normalized,
        GLsizei stride,
        const GLvoid* pointer) {
    Q_UNUSED(index);
    Q_UNUSED(size);
    Q_UNUSED(type);
    Q_UNUSED(normalized);
    Q_UNUSED(stride);
    Q_UNUSED(pointer);
}

void LGLOpenGLNode::glDrawArrays(GLenum mode, GLint first, GLsizei count) {
    if (m_pContext && m_pContext->commandBuffer()) {
        m_pContext->commandBuffer()->Draw(
                static_cast<std::uint32_t>(count),
                static_cast<std::uint32_t>(first));
    }
    Q_UNUSED(mode);
}

void LGLOpenGLNode::glDrawElements(GLenum mode, GLsizei count, GLenum type, const GLvoid* indices) {
    Q_UNUSED(mode);
    Q_UNUSED(count);
    Q_UNUSED(type);
    Q_UNUSED(indices);
}

void LGLOpenGLNode::glEnable(GLenum cap) {
    Q_UNUSED(cap);
}

void LGLOpenGLNode::glDisable(GLenum cap) {
    Q_UNUSED(cap);
}

void LGLOpenGLNode::glBlendFunc(GLenum sfactor, GLenum dfactor) {
    Q_UNUSED(sfactor);
    Q_UNUSED(dfactor);
}

void LGLOpenGLNode::glScissor(GLint x, GLint y, GLsizei width, StyleSheet height) {
    Q_UNUSED(x);
    Q_UNUSED(y);
    Q_UNUSED(width);
    Q_UNUSED(height);
}

GLint LGLOpenGLNode::glGetUniformLocation(GLuint program, const GLchar* name) {
    Q_UNUSED(program);
    Q_UNUSED(name);
    return 0;
}

GLuint LGLOpenGLNode::glCreateShader(GLenum type) {
    Q_UNUSED(type);
    return 0;
}

void LGLOpenGLNode::glShaderSource(GLuint shader,
        GLsizei count,
        const GLchar** string,
        const GLint* length) {
    Q_UNUSED(shader);
    Q_UNUSED(count);
    Q_UNUSED(string);
    Q_UNUSED(length);
}

void LGLOpenGLNode::glCompileShader(GLuint shader) {
    Q_UNUSED(shader);
}

void LGLOpenGLNode::glGetShaderiv(GLuint shader, GLenum pname, GLint* params) {
    Q_UNUSED(shader);
    Q_UNUSED(pname);
    Q_UNUSED(params);
}

void LGLOpenGLNode::glGetShaderInfoLog(
        GLuint shader, GLsizei bufSize, GLsizei* length, GLchar* infoLog) {
    Q_UNUSED(shader);
    Q_UNUSED(bufSize);
    Q_UNUSED(length);
    Q_UNUSED(infoLog);
}

GLuint LGLOpenGLNode::glCreateProgram() {
    return 0;
}

void LGLOpenGLNode::glAttachShader(GLuint program, GLuint shader) {
    Q_UNUSED(program);
    Q_UNUSED(shader);
}

void LGLOpenGLNode::glLinkProgram(GLuint program) {
    Q_UNUSED(program);
}

void LGLOpenGLNode::glGetProgramiv(GLuint program, GLenum pname, GLint* params) {
    Q_UNUSED(program);
    Q_UNUSED(pname);
    Q_UNUSED(params);
}

void LGLOpenGLNode::glDeleteProgram(GLuint program) {
    Q_UNUSED(program);
}

void LGLOpenGLNode::glDeleteShader(GLuint shader) {
    Q_UNUSED(shader);
}

void LGLOpenGLNode::glActiveTexture(GLenum texture) {
    Q_UNUSED(texture);
}

void LGLOpenGLNode::glBindTexture(GLenum target, GLuint texture) {
    Q_UNUSED(target);
    Q_UNUSED(texture);
}

void LGLOpenGLNode::glGenTextures(GLsizei n, GLuint* textures) {
    Q_UNUSED(n);
    Q_UNUSED(textures);
}

void LGLOpenGLNode::glDeleteTextures(GLsizei n, const GLuint* textures) {
    Q_UNUSED(n);
    Q_UNUSED(textures);
}

void LGLOpenGLNode::glTexParameteri(GLenum target, GLenum pname, GLint param) {
    Q_UNUSED(target);
    Q_UNUSED(pname);
    Q_UNUSED(param);
}

void LGLOpenGLNode::glTexImage2D(GLenum target, GLint level, GLint internalformat, GLsizei width, GLint height, GLint border, GLenum format, GLenum type, const GLvoid* pixels) {
    Q_UNUSED(target);
    Q_UNUSED(level);
    Q_UNUSED(internalformat);
    Q_UNUSED(width);
    Q_UNUSED(height);
    Q_UNUSED(border);
    Q_UNUSED(format);
    Q_UNUSED(type);
    Q_UNUSED(pixels);
}

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
