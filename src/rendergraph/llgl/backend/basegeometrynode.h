#pragma once

#ifdef MIXXX_USE_LLGL

#include "backend/basellglnode.h"
#include "rendergraph/geometrynode.h"

namespace rendergraph {

class BaseLLGLGeometryNode : public BaseLLGLNode, public GeometryNode {
  public:
    BaseLLGLGeometryNode() = default;
    ~BaseLLGLGeometryNode() override = default;

    void initialize() override;
    void render() override;
    void resize(int w, int h) override;
};

} // namespace rendergraph

#endif // MIXXX_USE_LLGL
