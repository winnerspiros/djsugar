#pragma once

#include "backend/basenode.h"

#ifndef MIXXX_USE_LLGL
#include <QOpenGLFunctions>
#endif

namespace rendergraph {
class BaseGeometryNode;
class GeometryNode;
class Geometry;
class Material;
} // namespace rendergraph

class rendergraph::BaseGeometryNode : public rendergraph::BaseNode
#ifndef MIXXX_USE_LLGL
        , public QOpenGLFunctions
#endif
{
  public:
    BaseGeometryNode() = default;
    virtual ~BaseGeometryNode() = default;

    void initialize() override;
    void render() override;
    void resize(int w, int h) override;

  protected:
#ifdef MIXXX_USE_LLGL
    void renderLLGL(GeometryNode* pThis, Geometry& geometry, Material& material);
    void renderGL(GeometryNode* pThis, Geometry& geometry, Material& material);
#endif
};
