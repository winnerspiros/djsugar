//! GPU waveform rendering.
//!
//! Each [`WaveformView`] owns a texture. Every frame, [`WaveformView::render`]
//! draws the waveform into it with a fragment shader; the UI shows the same
//! texture as an image, so pixels never leave the GPU. A track's summary
//! (band peaks and RMS, at full resolution and at every coarser level) is
//! uploaded once as a [`GpuTrack`] that a deck's views share; per frame only
//! a small uniform block changes.
//!
//! This follows the approach of Mixxx's `allshader` renderers
//! (`src/waveform/renderers/allshader`), on wgpu instead of OpenGL.

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
pub use wgpu;

pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// How the waveform is coloured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// One shape coloured by the balance of the bands (Mixxx's RGB).
    #[default]
    Rgb,
    /// Low, mid and high drawn as separate stacked shapes.
    Bands,
    /// One shape in two flat colours, for a music player's scrubber: the
    /// played part in the accent, the rest grey, the mix zone (see
    /// [`WaveformView::set_mix`]) in a dim accent. No cues or minute dots.
    Mono,
    /// One shape in blue, lighter where the highs are loud (rekordbox's
    /// Blue).
    Blue,
    /// Each stem in its own colour, the loudest at the back, from the
    /// stem levels a [`GpuTrack`] has (see [`Extras`]); [`Style::Rgb`] for a
    /// track without them.
    Stems,
}

/// Stems a waveform draws at most, as the engine plays them.
pub const MAX_STEMS: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Zoomed-in view centred on the play position.
    Scrolling,
    /// The whole track, with the played part darkened.
    Overview,
}

/// What to draw this frame. All positions are in seconds.
#[derive(Clone, Debug)]
pub struct Frame {
    pub play_seconds: f64,
    /// Seconds of audio across the full width (scrolling mode only).
    pub visible_seconds: f64,
    pub duration: f64,
    /// The beat number of the track's first bar, numbered 1 on the scrolling
    /// waveform; a multiple of 4, as bar lines fall every fourth beat from 0.
    pub first_bar_beat: f64,
    /// The beat grid's sections around the view, earliest first.
    pub grid: [Option<GridSection>; GRID_SECTIONS],
    pub cue: Option<f64>,
    pub loop_range: Option<(f64, f64)>,
    /// Where a loop being marked out starts (after loop in, before loop
    /// out); drawn up to the play position.
    pub loop_marking: Option<f64>,
    pub hotcues: [Option<f64>; 8],
    /// Where the silent playhead is while slip mode holds the track
    /// elsewhere; drawn as a ghost play line.
    pub slip: Option<f64>,
    pub gain: f32,
    /// Where each phrase starts and its kind, earliest first; empty to
    /// draw none, at most [`MAX_PHRASES`]. The overview draws them as a
    /// bar along its foot; the scrolling waveform as a flag at each start,
    /// a line in the phrase's colour with its name in a tab (and on a live
    /// deck, which shows the leading deck's phrases, a thin strip along
    /// the top as well).
    pub phrases: Vec<(f64, PhraseKind)>,
}

/// Phrases the shader draws at once: far more than a track has, as a phrase
/// is at least 4 bars long and neighbours of one kind are merged.
pub const MAX_PHRASES: usize = 32;

/// A phrase's kind, which sets its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhraseKind {
    Intro,
    Build,
    Drop,
    Breakdown,
    Outro,
    Chorus,
    Verse,
}

impl PhraseKind {
    /// Every kind, in order.
    pub const ALL: [PhraseKind; 7] = [
        PhraseKind::Intro,
        PhraseKind::Build,
        PhraseKind::Drop,
        PhraseKind::Breakdown,
        PhraseKind::Outro,
        PhraseKind::Chorus,
        PhraseKind::Verse,
    ];

    /// The name on its flag on the scrolling waveform.
    pub fn name(self) -> &'static str {
        match self {
            PhraseKind::Intro => "INTRO",
            PhraseKind::Build => "BUILD",
            PhraseKind::Drop => "DROP",
            PhraseKind::Breakdown => "BREAK",
            PhraseKind::Outro => "OUTRO",
            PhraseKind::Chorus => "CHORUS",
            PhraseKind::Verse => "VERSE",
        }
    }
}

/// Tempo sections the shader draws at once: enough for any view, since a
/// section is at least a few beats long.
pub const GRID_SECTIONS: usize = 4;

/// Where a stretch of steady beats starts, its tempo, and the number of its
/// first beat (beat 0 is a bar line, then every fourth).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSection {
    pub seconds: f64,
    pub bpm: f64,
    pub beat: f64,
}

/// A grid at one tempo, with a bar starting at `first_beat`; none for a
/// tempo of 0.
pub fn steady_grid(bpm: f64, first_beat: f64) -> [Option<GridSection>; GRID_SECTIONS] {
    let mut g = [None; GRID_SECTIONS];
    if bpm > 0.0 {
        g[0] = Some(GridSection {
            seconds: first_beat,
            bpm,
            beat: 0.0,
        });
    }
    g
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    size: [f32; 2],
    play_bin: f32,
    bins_per_pixel: f32,
    num_bins: u32,
    mode: u32,
    grid_count: u32,
    /// 1 while a loop is being marked out: loop_start_bin..loop_end_bin is
    /// drawn grey up to the play position.
    loop_marking: u32,
    cue_bin: f32,
    loop_start_bin: f32,
    loop_end_bin: f32,
    gain: f32,
    num_levels: u32,
    style: u32,
    scale: f32,
    /// How far the text atlas's distance field reaches, in atlas pixels.
    spread: f32,
    low_color: [f32; 4],
    mid_color: [f32; 4],
    high_color: [f32; 4],
    background: [f32; 4],
    hotcues: [[f32; 4]; 2],
    grid_start: [f32; 4],
    grid_beat: [f32; 4],
    grid_index: [f32; 4],
    grid_bar: [f32; 4],
    /// The text atlas: cell width and height, the digits' advance and
    /// height, in atlas pixels.
    digit: [f32; 4],
    level_offset: [[u32; 4]; 4],
    level_len: [[u32; 4]; 4],
    /// Where each phrase starts, in slices, and its kind (as in
    /// [`PhraseKind`]); `phrase_count` of them.
    phrase_start: [[f32; 4]; MAX_PHRASES / 4],
    phrase_kind: [[u32; 4]; MAX_PHRASES / 4],
    phrase_count: u32,
    /// Slices per minute, for the overview's minute dots; 0 for none.
    minute_bins: f32,
    /// The silent playhead while slipping, in slices; < 0 for none.
    slip_bin: f32,
    /// Where the next mix starts and ends, in slices; drawn by the Mono
    /// style only, and only when the end is past the start.
    mix_start_bin: f32,
    mix_end_bin: f32,
    /// 1 for a live deck: the beat grid and bar numbers run on past the
    /// last slice, which is the play line.
    live: u32,
    /// How opaque the waveform is: 1, or less under lanes of marks.
    trace_alpha: f32,
    /// Rows the lanes split the height into (0 for none), and the marks
    /// drawn on them.
    lane_count: u32,
    mark_count: u32,
    /// The text atlas's width and height, in atlas pixels.
    atlas: [f32; 2],
    /// 1: drawn from the bottom edge up.
    half: u32,
    /// 1: the grid is being edited: beats and bar lines drawn louder, over
    /// the waveform too (scrolling only).
    grid_edit: u32,
    /// Scrolling: where the play line is, as a fraction of the width.
    needle: f32,
    _pad: [u32; 2],
    /// Bit `n` set: lane `n` (from the bottom) is tinted.
    lane_tint: [u32; 4],
    mark_color: [f32; 4],
    lit_color: [f32; 4],
    /// Each phrase name's width in the text atlas, in [`PhraseKind`] order.
    word_width: [[f32; 4]; 2],
    /// Each mark's start and end, in slices, its row, and its kind (as in
    /// [`MarkKind`]).
    marks: [[f32; 4]; MAX_MARKS],
    /// The vocal levels: first texel, count, finest slices per level, and
    /// 1 to tint the sung parts.
    vocal: [f32; 4],
    /// The stem levels: first texel, count, finest slices per level, and
    /// how many stems (0: not drawn).
    stems: [f32; 4],
    stem_colors: [[f32; 4]; MAX_STEMS],
}

/// The colours [`MarkKind::Level`]'s `color` 1 to 8 picks, as sRGB hex.
/// The shader keeps the same list.
pub const LEVEL_COLORS: [u32; 8] = [
    0xFF6B6B, 0xFFB347, 0xFFE066, 0x7BE495, 0x4FD1C5, 0x63B3ED, 0xB69CFF, 0xF78FB3,
];

/// Marks the shader draws at once (see [`Lanes`]).
pub const MAX_MARKS: usize = 256;

/// What a mark on a live deck's lanes is, which sets how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkKind {
    /// Played and let go: filled.
    Played,
    /// Still sounding: lit, and runs to the play line.
    Held,
    /// Still to come (a Groove Box's next hits): outlined.
    Coming,
    /// Part of a fill, played: in the lit colour.
    Fill,
    /// Part of a fill still to come: outlined in the lit colour.
    FillComing,
    /// A knob's value over the mark's span, from 0 at the row's foot to 1
    /// at its top, going from `from` to `to`: a line with a faint fill
    /// under it. `color` 0 draws it in the marks' colour; 1 to 8 in one of
    /// eight colours each (see [`LEVEL_COLORS`]), for several knobs on one
    /// lane.
    Level { from: f32, to: f32, color: u8 },
}

impl MarkKind {
    /// How the shader reads it: 0 to 4 for the shapes, 8 and up for a
    /// level: its colour, then its two values in steps of 1/1023, packed
    /// into one number (exact in an `f32`).
    fn code(self) -> f32 {
        let step = |v: f32| (v.clamp(0.0, 1.0) * 1023.0).round();
        match self {
            MarkKind::Played => 0.0,
            MarkKind::Held => 1.0,
            MarkKind::Coming => 2.0,
            MarkKind::Fill => 3.0,
            MarkKind::FillComing => 4.0,
            MarkKind::Level { from, to, color } => {
                8.0 + f32::from(color.min(8)) * 1_048_576.0 + step(from) * 1024.0 + step(to)
            }
        }
    }
}

/// One mark on a lane: a note or a hit, in seconds as the frame counts them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub start: f64,
    pub end: f64,
    /// From the bottom, 0 first.
    pub lane: u32,
    pub kind: MarkKind,
}

/// Rows of marks over a live deck's waveform: a note roll or drum lanes.
#[derive(Clone, Debug, PartialEq)]
pub struct Lanes {
    /// Rows the height is split into, from the bottom.
    pub count: u32,
    /// Bit `n` set: row `n` is tinted (a note of the scale).
    pub tinted: u128,
    /// Earliest first; at most [`MAX_MARKS`] are drawn.
    pub marks: Vec<Mark>,
    /// The waveform's opacity under the marks, from 0 to 1.
    pub trace: f32,
    /// The marks' colour, and the colour of held notes and fills, as RGBA.
    pub color: [f32; 4],
    pub lit: [f32; 4],
}

/// Levels the shader can address; the coarsest is 2^15 slices per texel pair,
/// far more than any overview needs.
const MAX_LEVELS: usize = 16;

/// Band colours; by default bass red, mids amber, highs blue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub low: [f32; 4],
    pub mid: [f32; 4],
    pub high: [f32; 4],
    pub background: [f32; 4],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            low: [1.0, 0.22, 0.16, 1.0],
            mid: [1.0, 0.72, 0.12, 1.0],
            high: [0.30, 0.62, 1.0, 1.0],
            background: [0.05, 0.05, 0.055, 1.0],
        }
    }
}

/// An sRGB colour as `0xRRGGBB`, for a [`Palette`].
const fn rgb(hex: u32) -> [f32; 4] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
        1.0,
    ]
}

/// The palettes the stacked bands ([`Style::Bands`]) come in, as low, mid
/// and high: blue, orange and white as on rekordbox; and lilac (DJ Sugar's
/// accent), cyan and white.
pub const BAND_PALETTES: [[[f32; 4]; 3]; 2] = [
    [rgb(0x0050E0), rgb(0xF8A000), rgb(0xF0E8D0)],
    [rgb(0xB69CFF), rgb(0x22C7E0), rgb(0xF4FBFF)],
];

impl Palette {
    /// The default background with one of [`BAND_PALETTES`] (the first if
    /// out of range).
    pub fn bands(index: usize) -> Self {
        let [low, mid, high] = BAND_PALETTES[if index < BAND_PALETTES.len() {
            index
        } else {
            0
        }];
        Self {
            low,
            mid,
            high,
            ..Self::default()
        }
    }
}

/// Shader and pipeline, shared by all views on one device.
pub struct WaveformRenderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    max_rows: u32,
    digits: DigitAtlas,
}

impl WaveformRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waveform"),
            source: wgpu::ShaderSource::Wgsl(include_str!("waveform.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("waveform"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("waveform"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waveform"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TEXTURE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            layout,
            max_rows: device.limits().max_texture_dimension_2d,
            digits: DigitAtlas::new(device, queue),
        }
    }
}

/// A track's waveform summary on the GPU, shared by every view showing it.
pub struct GpuTrack {
    id: usize,
    texture: wgpu::Texture,
    /// Slices per second in the finest level uploaded.
    bins_per_second: f64,
    level_offset: [[u32; 4]; 4],
    level_len: [[u32; 4]; 4],
    num_levels: u32,
    /// Where the vocal levels start (in texels, four to a texel), how many
    /// there are, and finest-level slices per level; none if `count` is 0.
    vocal: [f32; 3],
    /// Where the stem levels start (two texels a level), how many levels
    /// there are, finest-level slices per level, and how many stems.
    stems: [f32; 4],
    stem_colors: [[f32; 4]; MAX_STEMS],
}

/// What a [`GpuTrack`] can carry beside its waveform.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras<'a> {
    /// Vocal levels, 0 to 255, `vocal_rate` a second, that
    /// [`WaveformView::set_show_vocals`] marks the sung parts with.
    pub vocals: &'a [u8],
    pub vocal_rate: f64,
    /// Each stem's peak along the track, `stem_rate` a second, as the
    /// square root of its level relative to the loudest, scaled to 0..255
    /// (as waveform slices are), for [`Style::Stems`].
    pub stems: &'a [[u8; MAX_STEMS]],
    pub stem_rate: f64,
    /// Each stem's colour as `0xRRGGBB`; as many as there are stems.
    pub stem_colors: &'a [u32],
}

impl GpuTrack {
    /// Uploads `levels` (finest first, 8 bytes per slice: band peaks then band
    /// RMS). If the finest levels don't fit the device's texture limits, which
    /// only happens for tracks of an hour or more on small GPUs, they are
    /// skipped and the next level becomes the finest.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &WaveformRenderer,
        id: usize,
        levels: &[Vec<[u8; 8]>],
        bins_per_second: f64,
    ) -> Self {
        Self::with_extras(
            device,
            queue,
            renderer,
            id,
            levels,
            bins_per_second,
            Extras::default(),
        )
    }

    /// Uploads `levels` as [`GpuTrack::new`] does, with the track's vocal
    /// and stem levels (see [`Extras`]).
    pub fn with_extras(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &WaveformRenderer,
        id: usize,
        levels: &[Vec<[u8; 8]>],
        bins_per_second: f64,
        extras: Extras,
    ) -> Self {
        let vocals = if extras.vocal_rate > 0.0 {
            extras.vocals
        } else {
            &[]
        };
        let stem_count = extras.stem_colors.len().min(MAX_STEMS);
        let stems = if extras.stem_rate > 0.0 && stem_count > 0 {
            extras.stems
        } else {
            &[]
        };
        let capacity = (BIN_ROW as usize * renderer.max_rows as usize)
            .saturating_sub(vocals.len().div_ceil(4) + 2 * stems.len());
        let texels = |from: usize| -> usize {
            levels[from..]
                .iter()
                .take(MAX_LEVELS)
                .map(|l| 2 * l.len())
                .sum()
        };
        let base = (0..levels.len())
            .find(|&b| texels(b) <= capacity)
            .unwrap_or(levels.len());
        let used = &levels[base..levels.len().min(base + MAX_LEVELS)];
        let (data, level_offset, level_len, [vocal_offset, stem_offset]) =
            pack(used, vocals, stems);
        let rows = data.len() / BIN_ROW as usize;
        let texture = bins_texture(device, rows as u32);
        write_bins(queue, &texture, &data);
        let bins_per_second = bins_per_second / 2f64.powi(base as i32);
        Self {
            id,
            texture,
            bins_per_second,
            level_offset,
            level_len,
            num_levels: used.len() as u32,
            vocal: [
                vocal_offset as f32,
                vocals.len() as f32,
                if vocals.is_empty() {
                    0.0
                } else {
                    (bins_per_second / extras.vocal_rate) as f32
                },
            ],
            stems: if stems.is_empty() {
                [0.0; 4]
            } else {
                [
                    stem_offset as f32,
                    stems.len() as f32,
                    (bins_per_second / extras.stem_rate) as f32,
                    stem_count as f32,
                ]
            },
            stem_colors: std::array::from_fn(|k| {
                extras.stem_colors.get(k).map_or([0.0; 4], |&c| rgb(c))
            }),
        }
    }

    /// Writes `levels` over the slices uploaded, in place, when they have
    /// the same lengths (a live deck's waveform, a fixed window that moves
    /// on every frame). Returns false, writing nothing, when they don't; a
    /// new [`GpuTrack`] is needed then.
    pub fn rewrite(&self, queue: &wgpu::Queue, levels: &[Vec<[u8; 8]>]) -> bool {
        let used = &levels[..levels.len().min(MAX_LEVELS)];
        let (data, level_offset, level_len, _) = pack(used, &[], &[]);
        let rows = (data.len() / BIN_ROW as usize) as u32;
        if used.len() as u32 != self.num_levels
            || self.vocal[1] > 0.0
            || self.stems[1] > 0.0
            || level_len != self.level_len
            || level_offset != self.level_offset
            || rows != self.texture.height()
        {
            return false;
        }
        write_bins(queue, &self.texture, &data);
        true
    }

    /// The id passed to [`GpuTrack::new`], to tell whether a track is already uploaded.
    pub fn id(&self) -> usize {
        self.id
    }

    fn num_bins(&self) -> u32 {
        if self.num_levels == 0 {
            0
        } else {
            self.level_len[0][0]
        }
    }
}

/// Lays levels out as the shader reads them: two texels per slice, each
/// level after the one before, then the vocal and stem levels (their first
/// texels returned last), padded to whole rows of [`BIN_ROW`].
#[allow(clippy::type_complexity)]
fn pack(
    used: &[Vec<[u8; 8]>],
    vocals: &[u8],
    stems: &[[u8; MAX_STEMS]],
) -> (Vec<[u8; 4]>, [[u32; 4]; 4], [[u32; 4]; 4], [u32; 2]) {
    let mut level_offset = [[0u32; 4]; 4];
    let mut level_len = [[0u32; 4]; 4];
    let mut data: Vec<[u8; 4]> = Vec::with_capacity(used.iter().map(|l| 2 * l.len()).sum());
    for (i, level) in used.iter().enumerate() {
        level_offset[i / 4][i % 4] = data.len() as u32;
        level_len[i / 4][i % 4] = level.len() as u32;
        for b in level {
            data.push([b[0], b[1], b[2], b[3]]);
            data.push([b[4], b[5], b[6], b[7]]);
        }
    }
    // Then the vocal levels, four to a texel.
    let vocal_offset = data.len() as u32;
    data.extend(
        vocals
            .chunks(4)
            .map(|c| std::array::from_fn(|k| c.get(k).copied().unwrap_or(0))),
    );
    // Then the stem levels, two texels each.
    let stem_offset = data.len() as u32;
    for s in stems {
        data.push([s[0], s[1], s[2], s[3]]);
        data.push([s[4], 0, 0, 0]);
    }
    // Pad to whole rows so the upload is one rectangle.
    let rows = data.len().div_ceil(BIN_ROW as usize).max(1);
    data.resize(rows * BIN_ROW as usize, [0; 4]);
    (data, level_offset, level_len, [vocal_offset, stem_offset])
}

/// Uploads packed slices (see [`pack`]) into a bins texture of their size.
fn write_bins(queue: &wgpu::Queue, texture: &wgpu::Texture, data: &[[u8; 4]]) {
    let rows = (data.len() / BIN_ROW as usize) as u32;
    queue.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(data),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(BIN_ROW * 4),
            rows_per_image: Some(rows),
        },
        wgpu::Extent3d {
            width: BIN_ROW,
            height: rows,
            depth_or_array_layers: 1,
        },
    );
}

/// One on-screen waveform: a texture the UI shows, drawn from a [`GpuTrack`].
pub struct WaveformView {
    mode: Mode,
    style: Style,
    palette: Palette,
    /// Drawn from the bottom up (see [`WaveformView::set_half`]).
    half: bool,
    /// Where the play line is (see [`WaveformView::set_needle`]).
    needle: f32,
    /// Marks the sung parts (see [`WaveformView::set_show_vocals`]).
    show_vocals: bool,
    /// The next mix, as fractions of the track (Mono style).
    mix: Option<(f32, f32)>,
    texture: wgpu::Texture,
    size: (u32, u32),
    params: wgpu::Buffer,
    track: Option<(Arc<GpuTrack>, wgpu::BindGroup)>,
    empty: wgpu::Texture,
    /// Draws a live deck (see [`WaveformView::set_live`]).
    live: bool,
    /// Draws the grid louder (see [`WaveformView::set_grid_edit`]).
    grid_edit: bool,
    /// Lanes of marks over the waveform (see [`WaveformView::set_lanes`]).
    lanes: Option<Lanes>,
}

impl WaveformView {
    pub fn new(device: &wgpu::Device, mode: Mode, width: u32, height: u32) -> Self {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("waveform params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let empty = bins_texture(device, 1);
        Self {
            mode,
            style: Style::default(),
            palette: Palette::default(),
            half: false,
            needle: 0.5,
            show_vocals: false,
            mix: None,
            texture: create_texture(device, width, height),
            size: (width.max(1), height.max(1)),
            params,
            track: None,
            empty,
            live: false,
            grid_edit: false,
            lanes: None,
        }
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// The band colours, keeping the background.
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = Palette {
            background: self.palette.background,
            ..palette
        };
    }

    /// Draws the waveform from the bottom edge up instead of mirrored about
    /// the middle, as rekordbox does. A scrolling view showing lanes of
    /// marks stays whole.
    pub fn set_half(&mut self, half: bool) {
        self.half = half;
    }

    /// Where a scrolling view's play line is, as a fraction of its width
    /// from the left: 0.5 in the middle (the default), 0.25 a quarter in so
    /// more of what is coming shows.
    pub fn set_needle(&mut self, at: f32) {
        self.needle = at.clamp(0.1, 0.9);
    }

    /// Marks where someone sings with a cyan bar along the bottom of a
    /// scrolling view, from the vocal levels its [`GpuTrack::with_extras`]
    /// has. Not on overviews, nor in the Mono style.
    pub fn set_show_vocals(&mut self, show: bool) {
        self.show_vocals = show;
    }

    /// The stretch where the next mix happens, as fractions of the track
    /// from 0 to 1; the Mono style draws it in a dim accent.
    pub fn set_mix(&mut self, mix: Option<(f32, f32)>) {
        self.mix = mix;
    }

    /// Whether the view shows a live deck, whose waveform ends at the play
    /// line: its beat grid and bar numbers then run on past the end.
    /// Draws the beat grid louder, for editing it (scrolling only).
    pub fn set_grid_edit(&mut self, on: bool) {
        self.grid_edit = on;
    }

    pub fn set_live(&mut self, live: bool) {
        self.live = live;
    }

    /// Lanes of marks drawn over the waveform (a live deck's notes or drum
    /// hits), or none.
    pub fn set_lanes(&mut self, lanes: Option<Lanes>) {
        self.lanes = lanes;
    }

    /// The colour behind the waveform, as RGBA from 0 to 1.
    pub fn set_background(&mut self, background: [f32; 4]) {
        self.palette.background = background;
    }

    /// The texture the UI displays. Cheap to clone (reference counted).
    pub fn texture(&self) -> wgpu::Texture {
        self.texture.clone()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Recreates the texture at a new size. Returns true if it changed, in which
    /// case the UI must be handed the new [`texture`](Self::texture).
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        let (w, h) = (width.clamp(1, 8192), height.clamp(1, 8192));
        if (w, h) == self.size {
            return false;
        }
        self.texture = create_texture(device, w, h);
        self.size = (w, h);
        true
    }

    /// Shows `track`, or nothing.
    pub fn set_track(
        &mut self,
        device: &wgpu::Device,
        renderer: &WaveformRenderer,
        track: Option<&Arc<GpuTrack>>,
    ) {
        match track {
            None => self.track = None,
            Some(t) => {
                if matches!(&self.track, Some((cur, _)) if Arc::ptr_eq(cur, t)) {
                    return;
                }
                let bind = bind_group(device, renderer, &self.params, &t.texture);
                self.track = Some((t.clone(), bind));
            }
        }
    }

    /// Draws one frame into the texture. `scale` is physical pixels per
    /// logical pixel, so lines and markers keep their size on HiDPI screens.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &WaveformRenderer,
        frame: Option<&Frame>,
        scale: f32,
    ) {
        let (w, h) = self.size;
        let track = self.track.as_ref().map(|(t, _)| t.as_ref());
        let num_bins = track.map_or(0, GpuTrack::num_bins);
        let bins_per_second = track.map_or(1.0, |t| t.bins_per_second);
        let to_bin = |s: f64| (s * bins_per_second) as f32;
        let mut params = Params {
            size: [w as f32, h as f32],
            play_bin: 0.0,
            bins_per_pixel: 1.0,
            num_bins,
            mode: if self.mode == Mode::Scrolling { 0 } else { 1 },
            grid_count: 0,
            loop_marking: 0,
            cue_bin: -1.0,
            loop_start_bin: 0.0,
            loop_end_bin: -1.0,
            gain: 1.0,
            num_levels: track.map_or(0, |t| t.num_levels),
            style: match self.style {
                Style::Rgb => 0,
                Style::Bands => 1,
                Style::Mono => 2,
                Style::Blue => 3,
                Style::Stems => 4,
            },
            scale,
            spread: DIGIT_SPREAD,
            low_color: self.palette.low,
            mid_color: self.palette.mid,
            high_color: self.palette.high,
            background: self.palette.background,
            hotcues: [[-1.0; 4]; 2],
            grid_start: [0.0; 4],
            grid_beat: [0.0; 4],
            grid_index: [0.0; 4],
            grid_bar: [0.0; 4],
            digit: renderer.digits.metrics,
            level_offset: track.map_or([[0; 4]; 4], |t| t.level_offset),
            level_len: track.map_or([[0; 4]; 4], |t| t.level_len),
            phrase_start: [[0.0; 4]; MAX_PHRASES / 4],
            phrase_kind: [[0; 4]; MAX_PHRASES / 4],
            phrase_count: 0,
            minute_bins: 0.0,
            slip_bin: -1.0,
            mix_start_bin: 0.0,
            mix_end_bin: -1.0,
            live: self.live as u32,
            trace_alpha: 1.0,
            lane_count: 0,
            mark_count: 0,
            atlas: renderer.digits.size,
            half: (self.half && (self.mode == Mode::Overview || self.lanes.is_none())) as u32,
            grid_edit: (self.grid_edit && self.mode == Mode::Scrolling) as u32,
            needle: if self.mode == Mode::Scrolling {
                self.needle
            } else {
                0.5
            },
            _pad: [0; 2],
            lane_tint: [0; 4],
            mark_color: [0.0; 4],
            lit_color: [0.0; 4],
            word_width: renderer.digits.words,
            marks: [[0.0; 4]; MAX_MARKS],
            vocal: [0.0; 4],
            stems: [0.0; 4],
            stem_colors: [[0.0; 4]; MAX_STEMS],
        };
        if let (Some(t), Style::Stems) = (track, self.style) {
            params.stems = t.stems;
            params.stem_colors = t.stem_colors;
        }
        if let (Some(t), true) = (track, self.show_vocals) {
            if t.vocal[1] > 0.0 && t.vocal[2] > 0.0 {
                params.vocal = [t.vocal[0], t.vocal[1], t.vocal[2], 1.0];
            }
        }
        if let Some(l) = &self.lanes {
            params.trace_alpha = l.trace;
            params.lane_count = l.count.min(128);
            params.lane_tint = std::array::from_fn(|i| (l.tinted >> (32 * i)) as u32);
            params.mark_color = l.color;
            params.lit_color = l.lit;
        }
        if let Some((a, b)) = self.mix.filter(|_| self.style == Style::Mono) {
            params.mix_start_bin = a * num_bins as f32;
            params.mix_end_bin = b * num_bins as f32;
        }
        if let (Some(f), true) = (frame, num_bins > 0) {
            let bins_per_pixel = match self.mode {
                Mode::Scrolling => f.visible_seconds * bins_per_second / w as f64,
                Mode::Overview => num_bins as f64 / w as f64,
            };
            params.bins_per_pixel = bins_per_pixel as f32;
            let play_bin = f.play_seconds * bins_per_second;
            // Not rounded to whole pixels: the shader keeps each column on the
            // same slices and moves the picture smoothly between pixels.
            params.play_bin = play_bin as f32;
            for (k, section) in f.grid.iter().map_while(|g| g.as_ref()).enumerate() {
                if section.bpm <= 0.0 {
                    break;
                }
                params.grid_start[k] = to_bin(section.seconds);
                params.grid_beat[k] = to_bin(60.0 / section.bpm);
                // Only the bar position matters, which keeps the float exact.
                params.grid_index[k] = section.beat.rem_euclid(4.0) as f32;
                params.grid_bar[k] = ((section.beat - f.first_bar_beat) / 4.0).floor() as f32 + 1.0;
                params.grid_count = k as u32 + 1;
            }
            params.cue_bin = f.cue.map(to_bin).unwrap_or(-1.0);
            if let Some((a, b)) = f.loop_range {
                params.loop_start_bin = to_bin(a);
                params.loop_end_bin = to_bin(b);
            } else if let Some(a) = f.loop_marking {
                params.loop_start_bin = to_bin(a);
                params.loop_end_bin = play_bin as f32;
                params.loop_marking = 1;
            }
            params.slip_bin = f.slip.map(to_bin).unwrap_or(-1.0);
            params.gain = f.gain;
            for (i, hc) in f.hotcues.iter().enumerate() {
                params.hotcues[i / 4][i % 4] = hc.map(to_bin).unwrap_or(-1.0);
            }
            for (i, (seconds, kind)) in f.phrases.iter().take(MAX_PHRASES).enumerate() {
                params.phrase_start[i / 4][i % 4] = to_bin(*seconds);
                params.phrase_kind[i / 4][i % 4] = *kind as u32;
                params.phrase_count = i as u32 + 1;
            }
            if let Some(l) = &self.lanes {
                for (i, m) in l.marks.iter().take(MAX_MARKS).enumerate() {
                    params.marks[i] =
                        [to_bin(m.start), to_bin(m.end), m.lane as f32, m.kind.code()];
                    params.mark_count = i as u32 + 1;
                }
            }
            if f.duration > 60.0 && self.style != Style::Mono {
                params.minute_bins = to_bin(60.0);
            }
        }
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&params));

        let fallback;
        let bind = match &self.track {
            Some((_, bind)) => bind,
            None => {
                fallback = bind_group(device, renderer, &self.params, &self.empty);
                &fallback
            }
        };
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("waveform"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("waveform"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&renderer.pipeline);
            pass.set_bind_group(0, bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
    }
}

fn create_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("waveform"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// Bins per texture row; matches `ROW` in the shader. 2048 is the smallest
/// maximum texture width wgpu guarantees (WebGL2/GLES downlevel limits).
const BIN_ROW: u32 = 2048;

fn bins_texture(device: &wgpu::Device, rows: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("waveform bins"),
        size: wgpu::Extent3d {
            width: BIN_ROW,
            height: rows.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn bind_group(
    device: &wgpu::Device,
    renderer: &WaveformRenderer,
    params: &wgpu::Buffer,
    bins: &wgpu::Texture,
) -> wgpu::BindGroup {
    let view = bins.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("waveform"),
        layout: &renderer.layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&renderer.digits.view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&renderer.digits.sampler),
            },
        ],
    })
}

/// The font the bar numbers and phrase names are drawn in (tabular, so
/// every digit is as wide).
const DIGIT_FONT: &[u8] = include_bytes!("../../app/ui/fonts/InterTabular-SemiBold.otf");
/// The size the text is rasterized at; the distance field keeps its edges
/// sharp at any size and position on screen.
const DIGIT_PX: f32 = 64.0;
/// How far the distance field reaches either side of an edge, in atlas pixels.
const DIGIT_SPREAD: f32 = 6.0;

/// The text the shader draws, as a signed distance field (0.5 on a glyph's
/// edge, more inside, less outside): the digits 0 to 9 side by side in the
/// first row of cells, then each phrase name (see [`PhraseKind::name`]) on
/// a row of its own, in [`PhraseKind`] order.
struct DigitAtlas {
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    /// Cell width and height, the digits' advance and their height, in
    /// atlas pixels.
    metrics: [f32; 4],
    /// The atlas's width and height.
    size: [f32; 2],
    /// Each phrase name's width, from its row's left edge plus the spread.
    words: [[f32; 4]; 2],
}

impl DigitAtlas {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let atlas = text_field();
        let (width, height) = (atlas.width, atlas.height);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("waveform text"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Rows padded to the 256 bytes a texture upload needs.
        let row = width.next_multiple_of(256);
        let mut padded = vec![0u8; row * height];
        for y in 0..height {
            padded[y * row..y * row + width]
                .copy_from_slice(&atlas.field[y * width..(y + 1) * width]);
        }
        queue.write_texture(
            texture.as_image_copy(),
            &padded,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row as u32),
                rows_per_image: Some(height as u32),
            },
            wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("waveform text"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mut words = [[0.0; 4]; 2];
        for (i, w) in atlas.words.iter().enumerate() {
            words[i / 4][i % 4] = *w;
        }
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            sampler,
            metrics: [
                atlas.cell_w as f32,
                atlas.cell_h as f32,
                atlas.advance,
                atlas.digit_h,
            ],
            size: [width as f32, height as f32],
            words,
        }
    }
}

/// The text atlas as [`text_field`] builds it.
struct TextField {
    cell_w: usize,
    cell_h: usize,
    width: usize,
    height: usize,
    /// `width * height` bytes, row by row.
    field: Vec<u8>,
    advance: f32,
    digit_h: f32,
    words: [f32; 7],
}

/// Rasterizes the digits and the phrase names and turns them into a
/// distance field. Each cell has its pen position `DIGIT_SPREAD` pixels in
/// from its left and the digits' top `DIGIT_SPREAD` pixels down; the names
/// start `DIGIT_SPREAD` pixels in and sit on the same baseline in their rows.
fn text_field() -> TextField {
    use ab_glyph::{Font, FontRef, OutlinedGlyph, PxScale, ScaleFont};
    let font = FontRef::try_from_slice(DIGIT_FONT).expect("digit font");
    let scaled = font.as_scaled(PxScale::from(DIGIT_PX));
    let glyph = |c: char, x: f32| {
        scaled.outline_glyph(
            scaled
                .glyph_id(c)
                .with_scale_and_position(DIGIT_PX, ab_glyph::point(x, scaled.ascent())),
        )
    };
    let digits: Vec<Option<OutlinedGlyph>> = ('0'..='9').map(|c| glyph(c, 0.0)).collect();
    // Each name laid out with its advances and kerning.
    let mut words: Vec<(Vec<OutlinedGlyph>, f32)> = Vec::new();
    for kind in PhraseKind::ALL {
        let mut x = 0.0;
        let mut prev = None;
        let mut glyphs = Vec::new();
        for c in kind.name().chars() {
            let id = scaled.glyph_id(c);
            if let Some(p) = prev {
                x += scaled.kern(p, id);
            }
            glyphs.extend(glyph(c, x));
            x += scaled.h_advance(id);
            prev = Some(id);
        }
        words.push((glyphs, x));
    }
    let advance = scaled.h_advance(scaled.glyph_id('0'));
    let digit_bounds = digits.iter().flatten().map(|g| g.px_bounds());
    let (top, bottom) = digit_bounds.fold((f32::MAX, f32::MIN), |(t, b), r| {
        (t.min(r.min.y), b.max(r.max.y))
    });
    let digit_h = bottom - top;
    // Round letters overshoot the digits by a pixel or so, into the
    // spread above and below them; the cells grow for any more below.
    let wbottom = words
        .iter()
        .flat_map(|(g, _)| g.iter().map(|g| g.px_bounds().max.y))
        .fold(bottom, f32::max);
    let pad = DIGIT_SPREAD as usize;
    let cell_w = advance.ceil() as usize + 2 * pad;
    let cell_h = (wbottom - top).ceil() as usize + 2 * pad;
    let widest = words.iter().map(|(_, w)| *w).fold(0.0, f32::max);
    let width = (cell_w * 10).max(widest.ceil() as usize + 2 * pad);
    let height = cell_h * (1 + words.len());
    let mut cover = vec![0f32; width * height];
    let mut draw = |g: &OutlinedGlyph, left: f32, row: usize| {
        let r = g.px_bounds();
        g.draw(|x, y, c| {
            let cx = r.min.x + x as f32 + left;
            let cy = r.min.y - top + y as f32 + DIGIT_SPREAD;
            if cx >= 0.0 && (cx as usize) < width && cy >= 0.0 && (cy as usize) < cell_h {
                cover[(row * cell_h + cy as usize) * width + cx as usize] = c;
            }
        });
    };
    for (d, g) in digits.iter().enumerate() {
        if let Some(g) = g {
            draw(g, (d * cell_w) as f32 + DIGIT_SPREAD, 0);
        }
    }
    for (i, (glyphs, _)) in words.iter().enumerate() {
        for g in glyphs {
            draw(g, DIGIT_SPREAD, i + 1);
        }
    }
    // Then the distance to the nearest pixel on the other side of the edge,
    // within the spread, signed (inside positive) and mapped to 0..1. Rows
    // are fields of their own, so names never reach into each other.
    let reach = pad as i64;
    let mut field = vec![0u8; width * height];
    for y in 0..height as i64 {
        let row = y / cell_h as i64;
        let (r0, r1) = (row * cell_h as i64, (row + 1) * cell_h as i64);
        for x in 0..width as i64 {
            let inside = cover[y as usize * width + x as usize] >= 0.5;
            let mut best = DIGIT_SPREAD;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < r0 || nx >= width as i64 || ny >= r1 {
                        continue;
                    }
                    if (cover[ny as usize * width + nx as usize] >= 0.5) != inside {
                        let dist = ((dx * dx + dy * dy) as f32).sqrt() - 0.5;
                        best = best.min(dist);
                    }
                }
            }
            let signed = if inside { best } else { -best };
            let v = 0.5 + signed / (2.0 * DIGIT_SPREAD);
            field[y as usize * width + x as usize] = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
    }
    let mut widths = [0.0; 7];
    for (i, (_, w)) in words.iter().enumerate() {
        widths[i] = *w;
    }
    TextField {
        cell_w,
        cell_h,
        width,
        height,
        field,
        advance,
        digit_h,
        words: widths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_holds_the_digits_and_every_phrase_name() {
        let t = text_field();
        assert_eq!(t.height, t.cell_h * 8);
        assert!(t.width >= t.cell_w * 10);
        // Every row has some ink, and each name fits the atlas.
        for row in 0..8 {
            let ink = (row * t.cell_h..(row + 1) * t.cell_h)
                .flat_map(|y| t.field[y * t.width..(y + 1) * t.width].iter())
                .any(|&v| v > 128);
            assert!(ink, "row {row} is blank");
        }
        for w in t.words {
            assert!(w > t.advance && w + 2.0 * DIGIT_SPREAD <= t.width as f32);
        }
        // CHORUS is the widest name, DROP the narrowest.
        assert!(t.words[5] > t.words[2]);
    }
}
