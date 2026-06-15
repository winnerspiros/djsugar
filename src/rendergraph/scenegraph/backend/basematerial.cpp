#include "rendergraph/material.h"
#include "rendergraph/materialshader.h"

using namespace rendergraph;

bool BaseMaterial::updateUniformsByteArray(QByteArray* buf) {
    auto pThis = static_cast<Material*>(this);
    if (pThis->clearUniformsCacheDirty()) {
        memcpy(buf->data(), pThis->uniformsCache().data(), pThis->uniformsCache().size());
        return true;
    }
    return false;
}

int BaseMaterial::compare(const QSGMaterial* other) const {
    auto pThis = static_cast<const Material*>(this);
    return pThis->compare(static_cast<const Material*>(other));
}

QSGMaterialShader* BaseMaterial::createShader(QSGRendererInterface::RenderMode) const {
    // Note: this looks like a leak but it isn't: we pass ownership to Qt.
    // Qt will cache and reuse the shader for all Material of the same type.
    // TODO make sure that RenderMode is always the same.
    auto pThis = static_cast<const Material*>(this);
    // MaterialShader is the common base. For the scenegraph target,
    // the concrete shader type inherits from both MaterialShader and QSGMaterialShader.
    auto pShader = pThis->createShader().release();
    return static_cast<QSGMaterialShader*>(pShader);
}
