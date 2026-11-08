#version 440

// The fragments of waveformrgb.vert: like rgb.frag, built for the same GLSL
// versions as the vertex shader (they must match to be linked)

// Colors only: mediump runs at twice the rate on mobile GPUs (Mali,
// Adreno) and is exact enough for 8-bit color.
layout(location = 0) in mediump vec3 vColor;
layout(location = 0) out mediump vec4 fragColor;

void main() {
    fragColor = vec4(vColor, 1.0);
}
