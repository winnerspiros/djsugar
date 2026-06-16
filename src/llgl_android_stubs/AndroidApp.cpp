// Minimal Android app stub for LLGL
// Replaces the original that uses deprecated ALooper_pollAll
#include <LLGL/Platform/Android/AndroidApp.h>
#include <cstring>

namespace LLGL
{

struct AndroidApp::Impl
{
    bool isWindowReady = true;
    bool isContentReady = true;
};

AndroidApp::AndroidApp() : impl_(new Impl) {}
AndroidApp::~AndroidApp() { delete impl_; }

bool AndroidApp::IsReady() const { return impl_->isWindowReady; }

} // namespace LLGL
