struct Quad {
    @location(0) rect: vec4<f32>,
    @location(1) shape: vec4<f32>,
    @location(2) fill: u32,
    @location(3) stroke: u32,
    @location(4) fill2: u32,
    @location(5) arc: u32,
}

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half: vec2<f32>,
    @location(2) @interpolate(flat) shape: vec4<f32>,
    @location(3) @interpolate(flat) fill: vec4<f32>,
    @location(4) @interpolate(flat) stroke: vec4<f32>,
    @location(5) @interpolate(flat) fill2: vec4<f32>,
    @location(6) @interpolate(flat) arc: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: vec2<f32>;

const TAU: f32 = 6.2831853;

fn linear(c: u32) -> vec4<f32> {
    let s = unpack4x8unorm(c);
    let rgb = select(pow((s.rgb + 0.055) / 1.055, vec3(2.4)), s.rgb / 12.92, s.rgb <= vec3(0.04045));
    return vec4(rgb * s.a, s.a);
}

@vertex
fn vs(@builtin(vertex_index) v: u32, q: Quad) -> Out {
    let corner = vec2(f32(v & 1u), f32(v >> 1u));
    let half = q.rect.zw * 0.5;
    let pad = 1.0 + q.shape.z;
    let local = (corner * 2.0 - 1.0) * (half + pad);
    let px = q.rect.xy + half + local;
    var o: Out;
    o.pos = vec4(px / viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    o.local = local;
    o.half = half;
    o.shape = vec4(min(q.shape.x, min(half.x, half.y)), q.shape.y, q.shape.z, q.shape.w);
    o.fill = linear(q.fill);
    o.stroke = linear(q.stroke);
    o.fill2 = select(o.fill, linear(q.fill2), q.fill2 != 0u);
    o.arc = unpack2x16unorm(q.arc);
    return o;
}

fn rounded(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let d = abs(p) - half + r;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let d = rounded(i.local, i.half, i.shape.x);
    let soft = i.shape.z + 1.0;
    let outer = clamp(0.5 - d / soft, 0.0, 1.0);
    let inner = clamp(0.5 - d - i.shape.y, 0.0, 1.0);
    var t = clamp(i.local.y / max(i.half.y, 1.0) * 0.5 + 0.5, 0.0, 1.0);
    if i.shape.w > 0.5 {
        t = clamp(length(i.local) / max(i.half.x, 1.0), 0.0, 1.0);
    }
    let fill = mix(i.fill, i.fill2, t);
    var c = mix(i.stroke, fill, select(1.0, inner, i.shape.y > 0.0)) * outer;
    if i.arc.y > 0.0 {
        let a = fract(atan2(i.local.x, -i.local.y) / TAU - i.arc.x + 1.0);
        let edge = 1.0 / max(TAU * length(i.local), 1.0);
        c *= clamp((i.arc.y - a) / edge + 0.5, 0.0, 1.0);
    }
    return c;
}
