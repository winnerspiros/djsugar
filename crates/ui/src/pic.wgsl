struct Pic {
    @location(0) rect: vec4<f32>,
    @location(1) uv: vec4<u32>,
    @location(2) radius: f32,
    @location(3) tint: u32,
}

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) corner: vec2<f32>,
    @location(1) @interpolate(flat) origin: vec2<f32>,
    @location(2) @interpolate(flat) size: vec2<f32>,
    @location(3) @interpolate(flat) half: vec2<f32>,
    @location(4) @interpolate(flat) shape: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: vec2<f32>;
@group(0) @binding(2) var pictures: texture_2d<f32>;

@vertex
fn vs(@builtin(vertex_index) v: u32, p: Pic) -> Out {
    let corner = vec2(f32(v & 1u), f32(v >> 1u));
    let px = p.rect.xy + corner * p.rect.zw;
    var o: Out;
    o.pos = vec4(px / viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    o.corner = corner;
    o.origin = vec2<f32>(p.uv.xy);
    o.size = vec2<f32>(p.uv.zw);
    o.half = p.rect.zw * 0.5;
    o.shape = vec2(min(p.radius, min(o.half.x, o.half.y)), unpack4x8unorm(p.tint).a);
    return o;
}

fn tap(i: Out, at: vec2<f32>) -> vec4<f32> {
    let p = clamp(at - 0.5, vec2(0.0), i.size - 1.0);
    let base = floor(p);
    let f = p - base;
    let a = vec2<i32>(i.origin + base);
    let b = vec2<i32>(i.origin + min(base + 1.0, i.size - 1.0));
    let top = mix(textureLoad(pictures, a, 0), textureLoad(pictures, vec2(b.x, a.y), 0), f.x);
    let bottom = mix(textureLoad(pictures, vec2(a.x, b.y), 0), textureLoad(pictures, b, 0), f.x);
    return mix(top, bottom, f.y);
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    // Four taps a pixel, spread over the texels it covers, so a picture drawn small does not shimmer.
    let at = i.corner * i.size;
    let step = i.size / (i.half * 2.0) * 0.25;
    let c = (tap(i, at + step * vec2(-1.0, -1.0)) + tap(i, at + step * vec2(1.0, -1.0))
        + tap(i, at + step * vec2(-1.0, 1.0)) + tap(i, at + step)) * 0.25;
    let p = (i.corner - 0.5) * 2.0 * i.half;
    let q = abs(p) - i.half + i.shape.x;
    let d = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - i.shape.x;
    return vec4(c.rgb, 1.0) * clamp(0.5 - d, 0.0, 1.0) * i.shape.y;
}
