#include "llglmaterialshader.h"

#include "../llgl/backend/shadersourceprovider.h"

using namespace rendergraph;

// LLGLMaterialShader constructor

bool LLGLMaterialShader::load(const QString& vertexShader, const QString& fragmentShader) {
    m_loaded = m_shader.addShaderFromSourceCode(vertexShader, fragmentShader);
    return m_loaded;
}

bool LLGLMaterialShader::link() {
    return m_loaded && m_shader.link();
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
    m_shader.setUniformValue(location, static_cast<GLfloat>(value));
}

void LLGLMaterialShader::enableAttributeArray(int location) {
    m_shader.enableAttributeArray(location);
}

void LLGLMaterialShader::disableAttributeArray(int location) {
    m_shader.disableAttributeArray(location);
}

void LLGLMaterialShader::setAttributeArray(int location, const float* data, int tupleSize, int stride) {
    m_shader.setAttributeArray(location, data, tupleSize, stride);
}

int LLGLMaterialShader::uniformLocation(int index) const {
    if (index < 0 || index >= static_cast<int>(m_uniformLocations.size())) {
        return -1;
    }
    return m_uniformLocations[index];
}

int LLGLMaterialShader::attributeLocation(int index) const {
    if (index < 0 || index >= static_cast<int>(m_attributeLocations.size())) {
        return -1;
    }
    return m_attributeLocations[index];
}