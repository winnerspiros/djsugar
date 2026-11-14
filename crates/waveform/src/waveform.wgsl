// Waveform shader. One fragment = one pixel; the whole view is one triangle.
//
// The track's summary lives in a texture, uploaded once per track: for every
// slice two texels, band peaks (low, mid, high, all) then band RMS, each the
// square root of the level scaled to 0..1. Coarser levels (pairs of slices
// merged) follow the finest one, so any zoom reads at most a few texels per
// pixel. Each frame only the uniforms (play position, zoom, markers) change.

struct Params {
    size: vec2<f32>,          // target size in pixels
    play_bin: f32,            // play position, in finest-level slices
    bins_per_pixel: f32,
    num_bins: u32,            // slices in the finest level
    mode: u32,                // 0 = scrolling, 1 = overview
    grid_count: u32,          // tempo sections in grid_* (0 = no grid)
    loop_marking: u32,        // 1 = loop being marked out: grey, up to the play position
    cue_bin: f32,
    loop_start_bin: f32,
    loop_end_bin: f32,        // < loop_start = no loop
    gain: f32,
    num_levels: u32,
    style: u32,               // 0 = RGB (colour by frequency), 1 = stacked bands, 2 = mono (player scrubber), 3 = blue
    scale: f32,               // physical pixels per logical pixel
    spread: f32,              // how far the text atlas's distance field reaches, in atlas pixels
    low_color: vec4<f32>,
    mid_color: vec4<f32>,
    high_color: vec4<f32>,
    background: vec4<f32>,
    hotcues: array<vec4<f32>, 2>,      // 8 hotcue positions in slices, < 0 = unset
    grid_start: vec4<f32>,             // where each tempo section starts, in slices
    grid_beat: vec4<f32>,              // its beat length in slices
    grid_index: vec4<f32>,             // its first beat's place in the bar (0 = bar line)
    grid_bar: vec4<f32>,               // the number of the bar its first beat is in (1 = the first)
    digit: vec4<f32>,                  // text atlas: cell width, cell height, the digits' advance and height
    level_offset: array<vec4<u32>, 4>, // first texel of each level
    level_len: array<vec4<u32>, 4>,    // slices in each level
    phrase_start: array<vec4<f32>, 8>, // where each phrase starts, in slices
    phrase_kind: array<vec4<u32>, 8>,  // its kind: intro, build, drop, breakdown, outro, chorus, verse
    phrase_count: u32,
    minute_bins: f32,                  // slices per minute, for the overview's minute dots (0 = none)
    slip_bin: f32,                     // the silent playhead while slipping, in slices (< 0 = none)
    mix_start_bin: f32,                // the next mix, in slices (Mono style; none unless end > start)
    mix_end_bin: f32,
    live: u32,                         // 1: a live deck; the grid runs on past the last slice
    trace_alpha: f32,                  // the waveform's opacity (less under lanes of marks)
    lane_count: u32,                   // rows of marks from the bottom (0 = none)
    mark_count: u32,
    atlas_w: f32,                      // the text atlas's width and height
    atlas_h: f32,
    half: u32,                         // 1: drawn from the bottom edge up
    grid_edit: u32,                    // 1: the grid is being edited: drawn louder, over the waveform too
    needle: f32,                       // scrolling: where the play line is, as a fraction of the width
    lane_tint: vec4<u32>,              // bit n: row n tinted (a scale note)
    mark_color: vec4<f32>,
    lit_color: vec4<f32>,
    word_width: array<vec4<f32>, 2>,   // each phrase name's width in the text atlas, by kind
    marks: array<vec4<f32>, 256>,      // start and end slice, row, kind (0 played, 1 held, 2 coming, 3 fill, 4 coming fill, 8+ level: 8 + colour * 2^20 + from * 1024 + to)
    vocal: vec4<f32>,                  // vocal levels: first texel, count, finest slices per level, 1 = tint the sung parts
    stems: vec4<f32>,                  // stem levels: first texel (two a level), count, finest slices per level, stems (0 = not drawn)
    stem_colors: array<vec4<f32>, 5>,  // each stem's colour
};

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var bins: texture_2d<f32>;
// Text as a signed distance field (0.5 on the edge): digits 0 to 9 side by
// side in the first row of cells, then a row for each phrase name, by kind.
@group(0) @binding(2) var digits_atlas: texture_2d<f32>;
@group(0) @binding(3) var digits_sampler: sampler;
const ROW: u32 = 2048u;

// Hotcue colours, matching the pads in the skin.
const HOTCUE_COLORS = array<vec3<f32>, 8>(
    vec3<f32>(0.00, 0.39, 0.78), // #0064C8
    vec3<f32>(0.02, 0.74, 0.00), // #04BC00
    vec3<f32>(0.86, 0.72, 0.00), // #DCB800
    vec3<f32>(0.77, 0.35, 0.00), // #C45800
    vec3<f32>(0.75, 0.00, 0.17), // #C0002C
    vec3<f32>(0.77, 0.16, 0.45), // #C42874
    vec3<f32>(0.47, 0.22, 0.77), // #7838C4
    vec3<f32>(0.06, 0.74, 0.74), // #10BCBC
);
const AMBER = vec3<f32>(0.91, 0.70, 0.23);
// A loop: Electric Lilac (#B69CFF); light grey while it is being marked out.
const LOOP_LILAC = vec3<f32>(0.714, 0.612, 1.0);
const LOOP_MARKING = vec3<f32>(0.82, 0.82, 0.84);
const CUE_ORANGE = vec3<f32>(1.0, 0.6, 0.1);
// Mono style (the Player's scrubber): played, the next mix, the rest.
const MONO_LILAC = vec3<f32>(0.714, 0.612, 1.0);
const MONO_MIX = vec3<f32>(0.357, 0.298, 0.541);
const MONO_GREY = vec3<f32>(0.40, 0.40, 0.44);
// Blue style: the body, and what it lightens towards where the highs are loud.
// 3-Band weights (see three_band): the mids and highs against the lows,
// the highs' extra share when the overview stacks the bands, and how much
// darker the mid colour is where the low and the mids overlap.
const BAND_MID: f32 = 0.75;
const BAND_HIGH: f32 = 0.2;
const BAND_HIGH_STACKED: f32 = 2.5;
const BAND_BOTH = vec3<f32>(0.7, 0.6, 0.6);
const BLUE_BODY = vec3<f32>(0.173, 0.482, 0.898);   // #2C7BE5
const BLUE_HIGHS = vec3<f32>(0.863, 0.922, 1.0);    // #DCEBFF
// Show vocals: the strip over the sung parts.
const VOCAL_CYAN = vec3<f32>(0.247, 0.831, 0.933); // #3FD4EE
// Phrase colours, in PhraseKind order: intro, build (up), drop (peak),
// breakdown (break or bridge), outro, chorus (main), verse.
const PHRASE_COLORS = array<vec3<f32>, 7>(
    vec3<f32>(0.141, 0.353, 0.471), // #245A78
    vec3<f32>(0.580, 0.447, 0.835), // #9472D5
    vec3<f32>(0.792, 0.294, 0.408), // #CA4B68
    vec3<f32>(0.914, 0.608, 0.408), // #E99B68
    vec3<f32>(0.141, 0.353, 0.471), // #245A78
    vec3<f32>(0.282, 0.773, 0.631), // #48C5A1
    vec3<f32>(0.788, 0.663, 0.306), // #C9A94E
);

// The colours knobs' levels can be drawn in (MarkKind::Level's colour 1 to 8).
const LEVEL_COLORS = array<vec3<f32>, 8>(
    vec3<f32>(1.000, 0.420, 0.420), // #FF6B6B
    vec3<f32>(1.000, 0.702, 0.278), // #FFB347
    vec3<f32>(1.000, 0.878, 0.400), // #FFE066
    vec3<f32>(0.482, 0.894, 0.584), // #7BE495
    vec3<f32>(0.310, 0.820, 0.773), // #4FD1C5
    vec3<f32>(0.388, 0.702, 0.929), // #63B3ED
    vec3<f32>(0.714, 0.612, 1.000), // #B69CFF
    vec3<f32>(0.969, 0.561, 0.702), // #F78FB3
);

struct Slice {
    peak: vec4<f32>,
    rms: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn level_offset(l: u32) -> u32 {
    return p.level_offset[l / 4u][l % 4u];
}

fn level_len(l: u32) -> u32 {
    return p.level_len[l / 4u][l % 4u];
}

fn texel(t: u32) -> vec4<f32> {
    return textureLoad(bins, vec2<u32>(t % ROW, t / ROW), 0);
}

// Vocal level `j` (0 to 1), four to a texel after the levels.
fn vocal_level(j: u32) -> f32 {
    let v = texel(u32(p.vocal.x) + j / 4u);
    return v[j % 4u];
}

// How much of the track around `bin` is vocals, 0 to 1: interpolated
// between levels, and on the overview the most over the column.
fn vocal_at(bin: f32) -> f32 {
    let count = u32(p.vocal.y);
    if (count == 0u || p.vocal.z <= 0.0 || bin < 0.0) {
        return 0.0;
    }
    let per = p.vocal.z;
    let span = max(p.bins_per_pixel / per, 1.0);
    var most = 0.0;
    let steps = min(u32(ceil(span)), 16u);
    for (var i = 0u; i < steps; i = i + 1u) {
        let x = bin / per - 0.5 * span + (f32(i) + 0.5) * span / f32(steps);
        let j = u32(clamp(floor(x), 0.0, f32(count - 1u)));
        let k = min(j + 1u, count - 1u);
        let v = mix(vocal_level(j), vocal_level(k), clamp(fract(x), 0.0, 1.0));
        most = max(most, v);
    }
    return most;
}

// Stem level `j`: each stem's peak (0 to 1, a square root), two texels
// after the vocal levels.
fn stem_level(j: u32) -> array<f32, 5> {
    let t = u32(p.stems.x) + 2u * j;
    let a = texel(t);
    let b = texel(t + 1u);
    return array<f32, 5>(a.x, a.y, a.z, a.w, b.x);
}

// Each stem's peak around `bin`, as a linear level: interpolated between
// levels, and on the overview the most over the column.
fn stems_at(bin: f32) -> array<f32, 5> {
    var most = array<f32, 5>(0.0, 0.0, 0.0, 0.0, 0.0);
    let count = u32(p.stems.y);
    if (count == 0u || p.stems.z <= 0.0 || bin < 0.0) {
        return most;
    }
    let per = p.stems.z;
    let span = max(p.bins_per_pixel / per, 1.0);
    let steps = min(u32(ceil(span)), 16u);
    for (var i = 0u; i < steps; i = i + 1u) {
        let x = bin / per - 0.5 * span + (f32(i) + 0.5) * span / f32(steps);
        if (x > f32(count)) {
            break;
        }
        let j = u32(clamp(floor(x), 0.0, f32(count - 1u)));
        let k = min(j + 1u, count - 1u);
        let a = stem_level(j);
        let b = stem_level(k);
        let t = clamp(fract(x), 0.0, 1.0);
        for (var s = 0u; s < 5u; s = s + 1u) {
            let v = mix(a[s], b[s], t);
            most[s] = max(most[s], v * v);
        }
    }
    return most;
}

// The Stems look: each stem's shape in its colour, the tallest at the
// back so every stem shows where it is louder than the ones in front.
fn stems_wave(base: vec3<f32>, bin: f32, d: f32, span: f32) -> vec3<f32> {
    var color = base;
    var h = stems_at(bin);
    let n = min(u32(p.stems.w), 5u);
    var done = array<bool, 5>(false, false, false, false, false);
    for (var round = 0u; round < n; round = round + 1u) {
        var pick = 0u;
        var best = -1.0;
        for (var s = 0u; s < n; s = s + 1u) {
            if (!done[s] && h[s] > best) {
                best = h[s];
                pick = s;
            }
        }
        done[pick] = true;
        // The rest are quieter still: under a pixel, they'd only smear the
        // centre line.
        let reach = height(best) * span;
        if (reach < 1.0) {
            break;
        }
        // Pad colours are dark; lift them a little to read on the background.
        let c = mix(p.stem_colors[pick].rgb, vec3<f32>(1.0), 0.18);
        color = over(color, c * 0.75, fill(reach, d) * 0.5);
        color = over(color, c, fill(reach * 0.8, d) * 0.95);
    }
    return color;
}

// Levels are stored as square roots; this returns them as linear levels.
fn load(l: u32, i: i32) -> Slice {
    if (i < 0 || u32(i) >= level_len(l)) {
        return Slice(vec4<f32>(0.0), vec4<f32>(0.0));
    }
    let t = level_offset(l) + 2u * u32(i);
    let pk = texel(t);
    let rm = texel(t + 1u);
    return Slice(pk * pk, rm * rm);
}

// The audio under the slice range [a, b) (in finest-level slices).
fn sample_range(a: f32, b: f32) -> Slice {
    let w = b - a;
    if (w <= 1.0) {
        // Zoomed in further than one slice per pixel: interpolate between
        // slice centres so the outline is smooth rather than stepped.
        let c = (a + b) * 0.5 - 0.5;
        let i = floor(c);
        let t = c - i;
        let s0 = load(0u, i32(i));
        let s1 = load(0u, i32(i) + 1);
        return Slice(mix(s0.peak, s1.peak, t), mix(s0.rms, s1.rms, t));
    }
    // Pick the level where a pixel spans one to two slices. As the zoom
    // glides out towards the next level, fade into it over the last quarter
    // of the way, so the outline doesn't jump where the level changes.
    let at = log2(w);
    let lvl = min(u32(floor(at)), p.num_levels - 1u);
    let fine = sample_level(lvl, a, b);
    let fade = smoothstep(0.75, 1.0, fract(at));
    if (fade <= 0.0 || lvl + 1u >= p.num_levels) {
        return fine;
    }
    let coarse = sample_level(lvl + 1u, a, b);
    return Slice(mix(fine.peak, coarse.peak, fade), mix(fine.rms, coarse.rms, fade));
}

// The slices of level `lvl` under [a, b) (in finest-level slices),
// combined and weighted by how much of each the range covers. The weighting
// keeps peaks from popping as the waveform scrolls.
fn sample_level(lvl: u32, a: f32, b: f32) -> Slice {
    let s = exp2(f32(lvl));
    let la = a / s;
    let lb = b / s;
    var peak = vec4<f32>(0.0);
    var power = vec4<f32>(0.0);
    var total = 0.0;
    let first = i32(floor(la));
    let last = min(i32(ceil(lb)) - 1, first + 63);
    for (var i = first; i <= last; i = i + 1) {
        let cover = clamp(min(lb, f32(i + 1)) - max(la, f32(i)), 0.0, 1.0);
        let sl = load(lvl, i);
        peak = max(peak, sl.peak * smoothstep(0.0, 0.5, cover));
        power = power + sl.rms * sl.rms * cover;
        total = total + cover;
    }
    return Slice(peak, sqrt(power / max(total, 1e-6)));
}

// The two columns a pixel at slice `bin` falls between, and how far it is
// towards the second. Scrolling, columns are fixed to the track (column i
// covers slices i to i + 1 times bins_per_pixel), so each always shows the
// same slices and nothing shimmers. The pixel draws both columns' shapes and
// blends the pictures, so the waveform glides between pixels without its
// peaks shrinking as they cross one (blending the levels instead drew a
// spike at half height halfway between pixels: the outline bobbed up and
// down as it scrolled). The overview takes one column per pixel.
struct Columns {
    a: Slice,
    b: Slice,
    t: f32,
};

fn columns(bin: f32) -> Columns {
    let w = p.bins_per_pixel;
    if (p.mode != 0u) {
        let s = sample_range(bin - 0.5 * w, bin + 0.5 * w);
        return Columns(s, s, 0.0);
    }
    let c = bin / w - 0.5;
    let i = floor(c);
    let a = sample_range(i * w, (i + 1.0) * w);
    let b = sample_range((i + 1.0) * w, (i + 2.0) * w);
    return Columns(a, b, c - i);
}

fn column_bin(x: f32) -> f32 {
    if (p.mode == 0u) {
        return p.play_bin + (x - p.size.x * p.needle) * p.bins_per_pixel;
    }
    return x * p.bins_per_pixel;
}

// Display height for a linear level: a gentle curve so quiet passages and
// breakdowns stay visible next to the drops.
fn height(v: f32) -> f32 {
    return clamp(pow(v, 0.8) * 0.94 * p.gain, 0.0, 1.0);
}

// The waveform is one solid shape: the RMS body, doubled so a loud track
// fills about as much height as its peaks did, and never past the peaks.
// Short hits no longer stick out as a fainter outline.
fn body(rms: f32, peak: f32) -> f32 {
    return min(rms * 2.0, peak);
}

// Coverage of a pixel by a filled shape that reaches `edge` (pixels from the centre).
fn fill(edge: f32, d: f32) -> f32 {
    return clamp(edge - d + 0.5, 0.0, 1.0);
}

// Coverage of a pixel by a vertical line `half_w` pixels either side of `at`.
// On the overview, thin lines snap to the nearest pixel centre so they stay
// crisp instead of smearing across two half-lit columns; scrolling, they
// glide between pixels with the waveform.
fn line(x: f32, at: f32, half_w: f32) -> f32 {
    let c = select(at, floor(at) + 0.5, half_w <= 0.5 && p.mode == 1u);
    return clamp(half_w + 0.5 - abs(x - c), 0.0, 1.0);
}

// How much of pixel `x` the `w` pixels just left of `at` cover.
fn gap_before(x: f32, at: f32, w: f32) -> f32 {
    return clamp(min(x + 0.5, at) - max(x - 0.5, at - w), 0.0, 1.0);
}

// The x pixel of a slice position.
fn bin_x(bin: f32) -> f32 {
    if (p.mode == 0u) {
        return (bin - p.play_bin) / p.bins_per_pixel + p.size.x * p.needle;
    }
    return bin / p.bins_per_pixel;
}

// The phrase at slice `bin`: its index, or -1 before the first or with none.
fn phrase_at(bin: f32) -> i32 {
    var found = -1;
    for (var i = 0u; i < p.phrase_count; i = i + 1u) {
        if (p.phrase_start[i / 4u][i % 4u] <= bin) {
            found = i32(i);
        }
    }
    return found;
}

fn over(base: vec3<f32>, c: vec3<f32>, a: f32) -> vec3<f32> {
    return mix(base, c, clamp(a, 0.0, 1.0));
}

// Colour of a column from the balance of its bands, like Mixxx's RGB waveform:
// bass-heavy is red, mids amber, hats and vocals blue.
fn band_color(s: Slice) -> vec3<f32> {
    let w = s.rms.xyz * s.rms.xyz + 1e-5;
    let c = (w.x * p.low_color.rgb + w.y * p.mid_color.rgb + w.z * p.high_color.rgb) / (w.x + w.y + w.z);
    // Normalise brightness so mixed colours stay as vivid as pure ones.
    return c / max(max(c.r, max(c.g, c.b)), 0.35);
}

// Coverage of pixel (x, y) by the number of the bar whose line is the last
// one at or before it: drawn just right of the line along the top. Drawn
// here rather than by the UI so it moves with the waveform; the digits come
// from a distance field, so they stay sharp between pixels.
fn bar_number(x: f32, y: f32, k: f32, sec: u32) -> f32 {
    let beat_len = p.grid_beat[sec];
    // Only where bars are far enough apart for their numbers.
    if (beat_len <= 0.0 || 4.0 * beat_len / p.bins_per_pixel < 28.0 * k) {
        return 0.0;
    }
    // Atlas pixels per screen pixel, for digits 8 points tall.
    let s = p.digit.w / (8.0 * k);
    let ay = (y - 3.0 * k) * s + p.spread;
    if (ay < 0.0 || ay > p.digit.y) {
        return 0.0;
    }
    let start = p.grid_start[sec];
    let n = floor((column_bin(x) - start) / beat_len);
    let index = i32(p.grid_index[sec]);
    let m = n - f32(((i32(n) + index) % 4 + 4) % 4);
    let at = start + m * beat_len;
    // The first section's bars run back to the start of the track; later
    // ones begin at their section.
    if (at < 0.0 || (at >= f32(p.num_bins) && p.live == 0u) || (sec > 0u && at < start - 0.001 * beat_len)) {
        return 0.0;
    }
    let bar = i32(p.grid_bar[sec]) + (i32(m) + index) / 4;
    if (bar < 1) {
        return 0.0;
    }
    let ax = (x - bin_x(at) - 3.0 * k) * s;
    if (ax < 0.0) {
        return 0.0;
    }
    var digits = 1;
    var place = 1;
    while (bar / (place * 10) > 0) {
        digits = digits + 1;
        place = place * 10;
    }
    let which = i32(floor(ax / p.digit.z));
    if (which >= digits) {
        return 0.0;
    }
    for (var i = 0; i < which; i = i + 1) {
        place = place / 10;
    }
    let digit = (bar / place) % 10;
    let gx = ax - f32(which) * p.digit.z + p.spread;
    let uv = vec2<f32>((f32(digit) * p.digit.x + gx) / p.atlas_w, ay / p.atlas_h);
    let field = textureSampleLevel(digits_atlas, digits_sampler, uv, 0.0).r;
    // The field's distance from the edge, in screen pixels.
    let dist = (field - 0.5) * 2.0 * p.spread / s;
    return clamp(dist + 0.5, 0.0, 1.0);
}

// Coverage of pixel (x, y) by the name of phrase kind `kind`, 8 points
// tall, its pen starting at `left` and its top at `top`.
fn phrase_name(x: f32, y: f32, k: f32, kind: u32, left: f32, top: f32) -> f32 {
    let s = p.digit.w / (8.0 * k);
    let ay = (y - top) * s + p.spread;
    let ax = (x - left) * s + p.spread;
    let w = p.word_width[kind / 4u][kind % 4u];
    if (ay < 0.0 || ay > p.digit.y || ax < 0.0 || ax > w + 2.0 * p.spread) {
        return 0.0;
    }
    let uv = vec2<f32>(ax / p.atlas_w, (f32(kind + 1u) * p.digit.y + ay) / p.atlas_h);
    let field = textureSampleLevel(digits_atlas, digits_sampler, uv, 0.0).r;
    let dist = (field - 0.5) * 2.0 * p.spread / s;
    return clamp(dist + 0.5, 0.0, 1.0);
}

// A phrase kind's colour.
fn phrase_color(kind: u32) -> vec3<f32> {
    return PHRASE_COLORS[min(kind, 6u)];
}

// 3-Band, layered as rekordbox does it. Each band is its RMS body, solid
// (no peak outlines), weighted so the highs stay a highlight on top rather
// than the whole shape: the analysis lifts the mids and highs (by 1.4 and 2)
// so the RGB colours read, which is undone here, and the highs are drawn
// smaller again, as rekordbox does.
//
// Scrolling: the louder of low and mid at the back in its own colour, the
// part where both sound (the quieter one's height) in a darker mid, and the
// highs on top. The overview stacks the bands instead, as rekordbox's
// overview does: low from the line, the mids on it and the highs on top, so
// the highs are a thin edge along the outside.
fn three_band(base: vec3<f32>, s: Slice, d: f32, span: f32) -> vec3<f32> {
    let low = body(s.rms.x, s.peak.x);
    let mid = body(s.rms.y, s.peak.y) * BAND_MID;
    let high = body(s.rms.z, s.peak.z) * BAND_HIGH;
    var color = base;
    if (p.mode == 1u) {
        let lr = height(low) * span;
        let mr = height(low + mid) * span;
        let hr = height(low + mid + high * BAND_HIGH_STACKED) * span;
        color = over(color, p.high_color.rgb, fill(hr, d));
        color = over(color, p.mid_color.rgb, fill(mr, d));
        color = over(color, p.low_color.rgb, fill(lr, d));
        return color;
    }
    let lr = height(low) * span;
    let mr = height(mid) * span;
    let hr = height(high) * span;
    let back = select(p.mid_color.rgb, p.low_color.rgb, lr > mr);
    color = over(color, back, fill(max(lr, mr), d));
    color = over(color, p.mid_color.rgb * BAND_BOTH, fill(min(lr, mr), d));
    color = over(color, p.high_color.rgb, fill(hr, d));
    return color;
}

// Column `s`'s waveform over `base`, at `d` pixels from the centre line.
fn wave(base: vec3<f32>, s: Slice, bin: f32, d: f32, span: f32) -> vec3<f32> {
    var color = base;
    if (p.style == 2u) {
        // Mono: played in lilac, the mix zone in a dim lilac, the rest
        // grey, as one solid shape (the RMS body, without a peak outline).
        var c = MONO_GREY;
        if (bin < p.play_bin) {
            c = MONO_LILAC;
        } else if (p.mix_end_bin > p.mix_start_bin && bin >= p.mix_start_bin && bin <= p.mix_end_bin) {
            c = MONO_MIX;
        }
        let rms_h = height(body(s.rms.w, s.peak.w)) * span;
        color = over(color, c, fill(rms_h, d));
    } else if (p.style == 3u) {
        // Blue: one shape, lighter where the highs are loud.
        let highs = clamp(s.rms.z / max(s.rms.w, 0.05) * 1.2, 0.0, 1.0);
        let c = mix(BLUE_BODY, BLUE_HIGHS, highs * 0.8);
        let rms_h = height(body(s.rms.w, s.peak.w)) * span;
        color = over(color, c, fill(rms_h, d));
    } else if (p.style == 0u || p.style == 4u) {
        // RGB, and Stems for a track without them.
        let c = band_color(s);
        let rms_h = height(body(s.rms.w, s.peak.w)) * span;
        // One solid shape (the RMS body, without a peak outline), slightly
        // brighter towards the centre.
        let shade = 0.82 + 0.18 * clamp(1.0 - d / max(rms_h, 1.0), 0.0, 1.0);
        color = over(color, c * shade, fill(rms_h, d));
    } else {
        color = three_band(color, s, d, span);
    }
    return color;
}

@fragment
fn fs_main(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let x = frag.x;
    let y = frag.y;
    let k = max(p.scale, 1.0);
    let bin = column_bin(x);
    // The overview keeps strips along the bottom, from the top: the
    // waveform, a dot at every minute, then the phrase bar.
    var bar_h = 0.0;
    var dots_h = 0.0;
    if (p.mode == 1u) {
        bar_h = select(0.0, 6.0 * k, p.phrase_count > 0u);
        dots_h = select(0.0, 6.0 * k, p.minute_bins > 0.0);
        // Too short to hold the strips under a waveform (drawn at a
        // placeholder size): the waveform alone, never only the strips.
        if (p.size.y < 3.0 * (bar_h + dots_h)) {
            bar_h = 0.0;
            dots_h = 0.0;
        }
    }
    let wave_h = p.size.y - bar_h - dots_h;
    let half_h = wave_h * 0.5;
    // Pixels from the line the waveform grows from, and the tallest a peak
    // can reach: the centre line, or for a half waveform the bottom edge.
    var d = abs(y - half_h) - 0.5;
    var span = half_h - 1.0;
    // Pixels from the nearer edge marks hang from: both, or only the top.
    var edge_dist = half_h - d;
    if (p.half == 1u) {
        d = select(1e6, wave_h - 0.5 - y, y < wave_h);
        span = wave_h - 2.0;
        edge_dist = y;
    }
    var color = p.background.rgb;

    // Loop region tint.
    let loop_color = select(LOOP_LILAC, LOOP_MARKING, p.loop_marking == 1u);
    if (p.loop_end_bin > p.loop_start_bin && bin >= p.loop_start_bin && bin <= p.loop_end_bin) {
        color = over(color, loop_color, 0.16);
    }

    let has_track = p.num_bins > 0u && bin >= 0.0 && bin < f32(p.num_bins);
    // Where the beat grid shows: over the track, and on a live deck on past
    // the play line too.
    let in_grid = has_track || (p.live == 1u && p.num_bins > 0u && bin >= 0.0);
    // The tempo section this pixel is in: the last one starting before it.
    var sec = 0u;
    for (var i = 1u; i < p.grid_count; i = i + 1u) {
        if (bin >= p.grid_start[i]) { sec = i; }
    }
    let beat_len = p.grid_beat[sec];
    let beats_visible = p.mode == 0u && p.grid_count > 0u && beat_len / p.bins_per_pixel > 4.0 * k;

    // Beat grid behind the waveform: faint beats, amber bars.
    var beat_a = 0.0;
    var bar_a = 0.0;
    if (beats_visible && in_grid) {
        let n = round((bin - p.grid_start[sec]) / beat_len);
        let bx = bin_x(p.grid_start[sec] + n * beat_len);
        let in_bar = ((i32(n) + i32(p.grid_index[sec])) % 4 + 4) % 4;
        let is_bar = in_bar == 0;
        // The first section's beats run back to the start of the track.
        if (sec > 0u || p.grid_start[sec] + n * beat_len >= 0.0) {
            // Bar lines a little wider while the grid is edited.
            let a = line(x, bx, select(0.5, 0.9, is_bar && p.grid_edit == 1u) * k);
            if (is_bar) { bar_a = a; } else { beat_a = a; }
        }
        if (p.grid_edit == 1u) {
            color = over(color, vec3<f32>(1.0), beat_a * 0.30);
            color = over(color, LOOP_LILAC, bar_a * 0.70);
        } else {
            color = over(color, vec3<f32>(1.0), beat_a * 0.10);
            color = over(color, AMBER, bar_a * 0.30);
        }
    }

    // Faint centre line, so silence still reads as a track.
    if (has_track) {
        color = over(color, vec3<f32>(1.0), fill(0.5 * k, d) * 0.12);
    }

    // The waveform; fainter under lanes of marks.
    let under_wave = color;
    if (has_track) {
        let cols = columns(bin);
        let stems = p.style == 4u && p.stems.w > 0.5;
        if (stems) {
            color = stems_wave(color, bin, d, span);
        } else {
            color = mix(wave(color, cols.a, bin, d, span), wave(color, cols.b, bin, d, span), cols.t);
        }
    }

    color = mix(under_wave, color, p.trace_alpha);

    // Show vocals: a thin cyan strip along the top edge where someone
    // sings, so the waveform keeps its own colours (the Stems look shows
    // the vocals in their own colour already).
    if (has_track && p.vocal.w > 0.5 && p.style != 2u && !(p.style == 4u && p.stems.w > 0.5)) {
        let strip = 3.0 * k;
        if (y < strip + 0.5) {
            let v = vocal_at(bin);
            if (v > 0.0) {
                color = over(color, VOCAL_CYAN, v * clamp(strip - y + 0.5, 0.0, 1.0));
            }
        }
    }

    // Lanes of marks (a live deck's notes or drum hits), in rows from the
    // bottom: tinted rows, then each mark on this pixel's row.
    if (p.lane_count > 0u) {
        let lh = wave_h / f32(p.lane_count);
        let row = u32(clamp(floor((wave_h - y) / lh), 0.0, f32(p.lane_count - 1u)));
        let top = wave_h - (f32(row) + 1.0) * lh;
        if (((p.lane_tint[row / 32u] >> (row % 32u)) & 1u) == 1u) {
            color = over(color, p.lit_color.rgb, 0.08);
        }
        // A pixel's gap between rows, where they are tall enough.
        let gap = select(0.0, 0.5 * k, lh > 4.0 * k);
        let y0 = top + gap;
        let y1 = top + lh - gap;
        if (y >= y0 && y <= y1) {
            for (var i = 0u; i < p.mark_count; i = i + 1u) {
                let m = p.marks[i];
                if (u32(m.z) != row) { continue; }
                let x0 = bin_x(m.x);
                let x1 = max(bin_x(m.y), x0 + 2.0 * k);
                if (x < x0 || x > x1) { continue; }
                // A knob's level: a line from one value to the other, with
                // a faint fill under it.
                if (m.w >= 8.0) {
                    let packed = u32(m.w - 8.0);
                    let hue = packed / 1048576u;
                    let code = packed % 1048576u;
                    let v0 = f32(code / 1024u) / 1023.0;
                    let v1 = f32(code % 1024u) / 1023.0;
                    // Colour 0 is the marks' colour; 1 to 8 a colour each,
                    // for lines sharing one lane, with a fainter fill.
                    var ink = p.mark_color.rgb;
                    var under = 0.18;
                    if (hue > 0u) {
                        ink = LEVEL_COLORS[min(hue, 8u) - 1u];
                        under = 0.08;
                    }
                    let span = max(bin_x(m.y) - x0, 1.0);
                    // Half open, so neighbours don't draw their shared pixel twice.
                    if (x >= x0 + span) { continue; }
                    let v = mix(v0, v1, clamp((x - x0) / span, 0.0, 1.0));
                    let pad = min(1.5 * k, 0.2 * (y1 - y0));
                    let ly = y1 - pad - v * (y1 - y0 - 2.0 * pad);
                    let slope = (v1 - v0) * (y1 - y0 - 2.0 * pad) / span;
                    let dist = abs(y - ly) / sqrt(1.0 + slope * slope);
                    if (y > ly) { color = over(color, ink, under); }
                    color = over(color, ink, clamp(1.25 * k - dist, 0.0, 1.0));
                    continue;
                }
                let edge = min(min(x - x0, x1 - x), min(y - y0, y1 - y));
                let outline = edge < 1.25 * k;
                let kind = u32(m.w);
                if (kind == 0u) {
                    color = over(color, p.mark_color.rgb, 0.85);
                } else if (kind == 1u) {
                    color = over(color, mix(p.mark_color.rgb, vec3<f32>(1.0), 0.35), 1.0);
                    if (outline) { color = over(color, p.lit_color.rgb, 1.0); }
                } else if (kind == 2u) {
                    if (outline) { color = over(color, p.mark_color.rgb, 0.9); }
                } else if (kind == 3u) {
                    color = over(color, p.lit_color.rgb, 0.85);
                } else if (outline) {
                    color = over(color, p.lit_color.rgb, 0.9);
                }
            }
        }
    }

    // Beat ticks along the top and bottom edges, over the waveform. While
    // the grid is edited, its lines run through the waveform as well, bars
    // in lilac.
    if (beats_visible && in_grid) {
        let edge = edge_dist;
        let tick = clamp(6.0 * k - edge + 0.5, 0.0, 1.0);
        let editing = p.grid_edit == 1u;
        let bar_color = select(AMBER, LOOP_LILAC, editing);
        if (editing) {
            color = over(color, vec3<f32>(1.0), beat_a * 0.35);
            color = over(color, LOOP_LILAC, bar_a * 0.75);
        }
        color = over(color, vec3<f32>(0.85), beat_a * tick * 0.8);
        color = over(color, bar_color, bar_a * clamp(10.0 * k - edge + 0.5, 0.0, 1.0));
        color = over(color, vec3<f32>(1.0), bar_number(x, y, k, sec) * 0.85);
    }

    // Phrases (overview): a bar along the bottom in each phrase's colour.
    // Each phrase starts exactly on its bar; the gap before the next one is
    // taken from the end of the phrase, so starts line up with cues and grid.
    if (p.phrase_count > 0u && has_track && bar_h > 0.0) {
        let ph = phrase_at(bin);
        // How much of this pixel the gap before its phrase or before the
        // next one covers (the first phrase's start, the track's, has none).
        var edge = 0.0;
        if (ph > 0) {
            let i = u32(ph);
            edge = gap_before(x, bin_x(p.phrase_start[i / 4u][i % 4u]), 2.0 * k);
        }
        let next = u32(ph + 1);
        if (next < p.phrase_count) {
            edge = max(edge, gap_before(x, bin_x(p.phrase_start[next / 4u][next % 4u]), 2.0 * k));
        }
        let from_bottom = p.size.y - y;
        if (ph >= 0 && from_bottom <= bar_h + 0.5) {
            let i = u32(ph);
            let c = phrase_color(p.phrase_kind[i / 4u][i % 4u]);
            let a = clamp(bar_h - from_bottom + 0.5, 0.0, 1.0);
            color = over(color, c, a * 0.9 * (1.0 - edge));
        }
    }

    // Phrases (scrolling): a flag at each start, a line in the phrase's
    // colour down the waveform and its name in a tab just under the bar
    // numbers, clear of the hotcue flags at the top and the lyrics at the
    // foot. Where the waveform is too short for tabs, the lines alone.
    if (p.mode == 0u && p.phrase_count > 0u) {
        let tab_top = 15.0 * k;
        let tab_h = 13.0 * k;
        let tabs = p.size.y >= 40.0 * k;
        let s = p.digit.w / (8.0 * k);
        for (var i = 0u; i < p.phrase_count; i = i + 1u) {
            let sx = bin_x(p.phrase_start[i / 4u][i % 4u]);
            if (x < sx - 2.0 * k || x > sx + 80.0 * k) { continue; }
            let kind = min(p.phrase_kind[i / 4u][i % 4u], 6u);
            let c = phrase_color(kind);
            color = over(color, c, line(x, sx, 0.75 * k) * 0.9);
            if (tabs && y >= tab_top && y <= tab_top + tab_h) {
                let text_w = p.word_width[kind / 4u][kind % 4u] / s;
                let tab_w = text_w + 8.0 * k;
                // How much of this pixel the tab covers. Its sides are
                // smoothed like the lines, so the tab glides with the name
                // in it: cut at whole pixels, its edges stepped while the
                // name moved smoothly, and the name seemed to wobble.
                let cover = clamp(min(x + 0.5, sx + tab_w) - max(x - 0.5, sx), 0.0, 1.0);
                if (cover > 0.0) {
                    color = over(color, c, 0.92 * cover);
                    // Dark letters on the light colours, white on the others.
                    let lum = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
                    let ink = select(vec3<f32>(1.0), vec3<f32>(0.08, 0.07, 0.16), lum > 0.55);
                    color = over(color, ink, phrase_name(x, y, k, kind, sx + 4.0 * k, tab_top + 2.5 * k));
                }
            }
        }
        // A live deck shows the leading deck's phrases: a thin strip along
        // the top as well, so the coming one shows ahead of the play line.
        if (p.live == 1u && y < 3.0 * k) {
            let ph = phrase_at(bin);
            if (ph >= 0) {
                let i = u32(ph);
                var edge = 0.0;
                let next = i + 1u;
                if (next < p.phrase_count) {
                    edge = gap_before(x, bin_x(p.phrase_start[next / 4u][next % 4u]), 2.0 * k);
                }
                let c = phrase_color(p.phrase_kind[i / 4u][i % 4u]);
                color = over(color, c, 0.9 * (1.0 - edge));
            }
        }
    }

    // Loop edges.
    if (p.loop_end_bin > p.loop_start_bin) {
        // No end edge while marking: the play position is the end.
        let end = select(line(x, bin_x(p.loop_end_bin), 0.75 * k), 0.0, p.loop_marking == 1u);
        let a = max(line(x, bin_x(p.loop_start_bin), 0.75 * k), end);
        color = over(color, loop_color, a * 0.9);
    }

    if (p.mode == 1u) {
        // Overview: darken what has been played (Mono colours it instead).
        if (bin < p.play_bin && p.style != 2u) {
            color = color * 0.45;
        }
        // A dot at every minute, between the waveform and the phrase bar,
        // undimmed so they read the same either side of the play position.
        if (dots_h > 0.0) {
            let m = round(bin / p.minute_bins);
            if (m >= 1.0 && m * p.minute_bins < f32(p.num_bins)) {
                let c = vec2<f32>(bin_x(m * p.minute_bins), wave_h + dots_h * 0.5);
                let r = 1.5 * k;
                let dot = clamp(r + 0.5 - length(vec2<f32>(x, y) - c), 0.0, 1.0);
                color = over(color, vec3<f32>(1.0), dot * 0.7);
            }
        }
    }

    // Cue point: a line with flags at both edges (of the waveform, clear of
    // the overview's dots and phrase bar).
    if (p.cue_bin >= 0.0 && y < wave_h && p.style != 2u) {
        let cx = bin_x(p.cue_bin);
        let edge = edge_dist;
        let flag = clamp(5.0 * k - abs(x - cx) - edge * 0.8 + 0.5, 0.0, 1.0);
        color = over(color, CUE_ORANGE, max(line(x, cx, 0.5 * k) * 0.8, flag));
    }

    // Hotcues: a coloured line with a flag at the top.
    for (var h = 0; h < 8; h = h + 1) {
        let pos = p.hotcues[h / 4][h % 4];
        if (pos >= 0.0 && y < wave_h && p.style != 2u) {
            let hx = bin_x(pos);
            let in_flag = x >= hx - 0.5 && x <= hx + 7.0 * k && y <= 7.0 * k;
            let a = max(line(x, hx, 0.5 * k), select(0.0, 1.0, in_flag));
            color = over(color, HOTCUE_COLORS[h], a * 0.95);
        }
    }

    // Slip: the silent playhead, a dashed lilac line where the track will
    // carry on from when the trick ends.
    if (p.slip_bin >= 0.0) {
        let dash = select(0.0, 1.0, fract(y / (6.0 * k)) < 0.6);
        color = over(color, vec3<f32>(0.71, 0.61, 1.0), line(x, bin_x(p.slip_bin), 1.0 * k) * dash);
    }

    // Play position.
    if (p.mode == 1u) {
        color = over(color, vec3<f32>(1.0), line(x, bin_x(p.play_bin), 0.75 * k));
    } else {
        let cx = p.size.x * p.needle;
        // White with a soft shadow either side, readable over any colour.
        color = over(color, vec3<f32>(0.0), clamp(1.0 - (abs(x - cx) - k) / (3.0 * k), 0.0, 1.0) * 0.35);
        color = over(color, vec3<f32>(1.0), line(x, cx, 1.0 * k));
    }

    return vec4<f32>(color, 1.0);
}
