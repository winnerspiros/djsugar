#include "backend/basematerial.h"

#include "rendergraph/material.h"

using namespace rendergraph;

int BaseMaterial::compare(const BaseMaterial* other) const {
    auto pThis = static_cast<const Material*>(this);
    return pThis->compare(static_cast<const Material*>(other));
}
