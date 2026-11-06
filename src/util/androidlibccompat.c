// Android 10 to 13 (API 29-33): functions that Qt and Mixxx import from the
// C library of a newer Android version, built against API 35:
// copy_file_range (Android 14), backtrace (Android 13) and
// pthread_cond_clockwait (Android 11). Without them the libraries can't be
// loaded and the app closes at once.
//
// libQt6Core and libmixxx are patched at build time (see
// cmake/modules/MixxxAndroid.cmake) to load this library and to import
// these functions without the version of the C library they were built
// against. Where the C library has them, they are used; otherwise
// copy_file_range reports ENOSYS, on which Qt and libc++ copy the file the
// classic way, backtrace walks the stack with the unwinder of the NDK, and
// pthread_cond_clockwait waits with the functions of the clock it is given.

#include <dlfcn.h>
#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <time.h>
#include <sys/types.h>
#include <unwind.h>

// Mixxx builds with hidden symbols by default
#define COMPAT_EXPORT __attribute__((visibility("default")))

typedef ssize_t (*CopyFileRangeFunction)(int, off64_t*, int, off64_t*, size_t, unsigned int);
typedef int (*BacktraceFunction)(void**, int);
typedef int (*CondClockwaitFunction)(
        pthread_cond_t*, pthread_mutex_t*, clockid_t, const struct timespec*);

static CopyFileRangeFunction s_copyFileRange;
static BacktraceFunction s_backtrace;
static CondClockwaitFunction s_condClockwait;

// Before the libraries that need these functions run any code: they load
// this library first
__attribute__((constructor)) static void resolveLibcFunctions(void) {
    void* libc = dlopen("libc.so", RTLD_NOW | RTLD_NOLOAD);
    if (!libc) {
        return;
    }
    s_copyFileRange = (CopyFileRangeFunction)dlsym(libc, "copy_file_range");
    s_backtrace = (BacktraceFunction)dlsym(libc, "backtrace");
    s_condClockwait = (CondClockwaitFunction)dlsym(libc, "pthread_cond_clockwait");
    dlclose(libc);
}

COMPAT_EXPORT ssize_t copy_file_range(int fdIn,
        off64_t* pOffsetIn,
        int fdOut,
        off64_t* pOffsetOut,
        size_t length,
        unsigned int flags) {
    if (s_copyFileRange) {
        return s_copyFileRange(fdIn, pOffsetIn, fdOut, pOffsetOut, length, flags);
    }
    // Not calling the system call directly: the seccomp filter of apps on
    // these Android versions may not allow it, and would kill the app
    errno = ENOSYS;
    return -1;
}

struct BacktraceState {
    void** pCurrent;
    void** pEnd;
};

static _Unwind_Reason_Code addFrame(struct _Unwind_Context* pContext, void* pArg) {
    struct BacktraceState* pState = (struct BacktraceState*)pArg;
    const uintptr_t pc = _Unwind_GetIP(pContext);
    if (pc) {
        if (pState->pCurrent == pState->pEnd) {
            return _URC_END_OF_STACK;
        }
        *pState->pCurrent++ = (void*)pc;
    }
    return _URC_NO_REASON;
}

COMPAT_EXPORT int backtrace(void** pBuffer, int size) {
    if (s_backtrace) {
        return s_backtrace(pBuffer, size);
    }
    if (size <= 0) {
        return 0;
    }
    struct BacktraceState state = {pBuffer, pBuffer + size};
    _Unwind_Backtrace(addFrame, &state);
    return (int)(state.pCurrent - pBuffer);
}

COMPAT_EXPORT int pthread_cond_clockwait(pthread_cond_t* pCond,
        pthread_mutex_t* pMutex,
        clockid_t clock,
        const struct timespec* pTimeout) {
    if (s_condClockwait) {
        return s_condClockwait(pCond, pMutex, clock, pTimeout);
    }
    // Both functions wait until the absolute time pTimeout of their clock,
    // whatever clock the condition variable was created with
    switch (clock) {
    case CLOCK_MONOTONIC:
        return pthread_cond_timedwait_monotonic_np(pCond, pMutex, pTimeout);
    case CLOCK_REALTIME:
        return pthread_cond_timedwait(pCond, pMutex, pTimeout);
    default:
        return EINVAL;
    }
}
