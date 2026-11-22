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

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// Surface: 0 flat, 1 radial, 2 brushed metal, 3 vinyl, 4 knurled, 5 grain.
// Every textured surface also gets a lit top edge and a shaded bottom edge.
fn surface(i: Out, base: vec4<f32>, d: f32) -> vec4<f32> {
    let m = u32(i.shape.w + 0.5);
    if m < 2u {
        return base;
    }
    var l = 1.0;
    let p = i.local;
    let r = length(p);
    if m == 2u {
        l = 0.9 + 0.14 * vnoise(vec2(p.x * 0.02, p.y * 1.7)) + 0.05 * hash(floor(p * vec2(0.5, 1.0)));
    } else if m == 3u {
        let groove = 0.5 + 0.5 * sin(r * 2.6);
        let ang = atan2(p.y, p.x);
        let sheen = pow(abs(cos(ang * 2.0 - 0.8)), 6.0) * 0.45;
        l = 0.82 + 0.14 * groove + sheen + 0.06 * hash(vec2(floor(r * 1.3), 1.0));
    } else if m == 4u {
        let ang = atan2(p.y, p.x);
        let ridge = 0.5 + 0.5 * cos(ang * 48.0);
        let rim = smoothstep(i.half.x * 0.78, i.half.x * 0.86, r);
        let light = 0.5 + 0.5 * dot(normalize(p + vec2(0.0001)), vec2(-0.5, -0.85));
        l = mix(1.0 + 0.25 * (0.6 - p.y / max(i.half.y, 1.0) * 0.5), 0.75 + 0.35 * ridge * light + 0.2 * light, rim);
    } else {
        l = 0.93 + 0.12 * hash(floor(p));
    }
    // Bevel: light from above along the top rim, shade along the bottom
    let edge = clamp(1.0 + d / 2.5, 0.0, 1.0);
    let up = clamp(-p.y / max(i.half.y, 1.0), -1.0, 1.0);
    l += edge * up * 0.35;
    return vec4(base.rgb * l, base.a);
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let d = rounded(i.local, i.half, i.shape.x);
    let soft = i.shape.z + 1.0;
    let outer = clamp(0.5 - d / soft, 0.0, 1.0);
    let inner = clamp(0.5 - d - i.shape.y, 0.0, 1.0);
    var t = clamp(i.local.y / max(i.half.y, 1.0) * 0.5 + 0.5, 0.0, 1.0);
    if i.shape.w > 0.5 && i.shape.w < 1.5 || i.shape.w > 2.5 && i.shape.w < 4.5 {
        t = clamp(length(i.local) / max(i.half.x, 1.0), 0.0, 1.0);
    }
    let fill = surface(i, mix(i.fill, i.fill2, t), d);
    var c = mix(i.stroke, fill, select(1.0, inner, i.shape.y > 0.0)) * outer;
    if i.arc.y > 0.0 {
        let a = fract(atan2(i.local.x, -i.local.y) / TAU - i.arc.x + 1.0);
        let edge = 1.0 / max(TAU * length(i.local), 1.0);
        c *= clamp((i.arc.y - a) / edge + 0.5, 0.0, 1.0);
    }
    return c;
}
