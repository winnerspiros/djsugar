// Minimal Android app header stub for LLGL
// Replaces the original that includes android_native_app_glue.h
#ifndef LLGL_ANDROID_APP_H
#define LLGL_ANDROID_APP_H

#include <LLGL/Types.h>

namespace LLGL
{

class AndroidApp
{
  public:
    AndroidApp();
    ~AndroidApp();
    bool IsReady() const;
    
  private:
    struct Impl;
    Impl* impl_;
};

} // namespace LLGL

#endif // LLGL_ANDROID_APP_H
