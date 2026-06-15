#include "backend/baseopenglnode.h"

#include "rendergraph/openglnode.h"

using namespace rendergraph;

#ifndef MIXXX_USE_LLGL
void BaseOpenGLNode::initialize() {
    initializeOpenGLFunctions();
    initializeGL();
}

void BaseOpenGLNode::render() {
    paintGL();
}

void BaseOpenGLNode::resize(int w, int h) {
    resizeGL(w, h);
}
#endif // !MIXXX_USE_LLGL
