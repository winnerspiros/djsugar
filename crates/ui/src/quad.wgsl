struct Quad {
    @location(0) rect: vec4<f32>,
    @location(1) shape: vec2<f32>,
    @location(2) fill: u32,
    @location(3) stroke: u32,
}

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) half: vec2<f32>,
    @location(2) @interpolate(flat) shape: vec2<f32>,
    @location(3) @interpolate(flat) fill: vec4<f32>,
    @location(4) @interpolate(flat) stroke: vec4<f32>,
}

@group(0) @binding(0) var<uniform> viewport: vec2<f32>;

fn linear(c: u32) -> vec4<f32> {
    let s = unpack4x8unorm(c);
    let rgb = select(pow((s.rgb + 0.055) / 1.055, vec3(2.4)), s.rgb / 12.92, s.rgb <= vec3(0.04045));
    return vec4(rgb * s.a, s.a);
}

@vertex
fn vs(@builtin(vertex_index) v: u32, q: Quad) -> Out {
    let corner = vec2(f32(v & 1u), f32(v >> 1u));
    let half = q.rect.zw * 0.5;
    let local = (corner * 2.0 - 1.0) * (half + 1.0);
    let px = q.rect.xy + half + local;
    var o: Out;
    o.pos = vec4(px / viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    o.local = local;
    o.half = half;
    o.shape = vec2(min(q.shape.x, min(half.x, half.y)), q.shape.y);
    o.fill = linear(q.fill);
    o.stroke = linear(q.stroke);
    return o;
}

fn rounded(p: vec2<f32>, half: vec2<f32>, r: f32) -> f32 {
    let d = abs(p) - half + r;
    return length(max(d, vec2(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let d = rounded(i.local, i.half, i.shape.x);
    let outer = clamp(0.5 - d, 0.0, 1.0);
    let inner = clamp(0.5 - d - i.shape.y, 0.0, 1.0);
    return mix(i.stroke, i.fill, select(1.0, inner, i.shape.y > 0.0)) * outer;
}
