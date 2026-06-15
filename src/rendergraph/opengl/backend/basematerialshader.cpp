#include "basematerialshader.h"

using namespace rendergraph;

int LLGLBaseMaterialShader::attributeLocation(int attributeIndex) const {
    return m_attributeLocations[attributeIndex];
}

int LLGLBaseMaterialShader::uniformLocation(int uniformIndex) const {
    return m_uniformLocations[uniformIndex];
}

BaseMaterial* LLGLBaseMaterialShader::lastModifiedByMaterial() const {
    return m_pLastModifiedByMaterial;
}

void LLGLBaseMaterialShader::setLastModifiedByMaterial(BaseMaterial* pMaterial) {
    m_pLastModifiedByMaterial = pMaterial;
}