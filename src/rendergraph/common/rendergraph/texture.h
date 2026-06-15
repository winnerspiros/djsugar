#pragma once

#include <QImage>
#include <memory>

#include "backend/basetexture.h"
#include "rendergraph/context.h"

namespace rendergraph {
class Texture;
} // namespace rendergraph

class rendergraph::Texture {
  public:
    Texture(Context* pContext, const QImage& image);

    BaseTexture* backendTexture() const {
        return m_pTexture;
    }

    // used by Material::compare
    qint64 comparisonKey() const;

  private:
    BaseTexture* m_pTexture{};
};
