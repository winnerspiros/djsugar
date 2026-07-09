#ifdef USE_BENCH
#include <benchmark/benchmark.h>
#endif

#include "errordialoghandler.h"
#include "mixxxtest.h"
#include "util/denormalsarezero.h"
#include "util/logging.h"

// On aarch64, flush denormals to zero to prevent
// STATUS_FLOAT_MULTIPLE_FAULTS on Windows ARM64.
// On x86/x64 this is done via SSE MXCSR.
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

int main(int argc, char **argv) {
    enableFlushToZero();
    // By default, render analyzer waveform tests to an offscreen buffer
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
        qputenv("QT_QPA_PLATFORM", QByteArray("offscreen"));
    }

    // We never want to popup error dialogs when running tests.
    ErrorDialogHandler::setEnabled(false);

#ifdef USE_BENCH
    bool run_benchmarks = false;
    for (int i = 0; i < argc; ++i) {
        if (strcmp(argv[i], "--benchmark") == 0) {
            run_benchmarks = true;
            break;
        } else if (strcmp(argv[i], "--trace") == 0) {
            mixxx::Logging::setLogLevel(mixxx::LogLevel::Trace);
        }
    }

    if (run_benchmarks) {
        benchmark::Initialize(&argc, argv);
        MixxxTest::ApplicationScope applicationScope(argc, argv);
        benchmark::RunSpecifiedBenchmarks();
        return 0;
    }

    // Otherwise, run the test suite:
#endif
    testing::InitGoogleTest(&argc, argv);
    MixxxTest::ApplicationScope applicationScope(argc, argv);
    return RUN_ALL_TESTS();
}
