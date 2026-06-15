#include "shadersourceprovider.h"

#include <QDebug>

namespace rendergraph {

ShaderSourceProvider& ShaderSourceProvider::instance() {
    static ShaderSourceProvider provider;
    return provider;
}

ShaderBackend ShaderSourceProvider::detectBackend(const char* renderSystemName) {
    if (renderSystemName == nullptr) {
        return ShaderBackend::GLSL;
    }
    if (strcmp(renderSystemName, "Metal") == 0) {
        return ShaderBackend::MetalSL;
    }
    if (strncmp(renderSystemName, "Direct3D", 7) == 0) {
        return ShaderBackend::HLSL;
    }
    /* OpenGL, Vulkan, OpenGLES */
    return ShaderBackend::GLSL;
}

const char* ShaderSourceProvider::profileString(ShaderBackend backend, bool isVertex) const {
    switch (backend) {
    case ShaderBackend::GLSL:
        return "440";
    case ShaderBackend::HLSL:
        return isVertex ? "vs_4_0" : "ps_4_0";
    case ShaderBackend::MetalSL:
        return "metal1.0";
    }
    return "440";
}

QString ShaderSourceProvider::vertexShader(ShaderBackend backend) const {
    switch (backend) {
    case ShaderBackend::GLSL:
        return QStringLiteral(R"(
#version 440 core
layout(location = 0) in vec2 aPosition;
layout(location = 1) in vec3 aColor;
layout(std140, binding = 0) uniform Uniforms {
    mat4 uMatrix;
};
out vec3 vColor;
void main() {
    gl_Position = uMatrix * vec4(aPosition, 0.0, 1.0);
    vColor = aColor;
}
)");

    case ShaderBackend::HLSL:
        return QStringLiteral(R"(
cbuffer Uniforms : register(b0) {
    float4x4 uMatrix;
};
struct VSIn {
    float2 position : POSITION;
    float3 color : COLOR;
};
struct VSOut {
    float4 position : SV_POSITION;
    float3 color : COLOR;
};
VSOut main(VSIn input) {
    VSOut output;
    output.position = mul(uMatrix, float4(input.position, 0.0, 1.0));
    output.color = input.color;
    return output;
}
)");

    case ShaderBackend::MetalSL:
        return QStringLiteral(R"(
#include <metal_stdlib>
using namespace metal;
struct Uniforms {
    float4x4 uMatrix;
};
struct VSOut {
    float4 position [[position]];
    float3 color;
};
vertex VSOut vertex_main(
    constant Uniforms& uniforms [[buffer(0)]],
    uint vid [[vertex_id]],
    constant float2* positions [[buffer(1)]],
    constant float3* colors [[buffer(2)]]
) {
    VSOut output;
    output.position = uniforms.uMatrix * float4(positions[vid], 0.0, 1.0);
    output.color = colors[vid];
    return output;
}
)");
    }
    return QString();
}

QString ShaderSourceProvider::fragmentShader(ShaderBackend backend) const {
    switch (backend) {
    case ShaderBackend::GLSL:
        return QStringLiteral(R"(
#version 440 core
in vec3 vColor;
out vec4 fragColor;
void main() {
    fragColor = vec4(vColor, 1.0);
}
)");

    case ShaderBackend::HLSL:
        return QStringLiteral(R"(
struct PSIn {
    float4 position : SV_POSITION;
    float3 color : COLOR;
};
float4 main(PSIn input) : SV_TARGET {
    return float4(input.color, 1.0);
}
)");

    case ShaderBackend::MetalSL:
        return QStringLiteral(R"(
#include <metal_stdlib>
using namespace metal;
struct FSIn {
    float4 position [[position]];
    float3 color;
};
fragment float4 fragment_main(FSIn input [[stage_in]]) {
    return float4(input.color, 1.0);
}
)");
    }
    return QString();
}

} // namespace rendergraph
