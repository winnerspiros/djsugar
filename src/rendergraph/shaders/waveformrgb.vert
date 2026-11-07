#version 440

// The RGB waveform drawn by the GPU (allshader::WaveformRendererRGBGpu): the
// geometry is a thin rectangle per pixel column, made once, and this shader
// gives each column its height and color from the waveform data in a
// texture, as WaveformRendererRGB does on the CPU. Only these uniforms change
// from frame to frame.

layout(std140, binding = 0) uniform buf {
    highp mat4 matrix;
    highp vec4 lowColor;
    highp vec4 midColor;
    highp vec4 highColor;
    highp vec4 axesColor;
    // The visual frame (left/right pair) at the first pixel column
    highp float firstFrame;
    highp float framesPerPixel;
    highp float frameCount;
    highp float textureWidth;
    highp float textureHeight;
    highp float halfBreadth;
    // allGain * halfBreadth: the data is normalized
    highp float heightFactor;
    highp float lowGain;
    highp float midGain;
    highp float highGain;
    highp float invDevicePixelRatio;
    highp float halfPixelSize;
    highp float waveformLength;
}
ubuf;

// The visual samples, left and right interleaved, row by row: low, mid,
// high and all in r, g, b and a
layout(binding = 1) uniform sampler2D waveformData;

// x: the pixel column, -1 for the axis; y and z: the corner (-1 or 1)
layout(location = 0) in highp vec3 vertexIn;
layout(location = 0) out highp vec3 vColor;

// More frames per pixel than this are not looked at (beyond the widest zoom)
const int kMaxFramesPerPixel = 64;

highp vec4 sampleAt(highp float index) {
    highp float row = floor(index / ubuf.textureWidth);
    highp float column = index - row * ubuf.textureWidth;
    return textureLod(waveformData,
            vec2((column + 0.5) / ubuf.textureWidth, (row + 0.5) / ubuf.textureHeight),
            0.0);
}

void main() {
    highp float column = vertexIn.x;
    highp vec2 corner = vertexIn.yz;
    if (column < 0.0) {
        vColor = ubuf.axesColor.rgb;
        gl_Position = ubuf.matrix *
                vec4(corner.x < 0.0 ? 0.0 : ubuf.waveformLength,
                        ubuf.halfBreadth + 0.5 * corner.y,
                        0.0,
                        1.0);
        return;
    }

    // The frames of the column, rounded like lround() in WaveformRendererRGB
    highp float frame = ubuf.firstFrame + column * ubuf.framesPerPixel;
    highp float range = ubuf.framesPerPixel * 0.5;
    highp float start = floor(frame - range + 0.5);
    highp float stop = max(floor(frame + range + 0.5), start + 1.0);
    highp vec4 maxima = vec4(0.0);
    for (int i = 0; i < kMaxFramesPerPixel; ++i) {
        highp float current = start + float(i);
        if (current >= stop) {
            break;
        }
        if (current >= 0.0 && current < ubuf.frameCount) {
            maxima = max(maxima, sampleAt(current * 2.0));
            maxima = max(maxima, sampleAt(current * 2.0 + 1.0));
        }
    }

    highp vec3 bands = maxima.rgb * vec3(ubuf.lowGain, ubuf.midGain, ubuf.highGain);
    highp float unscaled = maxima.r + maxima.g + maxima.b;
    highp float eqGain = unscaled > 0.0 ? (bands.r + bands.g + bands.b) / unscaled : 1.0;
    highp vec3 color = bands.r * ubuf.lowColor.rgb + bands.g * ubuf.midColor.rgb +
            bands.b * ubuf.highColor.rgb;
    highp float maxComponent = max(color.r, max(color.g, color.b));
    vColor = maxComponent > 0.0 ? color / maxComponent : vec3(0.0);

    highp float height = ubuf.heightFactor * eqGain * maxima.a;
    gl_Position = ubuf.matrix *
            vec4(column * ubuf.invDevicePixelRatio + corner.x * ubuf.halfPixelSize,
                    ubuf.halfBreadth + corner.y * height,
                    0.0,
                    1.0);
}
