#include "rendergraph/opengl/backend/shadercache.h"

#ifdef MIXXX_USE_LLGL

#include <QDir>
#include <QFile>
#include <QTextStream>
#include <QDebug>

using namespace rendergraph;

LLGLMaterialShader::LLGLMaterialShader(const char* vertexShaderFile,
        const char* fragmentShaderFile,
        const UniformSet& uniforms,
        const AttributeSet& attributeSet)
        : m_loaded(false) {
    Q_UNUSED(uniforms);
    Q_UNUSED(attributeSet);
    m_vertexFile = QString(vertexShaderFile);
    m_fragmentFile = QString(fragmentShaderFile);
}

bool LLGLMaterialShader::load(const QString& vertexShader, const QString& fragmentShader) {
    return m_shader.addShaderFromSourceCode(vertexShader, fragmentShader);
}

bool LLGLMaterialShader::link() {
    return m_shader.link();
}

bool LLGLMaterialShader::bind() {
    return m_shader.bind();
}

void LLGLMaterialShader::release() {
    m_shader.release();
}

void LLGLMaterialShader::setUniformValue(int location, GLfloat value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::setUniformValue(int location, const QVector2D& value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::setUniformValue(int location, const QVector3D& value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::setUniformValue(int location, const QVector4D& value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::setUniformValue(int location, const QMatrix4x4& value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::setUniformValue(int location, GLuint value) {
    m_shader.setUniformValue(location, value);
}

void LLGLMaterialShader::enableAttributeArray(int location) {
    m_shader.enableAttributeArray(location);
}

void LLGLMaterialShader::disableAttributeArray(int location) {
    m_shader.disableAttributeArray(location);
}

void LLGLMaterialShader::setAttributeArray(int location, const float* data,
        int tupleSize, int stride) {
    m_shader.setAttributeArray(location, data, tupleSize, stride);
}

void LLGLMaterialShader::updateVertexBuffer(const float* data,
        std::uint32_t vertexCount, std::uint32_t stride) {
    m_shader.updateVertexBuffer(data, vertexCount, stride);
}

void LLGLMaterialShader::drawArrays(GLenum mode, int first, int count) {
    m_shader.drawArrays(mode, first, count);
}

int LLGLMaterialShader::uniformLocation(int index) const {
    return m_shader.uniformLocation("");
}

int LLGLMaterialShader::attributeLocation(int index) const {
    static const char* attribNames[] = {"position", "color"};
    if (index >= 0 && index < 2) {
        return m_shader.attributeLocation(attribNames[index]);
    }
    return -1;
}

#endif // MIXXX_USE_LLGL
