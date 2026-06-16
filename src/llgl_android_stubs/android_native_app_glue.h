// Stub for android_native_app_glue.h
// The real header is part of the Android NDK's native_app_glue library
// which may not be available in all build environments.
// This stub provides the minimal definitions needed to compile.
#ifndef ANDROID_NATIVE_APP_GLUE_H
#define ANDROID_NATIVE_APP_GLUE_H

// Minimal stubs - not functional, just enough to compile
struct android_app {};
struct android_poll_source {
    int32_t id;
    struct android_app* app;
    void (*process)(struct android_app* app, struct android_poll_source* source);
};

typedef void* ALooper;

#endif // ANDROID_NATIVE_APP_GLUE_H
