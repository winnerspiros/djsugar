#include "util/androidcrashreport.h"

#include <dlfcn.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/prctl.h>
#include <time.h>
#include <ucontext.h>
#include <unistd.h>

#include <QDir>
#include <QFile>
#include <cstdint>
#include <cstring>

#include "util/versionstore.h"

namespace {

constexpr int kSignals[] = {SIGSEGV, SIGBUS, SIGFPE, SIGILL, SIGABRT, SIGTRAP};
constexpr int kSignalCount = sizeof(kSignals) / sizeof(kSignals[0]);
constexpr int kMaxFrames = 64;

struct sigaction s_previousActions[kSignalCount];
// Prepared up front: only async-signal-safe calls are made while crashing
char s_directory[512];
char s_build[128];

void restorePreviousActions() {
    for (int i = 0; i < kSignalCount; ++i) {
        sigaction(kSignals[i], &s_previousActions[i], nullptr);
    }
}

void writeString(int fd, const char* pText) {
    [[maybe_unused]] const auto written = write(fd, pText, strlen(pText));
}

void writeNumber(int fd, uint64_t value, int base) {
    char buffer[24];
    int pos = sizeof(buffer);
    buffer[--pos] = '\0';
    do {
        buffer[--pos] = "0123456789abcdef"[value % base];
        value /= base;
    } while (value != 0 && pos > 0);
    writeString(fd, buffer + pos);
}

/// Removes the pointer authentication code from a return address
uintptr_t stripPointerAuthentication(uintptr_t address) {
#if defined(__aarch64__)
    // XPACLRI, a no-op on CPUs without pointer authentication
    register uintptr_t x30 asm("x30") = address;
    asm("hint #7" : "+r"(x30));
    return x30;
#else
    return address;
#endif
}

void writeAddress(int fd, int index, uintptr_t address) {
    address = stripPointerAuthentication(address);
    writeString(fd, "#");
    writeNumber(fd, index, 10);
    writeString(fd, " pc 0x");
    Dl_info info;
    if (dladdr(reinterpret_cast<void*>(address), &info) != 0 && info.dli_fname) {
        writeNumber(fd, address - reinterpret_cast<uintptr_t>(info.dli_fbase), 16);
        writeString(fd, " ");
        const char* pName = strrchr(info.dli_fname, '/');
        writeString(fd, pName ? pName + 1 : info.dli_fname);
        if (info.dli_sname) {
            writeString(fd, " (");
            writeString(fd, info.dli_sname);
            writeString(fd, ")");
        }
    } else {
        writeNumber(fd, address, 16);
    }
    writeString(fd, "\n");
}

void onCrash(int signalNumber, siginfo_t* pInfo, void* pContext) {
    // A fault while writing the report goes to the system's handler
    restorePreviousActions();

    char path[sizeof(s_directory) + 48];
    strcpy(path, s_directory);
    char* pEnd = path + strlen(path);
    strcpy(pEnd, "/crash-");
    pEnd += strlen(pEnd);
    // The time as a number, without the non-reentrant time formatting
    uint64_t seconds = static_cast<uint64_t>(time(nullptr));
    char digits[24];
    int count = 0;
    do {
        digits[count++] = static_cast<char>('0' + seconds % 10);
        seconds /= 10;
    } while (seconds != 0);
    while (count > 0) {
        *pEnd++ = digits[--count];
    }
    strcpy(pEnd, ".txt");

    const int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd >= 0) {
        writeString(fd, "Mixxx crashed\nsignal ");
        writeNumber(fd, signalNumber, 10);
        writeString(fd, " code ");
        writeNumber(fd, static_cast<uint64_t>(pInfo->si_code), 10);
        writeString(fd, " fault address 0x");
        writeNumber(fd, reinterpret_cast<uintptr_t>(pInfo->si_addr), 16);
        char threadName[17] = {};
        prctl(PR_GET_NAME, threadName, 0, 0, 0);
        writeString(fd, "\nthread ");
        writeString(fd, threadName);
        writeString(fd, "\nbuild ");
        writeString(fd, s_build);
        writeString(fd, "\n\n");

#if defined(__aarch64__)
        const auto* pUserContext = static_cast<const ucontext_t*>(pContext);
        const mcontext_t& machine = pUserContext->uc_mcontext;
        writeAddress(fd, 0, machine.pc);
        writeAddress(fd, 1, machine.regs[30]);
        // The frame records: {previous frame pointer, return address}
        uintptr_t framePointer = machine.regs[29];
        for (int i = 2; i < kMaxFrames && framePointer != 0 && (framePointer & 7) == 0; ++i) {
            const auto* pRecord = reinterpret_cast<const uintptr_t*>(framePointer);
            const uintptr_t returnAddress = pRecord[1];
            if (returnAddress == 0) {
                break;
            }
            writeAddress(fd, i, returnAddress);
            const uintptr_t previous = pRecord[0];
            // Frames lie further up the same stack
            if (previous <= framePointer || previous - framePointer > 1024 * 1024) {
                break;
            }
            framePointer = previous;
        }
#else
        (void)pContext;
#endif
        close(fd);
    }

    if (signalNumber == SIGABRT) {
        // Raised explicitly; returning would continue after the raise()
        raise(signalNumber);
    }
    // Returning executes the faulting instruction again, which now goes to
    // the system's handler
}

} // namespace

namespace mixxx {
namespace android {

QString crashReportDirectory() {
    return QStringLiteral("/storage/emulated/0/Mixxx/logs");
}

void installCrashReporter() {
    const QString directory = crashReportDirectory();
    QDir().mkpath(directory);
    const QByteArray path = QFile::encodeName(directory);
    if (path.size() >= static_cast<int>(sizeof(s_directory))) {
        return;
    }
    strcpy(s_directory, path.constData());
    const QByteArray build = VersionStore::gitDescribe().toUtf8().left(sizeof(s_build) - 1);
    strcpy(s_build, build.constData());

    struct sigaction action = {};
    action.sa_sigaction = onCrash;
    action.sa_flags = SA_SIGINFO | SA_ONSTACK;
    sigemptyset(&action.sa_mask);
    for (int i = 0; i < kSignalCount; ++i) {
        sigaction(kSignals[i], &action, &s_previousActions[i]);
    }
}

void exportPreviousLog(const QString& logDirPath) {
    const QString previousLog = QDir(logDirPath).filePath(QStringLiteral("mixxx.log.1"));
    if (!QFile::exists(previousLog)) {
        return;
    }
    const QString target = crashReportDirectory() + QStringLiteral("/mixxx-previous.log");
    QDir().mkpath(crashReportDirectory());
    QFile::remove(target);
    QFile::copy(previousLog, target);
}

} // namespace android
} // namespace mixxx
