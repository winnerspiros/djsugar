#pragma once

#include <QString>
#include <QHash>
#include <QMutex>

namespace rendergraph {

enum class ShaderBackend {
    GLSL,       // OpenGL, Vulkan (GLSL compiled to SPIR-V by LLGL)
    HLSL,       // Direct3D 11, Direct3D 12
    MetalSL,    // Metal (macOS, iOS)
};

/// Provides shader source code for each backend.
/// All shaders render the same colored-rectangle waveform,
/// just in the native shading language of each backend.
class ShaderSourceProvider {
  public:
    static ShaderSourceProvider& instance();

    /// Get vertex shader source for the given backend.
    QString vertexShader(ShaderBackend backend) const;

    /// Get fragment shader source for the given backend.
    QString fragmentShader(ShaderBackend backend) const;

    /// Get the LLGL profile string for a backend.
    const char* profileString(ShaderBackend backend, bool isVertex) const;

    /// Detect which backend LLGL is using based on the render system name.
    static ShaderBackend detectBackend(const char* renderSystemName);

  private:
    ShaderSourceProvider() = default;
    QHash<ShaderBackend, QString> m_vertexShaders;
    QHash<ShaderBackend, QString> m_fragmentShaders;
};

} // namespace rendergraph
