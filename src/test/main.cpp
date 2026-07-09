#include <gtest/gtest.h>

#include "util/denormalsarezero.h"

// On aarch64 (Linux & Windows ARM64), flush denormals to zero to prevent
// STATUS_FLOAT_MULTIPLE_FAULTS on Windows ARM64, where denormal values
// raise an FPU exception. On x86/x64 this is done via SSE MXCSR.
// On Linux ARM64 the FTZ bit is usually already set, but we ensure it.
//
// Note: _MM_SET_DENORMALS_ZERO_MODE from denormalsarezero.h is a no-op
// on non-x86 (the #else branch). On aarch64 we set the FPCR directly.
#if defined(__aarch64__)
#if defined(_MSC_VER)
#include <intrin.h>
static void enableFlushToZero() {
    __int64 fpcr = _ReadStatusReg(ARM64_FPCR);
    _WriteStatusReg(ARM64_FPCR, fpcr | (static_cast<__int64>(1) << 24));
}
#else
static void enableFlushToZero() {
    int64_t savedFPCR;
    asm volatile("mrs %[savedFPCR], FPCR" : [savedFPCR] "=r"(savedFPCR));
    asm volatile("msr FPCR, %[src]" : : [src] "r"(savedFPCR | (static_cast<int64_t>(1) << 24)));
}
#endif
#else
static void enableFlushToZero() {
    _MM_SET_DENORMALS_ZERO_MODE(_MM_DENORMALS_ZERO_ON);
}
#endif

int main(int argc, char** argv) {
    enableFlushToZero();
    testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}