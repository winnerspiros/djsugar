#include "stems/onnxruntimeloader.h"

#include <dlfcn.h>

#include <memory>
#include <mutex>

namespace mixxx {
namespace stems {

namespace {

std::once_flag s_initFlag;
QString s_initError;
std::unique_ptr<Ort::Env> s_pEnv;

void loadOnnxRuntime() {
    // Packaged in the APK (see cmake/modules/MixxxAndroid.cmake)
    void* pLibrary = dlopen("libonnxruntime.so", RTLD_NOW | RTLD_LOCAL);
    if (!pLibrary) {
        s_initError = QStringLiteral("Unable to load ONNX Runtime: ") +
                QString::fromUtf8(dlerror());
        return;
    }
    using GetApiBaseFn = const OrtApiBase* (*)();
    auto getApiBase = reinterpret_cast<GetApiBaseFn>(dlsym(pLibrary, "OrtGetApiBase"));
    const OrtApi* pApi = getApiBase ? getApiBase()->GetApi(ORT_API_VERSION) : nullptr;
    if (!pApi) {
        s_initError = QStringLiteral("Unsupported ONNX Runtime library");
        return;
    }
    Ort::InitApi(pApi);
    try {
        s_pEnv = std::make_unique<Ort::Env>(ORT_LOGGING_LEVEL_WARNING, "mixxx-stems");
    } catch (const Ort::Exception& e) {
        s_initError = QString::fromUtf8(e.what());
    }
}

} // namespace

QString initOnnxRuntime() {
    std::call_once(s_initFlag, loadOnnxRuntime);
    return s_initError;
}

Ort::Env& onnxRuntimeEnv() {
    return *s_pEnv;
}

} // namespace stems
} // namespace mixxx
