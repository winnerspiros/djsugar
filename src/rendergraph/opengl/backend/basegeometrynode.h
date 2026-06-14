#pragma once

#include "backend/basenode.h"

namespace rendergraph {
class BaseGeometryNode;
class GeometryNode;
class Geometry;
class Material;
} // namespace rendergraph

class rendergraph::BaseGeometryNode : public rendergraph::BaseNode {
  public:
    BaseGeometryNode() = default;
    virtual ~BaseGeometryNode() = default;

    void initialize() override;
    void render() override;
    void resize(int w, int h) override;

  protected:
    void renderLLGL(GeometryNode* pThis, Geometry& geometry, Material& material);
    void renderGL(GeometryNode* pThis, Geometry& geometry, Material& material);
};
