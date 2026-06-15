#include "rendererbackend.h"

#include <QCoreApplication>

#ifdef Q_OS_WIN32
#include <windows.h>
#endif

namespace mixxx {

QList<RendererBackend> availableRenderers() {
    QList<RendererBackend> backends;
    backends.append(RendererBackend::Auto);

#if defined(Q_OS_WIN32)
    // Windows: DX12, DX11, Vulkan, OpenGL
    backends.append(RendererBackend::Direct3D12);
    backends.append(RendererBackend::Direct3D11);
    backends.append(RendererBackend::Vulkan);
    backends.append(RendererBackend::OpenGL);
#elif defined(Q_OS_MACOS) || defined(Q_OS_IOS)
    // macOS/iOS: Metal only (OpenGL deprecated)
    backends.append(RendererBackend::Metal);
    backends.append(RendererBackend::OpenGL);
#elif defined(Q_OS_LINUX)
    // Linux: Vulkan only
    backends.append(RendererBackend::Vulkan);
    backends.append(RendererBackend::OpenGL);
#elif defined(Q_OS_ANDROID)
    // Android: Vulkan, OpenGL ES
    backends.append(RendererBackend::Vulkan);
    backends.append(RendererBackend::OpenGL);
#else
    backends.append(RendererBackend::OpenGL);
#endif

    return backends;
}

RendererBackend defaultRenderer() {
#if defined(Q_OS_MACOS) || defined(Q_OS_IOS)
    return RendererBackend::Metal;
#elif defined(Q_OS_WIN32)
    return RendererBackend::Direct3D11;
#elif defined(Q_OS_LINUX)
    return RendererBackend::Vulkan;
#elif defined(Q_OS_ANDROID)
    return RendererBackend::Vulkan;
#else
    return RendererBackend::OpenGL;
#endif
}

QString rendererBackendToString(RendererBackend backend) {
    switch (backend) {
    case RendererBackend::Auto:
        return QStringLiteral("Auto");
    case RendererBackend::OpenGL:
        return QStringLiteral("OpenGL");
    case RendererBackend::Vulkan:
        return QStringLiteral("Vulkan");
    case RendererBackend::Direct3D11:
        return QStringLiteral("Direct3D 11");
    case RendererBackend::Direct3D12:
        return QStringLiteral("Direct3D 12");
    case RendererBackend::Metal:
        return QStringLiteral("Metal");
    }
    return QStringLiteral("Auto");
}

RendererBackend rendererBackendFromString(const QString& name) {
    if (name == QStringLiteral("OpenGL")) return RendererBackend::OpenGL;
    if (name == QStringLiteral("Vulkan")) return RendererBackend::Vulkan;
    if (name == QStringLiteral("Direct3D 11")) return RendererBackend::Direct3D11;
    if (name == QStringLiteral("Direct3D 12")) return RendererBackend::Direct3D12;
    if (name == QStringLiteral("Metal")) return RendererBackend::Metal;
    return RendererBackend::Auto;
}

const char* rendererBackendToModuleName(RendererBackend backend) {
    switch (backend) {
    case RendererBackend::Auto:
        return rendererBackendToModuleName(defaultRenderer());
    case RendererBackend::OpenGL:
        return "OpenGL";
    case RendererBackend::Vulkan:
        return "Vulkan";
    case RendererBackend::Direct3D11:
        return "Direct3D11";
    case RendererBackend::Direct3D12:
        return "Direct3D12";
    case RendererBackend::Metal:
        return "Metal";
    }
    return "OpenGL";
}

bool rendererBackendUsesGLSL(RendererBackend backend) {
    Q_UNUSED(backend);
    // LLGL accepts GLSL for all backends and compiles internally
    return true;
}

} // namespace mixxx
