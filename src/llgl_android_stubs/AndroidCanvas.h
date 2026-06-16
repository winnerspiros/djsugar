// Minimal Android canvas header stub for LLGL
#ifndef LLGL_ANDROID_CANVAS_H
#define LLGL_ANDROID_CANVAS_H

#include <LLGL/Types.h>

namespace LLGL {

class AndroidCanvas {
  public:
    AndroidCanvas();
    ~AndroidCanvas();
    bool IsReady() const;
};

} // namespace LLGL

#endif // LLGL_ANDROID_CANVAS_H
