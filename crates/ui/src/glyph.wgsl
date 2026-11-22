struct Glyph {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec4<u32>,
    @location(2) colour: u32,
}

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) @interpolate(flat) origin: vec2<i32>,
    @location(1) @interpolate(flat) colour: vec4<f32>,
    @location(2) @interpolate(flat) at: vec2<f32>,
}

@group(0) @binding(0) var<uniform> viewport: vec2<f32>;
@group(0) @binding(1) var atlas: texture_2d<f32>;

fn linear(c: u32) -> vec4<f32> {
    let s = unpack4x8unorm(c);
    let rgb = select(pow((s.rgb + 0.055) / 1.055, vec3(2.4)), s.rgb / 12.92, s.rgb <= vec3(0.04045));
    return vec4(rgb * s.a, s.a);
}

@vertex
fn vs(@builtin(vertex_index) v: u32, g: Glyph) -> Out {
    let corner = vec2(f32(v & 1u), f32(v >> 1u));
    let px = g.pos + corner * vec2<f32>(g.uv.zw);
    var o: Out;
    o.pos = vec4(px / viewport * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0);
    o.origin = vec2<i32>(g.uv.xy);
    o.at = g.pos;
    o.colour = linear(g.colour);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let texel = i.origin + vec2<i32>(floor(i.pos.xy - i.at));
    let a = textureLoad(atlas, texel, 0).r;
    return i.colour * a;
}
