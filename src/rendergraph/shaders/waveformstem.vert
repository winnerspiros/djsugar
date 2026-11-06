#version 440

// The stem waveform drawn by the GPU (allshader::WaveformRendererStemGpu):
// the geometry is made once, eight rectangles per strip of pixels (the
// outline and the fill of each of four stems, in drawing order), and this
// shader gives them their heights and colors from the stem data in a
// texture, as WaveformRendererStem does on the CPU.

layout(std140, binding = 0) uniform buf {
    highp mat4 matrix;
    highp vec4 color0;
    highp vec4 color1;
    highp vec4 color2;
    highp vec4 color3;
    // The stem drawn at each layer of the stack, bottom first
    highp vec4 stackOrder;
    // The gain of the fill of each stem (volume, mute, selection)
    highp vec4 stemGain;
    highp float firstFrame;
    highp float framesPerPixel;
    highp float frameCount;
    highp float textureWidth;
    highp float textureHeight;
    highp float halfBreadth;
    highp float stemBreadth;
    // allGain * halfBreadth: the data is normalized
    highp float heightFactor;
    // 1 when the stems are drawn on separate tracks
    highp float split;
    highp float opacity;
    highp float outlineOpacity;
    highp float stemCount;
    highp float stripWidth;
    highp float halfStripWidth;
    // The EQ, applied like on the RGB waveform (waveformrgb.vert)
    highp float lowGain;
    highp float midGain;
    highp float highGain;
}
ubuf;

// The visual samples, left and right interleaved, row by row: the four stems
// in r, g, b and a
layout(binding = 1) uniform sampler2D waveformData;
// The same samples of the whole track: low, mid, high and all
layout(binding = 2) uniform sampler2D bandData;

// x: the strip, y and z: the corner (-1 or 1), w: the layer (stack layer * 2,
// + 1 for the fill)
layout(location = 0) in highp vec4 vertexIn;
layout(location = 0) out highp vec4 vColor;

const int kMaxFramesPerPixel = 64;

highp vec2 texelAt(highp float index) {
    highp float row = floor(index / ubuf.textureWidth);
    highp float column = index - row * ubuf.textureWidth;
    return vec2((column + 0.5) / ubuf.textureWidth, (row + 0.5) / ubuf.textureHeight);
}

highp float component(highp vec4 values, highp float index) {
    return index < 0.5 ? values.x : (index < 1.5 ? values.y : (index < 2.5 ? values.z : values.w));
}

highp vec4 stemColor(highp float index) {
    return index < 0.5 ? ubuf.color0
                       : (index < 1.5 ? ubuf.color1 : (index < 2.5 ? ubuf.color2 : ubuf.color3));
}

void main() {
    highp float strip = vertexIn.x;
    highp vec2 corner = vertexIn.yz;
    highp float stackLayer = floor(vertexIn.w * 0.5);
    bool fill = vertexIn.w - stackLayer * 2.0 > 0.5;
    highp float stem = component(ubuf.stackOrder, stackLayer);

    highp vec4 color = stemColor(stem);
    color.a *= fill ? ubuf.opacity : ubuf.outlineOpacity;

    // The frames of the strip, rounded like lround() in WaveformRendererStem
    highp float frame = ubuf.firstFrame + strip * ubuf.framesPerPixel;
    highp float range = ubuf.framesPerPixel * 0.5;
    highp float start = floor(frame - range + 0.5);
    highp float stop = max(floor(frame + range + 0.5), start + 1.0);
    highp float maximum = 0.0;
    highp vec3 bands = vec3(0.0);
    for (int i = 0; i < kMaxFramesPerPixel; ++i) {
        highp float current = start + float(i);
        if (current >= stop) {
            break;
        }
        if (current >= 0.0 && current < ubuf.frameCount) {
            highp vec2 left = texelAt(current * 2.0);
            highp vec2 right = texelAt(current * 2.0 + 1.0);
            maximum = max(maximum, component(textureLod(waveformData, left, 0.0), stem));
            maximum = max(maximum, component(textureLod(waveformData, right, 0.0), stem));
            bands = max(bands, textureLod(bandData, left, 0.0).rgb);
            bands = max(bands, textureLod(bandData, right, 0.0).rgb);
        }
    }
    // The EQ: the height scaled by how much of the bands of the mix are left
    highp float unscaled = bands.r + bands.g + bands.b;
    if (unscaled > 0.0) {
        maximum *= (bands.r * ubuf.lowGain + bands.g * ubuf.midGain + bands.b * ubuf.highGain) /
                unscaled;
    }
    if (fill) {
        maximum *= component(ubuf.stemGain, stem);
    }
    if (stem >= ubuf.stemCount) {
        // A stem the track doesn't have
        maximum = 0.0;
        color.a = 0.0;
    }

    highp float height = ubuf.heightFactor * maximum;
    if (ubuf.split > 0.5) {
        height = min(height, ubuf.halfBreadth);
    }
    highp float center = (ubuf.split > 0.5 ? stem : stackLayer) * ubuf.stemBreadth +
            ubuf.halfBreadth;
    vColor = color;
    gl_Position = ubuf.matrix *
            vec4(strip * ubuf.stripWidth + corner.x * ubuf.halfStripWidth,
                    center + corner.y * height,
                    0.0,
                    1.0);
}
