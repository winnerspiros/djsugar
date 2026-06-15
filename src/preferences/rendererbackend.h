#pragma once

#include <QString>
#include <QList>

namespace mixxx {

/// Available rendering backends per platform.
/// Each backend maps to an LLGL render system.
enum class RendererBackend {
    Auto,           // Platform default
    OpenGL,         // Linux, Windows, macOS, Android
    Vulkan,         // Linux, Windows, Android (if supported)
    Direct3D11,     // Windows only
    Direct3D12,     // Windows only
    Metal,          // macOS, iOS only
};

/// Returns the list of backends available on the current platform.
QList<RendererBackend> availableRenderers();

/// Returns the default renderer for the current platform.
RendererBackend defaultRenderer();

/// Convert backend enum to display string.
QString rendererBackendToString(RendererBackend backend);

/// Convert string to backend enum.
RendererBackend rendererBackendFromString(const QString& name);

/// Returns the LLGL module name for a given backend.
/// Used to load the correct LLGL render system.
const char* rendererBackendToModuleName(RendererBackend backend);

/// Returns true if the given backend uses GLSL shaders.
/// When false, native shader language is used.
bool rendererBackendUsesGLSL(RendererBackend backend);

} // namespace mixxx
