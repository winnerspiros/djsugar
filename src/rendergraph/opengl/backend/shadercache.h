#pragma once

#include <unordered_map>
#include <memory>

#include "rendergraph/material.h"

#ifdef MIXXX_USE_LLGL
#include "rendergraph/llgl/backend/llglshaderprogram.h"
#endif

namespace rendergraph {

#ifdef MIXXX_USE_LLGL
/// LLGLMaterialShader wraps LLGLShaderProgram to provide the same interface
/// as MaterialShader (which wraps QOpenGLShaderProgram).
class LLGLMaterialShader {
  public:
    LLGLMaterialShader(const char* vertexShaderFile,
            const char* fragmentShaderFile,
            const UniformSet& uniforms,
            const AttributeSet& attributeSet);

    bool load(const QString& vertexShader, const QString& fragmentShader);
    bool link();

    // QOpenGLShaderProgram-compatible interface
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

    int uniformLocation(int index) const;
    int attributeLocation(int index) const;

    GLuint programId() const { return 0; }

    bool isLinked() const { return m_shader.isValid(); }

    // LLGL-specific
    LLGLShaderProgram* llglShader() { return &m_shader; }
    void setContext(LLGLContext* pContext) { m_shader.setContext(pContext); }
    void setCommandBuffer(LLGL::CommandBuffer* pCmdBuf) { m_shader.setCommandBuffer(pCmdBuf); }

    // Update vertex buffer from geometry data
    void updateVertexBuffer(const float* data, std::uint32_t vertexCount, std::uint32_t stride) {
        m_shader.updateVertexBuffer(data, vertexCount, stride);
    }

    void drawArrays(GLenum mode, int first, int count) {
        m_shader.drawArrays(mode, first, count);
    }

  private:
    LLGLShaderProgram m_shader;
    QString m_vertexFile;
    QString m_fragmentFile;
    bool m_loaded;
};
#endif

class ShaderCache {
  private:
#ifdef MIXXX_USE_LLGL
    static std::unordered_map<MaterialType*,
            std::shared_ptr<LLGLMaterialShader>>&
    map() {
        static std::unordered_map<MaterialType*,
                std::shared_ptr<LLGLMaterialShader>>
                s_map;
        return s_map;
    }
#else
    static std::unordered_map<MaterialType*,
            std::shared_ptr<MaterialShader>>&
    map() {
        static std::unordered_map<MaterialType*,
                std::shared_ptr<MaterialShader>>
                s_map;
        return s_map;
    }
#endif

  public:
#ifdef MIXXX_USE_LLGL
    static std::shared_ptr<LLGLMaterialShader> getShaderForMaterial(
            Material* pMaterial) {
        auto iter = map().find(pMaterial->type());
        if (iter != map().end()) {
            return iter->second;
        }
        auto pResult = std::shared_ptr<LLGLMaterialShader>(
                pMaterial->createLLGLShader());
        map().insert(std::pair<MaterialType*,
                std::shared_ptr<LLGLMaterialShader>>{
                pMaterial->type(), pResult});
        return pResult;
    }
#else
    static std::shared_ptr<MaterialShader> getShaderForMaterial(
            Material* pMaterial) {
        auto iter = map().find(pMaterial->type());
        if (iter != map().end()) {
            return iter->second;
        }
        auto pResult = std::shared_ptr<MaterialShader>(
                pMaterial->createShader());
        map().insert(std::pair<MaterialType*,
                std::shared_ptr<MaterialShader>>{
                pMaterial->type(), pResult});
        return pResult;
    }
#endif
    static void purge() {
        std::erase_if(map(), [](const auto& item) {
            auto const& [key, value] = item;
            return value.use_count() == 1;
        });
    }
};

} // namespace rendergraph
