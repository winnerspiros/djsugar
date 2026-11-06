#version 440

// The fragments of waveformstem.vert: like rgba.frag, built for the same GLSL
// versions as the vertex shader (they must match to be linked)

layout(location = 0) in highp vec4 vColor;
layout(location = 0) out highp vec4 fragColor;

void main() {
    fragColor = vec4(vColor.xyz * vColor.w, vColor.w); // premultiplied alpha
}
