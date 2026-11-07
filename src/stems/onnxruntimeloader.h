#pragma once

#include <QString>

// The ONNX Runtime library is loaded at runtime, only when a track is
// separated (see initOnnxRuntime()), so the C++ API is initialized manually
#define ORT_API_MANUAL_INIT
#include <onnxruntime_cxx_api.h>
#undef ORT_API_MANUAL_INIT

namespace mixxx {
namespace stems {

/// Loads the ONNX Runtime library and initializes its C++ API. Returns an
/// error message, empty on success. Thread-safe, loads the library once.
QString initOnnxRuntime();

/// The process-wide ONNX Runtime environment. Only valid after
/// initOnnxRuntime() succeeded.
Ort::Env& onnxRuntimeEnv();

} // namespace stems
} // namespace mixxx
