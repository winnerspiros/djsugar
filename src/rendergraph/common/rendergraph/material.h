#pragma once

#include <memory>

#include "backend/basematerial.h"
#include "rendergraph/assert.h"
#include "rendergraph/materialshader.h"
#include "rendergraph/materialtype.h"
#include "rendergraph/texture.h"
#include "rendergraph/uniformscache.h"
#include "rendergraph/uniformset.h"

#ifdef MIXXX_USE_LLGL
#include "rendergraph/llgl/backend/shadercache.h"
#else
#include <QOpenGLShaderProgram>
#endif

namespace rendergraph {
class Material;
} // namespace rendergraph

class rendergraph::Material : public rendergraph::BaseMaterial {
  public:
    Material(const UniformSet& uniformSet);
    virtual ~Material();

    /// See QSGMaterial::compare.
    int compare(const Material* pOther) const {
        DEBUG_ASSERT(type() == pOther->type());
        int cacheCompareResult = std::memcmp(m_uniformsCache.data(),
                pOther->m_uniformsCache.data(),
                m_uniformsCache.size());
        if (cacheCompareResult != 0) {
            return cacheCompareResult < 0 ? -1 : 1;
        }
        if (!texture(0) || !pOther->texture(0)) {
            return texture(0) ? 1 : -1;
        }
        const qint64 diff = texture(0)->comparisonKey() - pOther->texture(0)->comparisonKey();
        return diff < 0 ? -1 : (diff > 0 ? 1 : 0);
    }

    virtual std::unique_ptr<MaterialShader> createShader() const = 0;

#ifdef MIXXX_USE_LLGL
    virtual std::unique_ptr<LLGLMaterialShader> createLLGLShader() const = 0;
#endif

    template<typename T>
    void setUniform(int uniformIndex, const T& value) {
        m_uniformsCache.set(uniformIndex, value);
        m_uniformsCacheDirty = true;
    }

    const UniformsCache& uniformsCache() const {
        return m_uniformsCache;
    }

    bool clearUniformsCacheDirty() {
        if (m_uniformsCacheDirty) {
            m_uniformsCacheDirty = false;
            return true;
        }
        return false;
    }

    virtual Texture* texture(int) const {
        return nullptr;
    }

    // Shader access — returns the appropriate type for the backend
#ifdef MIXXX_USE_LLGL
    LLGLMaterialShader& shader() {
        return *m_pLLGLShader;
    }
    const LLGLMaterialShader& shader() const {
        return *m_pLLGLShader;
    }
    void setShader(std::shared_ptr<LLGLMaterialShader> pShader) {
        m_pLLGLShader = pShader;
    }
#else
#ifndef MIXXX_USE_LLGL
#include <QOpenGLShaderProgram>
#endif
    QOpenGLShaderProgram& shader() {
        return *m_pShader;
    }
    const QOpenGLShaderProgram& shader() const {
        return *m_pShader;
    }
    void setShader(std::shared_ptr<MaterialShader> pShader) {
        m_pShader = pShader;
    }
#endif

    // For BaseGeometryNode::render() which needs to check shader type
#ifdef MIXXX_USE_LLGL
    bool isLLGL() const { return true; }
#else
    bool isLLGL() const { return false; }
#endif

  private:
    UniformsCache m_uniformsCache;
    bool m_uniformsCacheDirty{};

#ifdef MIXXX_USE_LLGL
    std::shared_ptr<LLGLMaterialShader> m_pLLGLShader;
#else
    std::shared_ptr<MaterialShader> m_pShader;
#endif
};
