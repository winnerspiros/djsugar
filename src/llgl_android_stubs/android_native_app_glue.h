// Stub for android_native_app_glue.h
// The real header is part of the Android NDK's native_app_glue library.
// This stub provides the minimal definitions needed to compile LLGL.
#ifndef ANDROID_NATIVE_APP_GLUE_H
#define ANDROID_NATIVE_APP_GLUE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque types from Android NDK
typedef struct ANativeActivity ANativeActivity;
typedef struct AInputQueue AInputQueue;
typedef struct ALooper ALooper;
typedef struct AConfiguration AConfiguration;
typedef struct AAssetManager AAssetManager;
typedef struct AConfiguration AConfiguration;

// Minimal android_app struct stub
struct android_app {
    void* userData;
    void (*onAppCmd)(struct android_app* app, int32_t cmd);
    int32_t (*onInputEvent)(struct android_app* app, void* event);
    ANativeActivity* activity;
    AInputQueue* inputQueue;
    AConfiguration* config;
    ALooper* looper;
    void* savedState;
    size_t savedStateSize;
};

struct android_poll_source {
    int32_t id;
    struct android_app* app;
    void (*process)(struct android_app* app, struct android_poll_source* source);
};

// Minimal function stubs
int ALooper_pollAll(int timeoutMillis, int* outFd, int* outEvents, void** outData);
int ALooper_pollOnce(int timeoutMillis, int* outFd, int* outEvents, void** outData);
void android_app_cmd(struct android_app* app, int32_t cmd);

#ifdef __cplusplus
}
#endif

#endif // ANDROID_NATIVE_APP_GLUE_H
