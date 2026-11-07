#include "stems/stemseparator.h"

#include <sched.h>
#include <sys/resource.h>
#include <unistd.h>

#include <QFile>
#include <thread>
#include <vector>

#include "stems/htdemucsseparator.h"
#include "stems/spleeterseparator.h"

namespace mixxx {
namespace stems {

namespace {

// Nice value of the model threads: below the UI and far below the audio
constexpr int kModelThreadNiceness = 10;

struct ModelThread {
    std::thread thread;
};

OrtCustomThreadHandle createModelThread(
        void*, OrtThreadWorkerFn workerFn, void* pWorkerParam) {
    auto* pModelThread = new ModelThread;
    pModelThread->thread = std::thread([workerFn, pWorkerParam]() {
        setpriority(PRIO_PROCESS, static_cast<id_t>(gettid()), kModelThreadNiceness);
        workerFn(pWorkerParam);
    });
    return reinterpret_cast<OrtCustomThreadHandle>(pModelThread);
}

void joinModelThread(OrtCustomThreadHandle handle) {
    auto* pModelThread = reinterpret_cast<ModelThread*>(const_cast<OrtCustomHandleType*>(handle));
    pModelThread->thread.join();
    delete pModelThread;
}

int readMemInfoMb(const QByteArray& key) {
    QFile file(QStringLiteral("/proc/meminfo"));
    if (!file.open(QIODevice::ReadOnly)) {
        return -1;
    }
    for (const QByteArray& line : file.readAll().split('\n')) {
        if (line.startsWith(key)) {
            const QList<QByteArray> parts = line.simplified().split(' ');
            if (parts.size() >= 2) {
                return static_cast<int>(parts[1].toLongLong() / 1024);
            }
        }
    }
    return -1;
}

/// The cores of a big.LITTLE CPU with less than the highest maximum
/// frequency, or none if all are alike
std::vector<int> efficiencyCores() {
    std::vector<std::pair<int, qint64>> maxFrequencies;
    const int coreCount = static_cast<int>(std::thread::hardware_concurrency());
    for (int core = 0; core < coreCount; ++core) {
        QFile file(QStringLiteral("/sys/devices/system/cpu/cpu%1/cpufreq/cpuinfo_max_freq")
                        .arg(core));
        if (!file.open(QIODevice::ReadOnly)) {
            return {};
        }
        maxFrequencies.emplace_back(core, file.readAll().trimmed().toLongLong());
    }
    qint64 highest = 0;
    for (const auto& [core, frequency] : maxFrequencies) {
        highest = std::max(highest, frequency);
    }
    std::vector<int> cores;
    for (const auto& [core, frequency] : maxFrequencies) {
        if (frequency > 0 && frequency < highest) {
            cores.push_back(core);
        }
    }
    return cores;
}

} // namespace

void moveToEfficiencyCores() {
    static const std::vector<int> s_cores = efficiencyCores();
    if (s_cores.empty()) {
        return;
    }
    cpu_set_t set;
    CPU_ZERO(&set);
    for (int core : s_cores) {
        CPU_SET(core, &set);
    }
    sched_setaffinity(0, sizeof(set), &set);
}

// static
std::unique_ptr<StemSeparator> StemSeparator::create(Model model) {
    switch (model) {
    case Model::Spleeter:
        return std::make_unique<SpleeterSeparator>();
    case Model::HTDemucs:
        return std::make_unique<HTDemucsSeparator>();
    }
    return nullptr;
}

// static
Ort::SessionOptions StemSeparator::sessionOptions(const Options& options) {
    Ort::SessionOptions sessionOptions;
    sessionOptions.SetIntraOpNumThreads(options.threads);
    sessionOptions.SetInterOpNumThreads(1);
    sessionOptions.SetExecutionMode(ExecutionMode::ORT_SEQUENTIAL);
    // The arena keeps the peak memory of the largest run and made the runs
    // stall on Android
    sessionOptions.DisableCpuMemArena();
    sessionOptions.DisableMemPattern();
    sessionOptions.AddConfigEntry("session.use_device_allocator_for_initializers", "1");
    // Waiting threads sleep instead of spinning, leaving the CPU to the audio
    sessionOptions.AddConfigEntry("session.intra_op.allow_spinning", "0");
    sessionOptions.SetCustomCreateThreadFn(createModelThread);
    sessionOptions.SetCustomJoinThreadFn(joinModelThread);
    return sessionOptions;
}

int availableMemoryMb() {
    return readMemInfoMb(QByteArrayLiteral("MemAvailable:"));
}

int totalMemoryMb() {
    return readMemInfoMb(QByteArrayLiteral("MemTotal:"));
}

} // namespace stems
} // namespace mixxx
