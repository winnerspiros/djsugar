//! Instruments' and inputs' own images, shown on the cover of a deck
//! playing them (`Cover` in `ui/deck.slint`). An image file dropped on such
//! a deck, or chosen from its cover's right-click menu, is copied to
//! `Instruments/Art/` in the settings folder, named after the plugin's key
//! or the input. Without one, a plugin shows the art it brings: its NKS
//! artwork (Native Instruments' `NI Resources/image/<maker>/<plugin>/`),
//! else the icon in its bundle (macOS). Without any, the deck shows its
//! icon on the deck color, the same square a track without a cover gets.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use slint::{
    ComponentHandle, DataTransfer, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel,
};

/// `live` for a deck playing the Mixer.
const MIXER: i32 = djsugar_engine::inputs::MIXER as i32;

use crate::{settings, AppWindow, InstrumentsUi, TrackDrag};

/// File types taken as images.
const IMAGE_TYPES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// The inputs' names for their files, in `InstrumentsUi.input-art` order:
/// the microphone, aux inputs 1 to 3 (decks saved before the Mixer) and
/// the Mixer.
const INPUTS: [&str; 5] = ["microphone", "aux-1", "aux-2", "aux-3", "mixer"];

/// A ticked plugin, as its art is looked for.
#[derive(Clone, Debug, Default)]
pub struct Plugin {
    pub key: String,
    pub name: String,
    pub maker: String,
}

thread_local! {
    /// The ticked plugins, in the picker's order.
    static PLUGINS: RefCell<Vec<Plugin>> = const { RefCell::new(Vec::new()) };
    /// The art each plugin brings, by key, once looked for.
    static BROUGHT: RefCell<HashMap<String, Option<Image>>> = RefCell::new(HashMap::new());
}

/// Where Native Instruments keeps NKS artwork, by maker then product.
fn nks_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(PathBuf::from("/Users/Shared/NI Resources/image"))
    } else if cfg!(windows) {
        let public = std::env::var_os("PUBLIC")
            .map_or_else(|| PathBuf::from(r"C:\Users\Public"), PathBuf::from);
        Some(public.join("Documents").join("NI Resources").join("image"))
    } else {
        None
    }
}

/// A name as NKS folder names are compared: lower case letters and digits.
fn plain(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The NKS artwork for `name` by `maker` under `root`, if installed.
fn nks_art(root: &Path, maker: &str, name: &str) -> Option<PathBuf> {
    let (maker, name) = (plain(maker), plain(name));
    if maker.is_empty() || name.is_empty() {
        return None;
    }
    let find = |dir: &Path, want: &str| -> Option<PathBuf> {
        std::fs::read_dir(dir)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.is_dir()
                    && p.file_name()
                        .is_some_and(|f| plain(&f.to_string_lossy()) == want)
            })
    };
    let product = find(&find(root, &maker)?, &name)?;
    ["MST_artwork.png", "VB_artwork.png"]
        .iter()
        .map(|f| product.join(f))
        .find(|p| p.is_file())
}

/// The bundle a plugin key names (a `.clap` or `.vst3`), if any.
fn bundle(key: &str) -> Option<PathBuf> {
    let rest = key
        .strip_prefix("clap:")
        .or_else(|| key.strip_prefix("vst3:"))?;
    let (file, _) = rest.rsplit_once('#')?;
    Some(PathBuf::from(file))
}

/// The largest PNG picture in an `.icns` file's bytes: its entries from
/// 256 points up hold PNG data as it is.
fn icns_png(bytes: &[u8]) -> Option<&[u8]> {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.get(..4)? != b"icns" {
        return None;
    }
    let mut best: Option<&[u8]> = None;
    let mut at = 8;
    while at + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[at + 4..at + 8].try_into().ok()?) as usize;
        if len < 8 || at + len > bytes.len() {
            break;
        }
        let data = &bytes[at + 8..at + len];
        if data.starts_with(PNG) && best.is_none_or(|b| data.len() > b.len()) {
            best = Some(data);
        }
        at += len;
    }
    best
}

/// The icon in a plugin bundle's resources (macOS bundles keep one as
/// `.icns`), as a picture.
fn bundle_icon(bundle: &Path) -> Option<Image> {
    let resources = bundle.join("Contents").join("Resources");
    let mut icons: Vec<PathBuf> = std::fs::read_dir(resources)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("icns"))
        })
        .collect();
    icons.sort();
    icons.iter().find_map(|p| {
        let bytes = std::fs::read(p).ok()?;
        picture(icns_png(&bytes)?)
    })
}

/// A PNG's bytes as a picture.
fn picture(png: &[u8]) -> Option<Image> {
    let rgba = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
    );
    Some(Image::from_rgba8(buffer))
}

/// The art plugin `p` brings, looked for once.
fn brought(p: &Plugin) -> Option<Image> {
    if let Some(found) = BROUGHT.with_borrow(|b| b.get(&p.key).cloned()) {
        return found;
    }
    let found = nks_root()
        .and_then(|root| nks_art(&root, &p.maker, &p.name))
        .and_then(|path| Image::load_from_path(&path).ok())
        .or_else(|| bundle(&p.key).and_then(|b| bundle_icon(&b)));
    BROUGHT.with_borrow_mut(|b| b.insert(p.key.clone(), found.clone()));
    found
}

fn root() -> Option<PathBuf> {
    settings::config_dir().map(|d| d.join("Instruments").join("Art"))
}

/// A file name from a key: its last part (the plugin's id, or the
/// input's name) with letters, digits, `-` and `.` kept, then a hash of all
/// of it, since plugin keys hold long paths.
fn stem(key: &str) -> String {
    let tail = key.rsplit(['#', ':', '/']).next().unwrap_or(key);
    let readable: String = tail
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    });
    format!("{readable}-{hash:016x}")
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_TYPES.contains(&e.to_ascii_lowercase().as_str()))
}

/// The image file kept for `key` in `dir`, if any.
fn find(dir: &Path, key: &str) -> Option<PathBuf> {
    let stem = stem(key);
    IMAGE_TYPES
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|p| p.is_file())
}

/// Keeps a copy of `image` in `dir` as `key`'s, in place of any before.
fn keep(dir: &Path, key: &str, image: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    forget(dir, key);
    let ext = image
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_ascii_lowercase();
    let to = dir.join(format!("{}.{ext}", stem(key)));
    std::fs::copy(image, &to)?;
    Ok(to)
}

fn forget(dir: &Path, key: &str) {
    while let Some(old) = find(dir, key) {
        if std::fs::remove_file(&old).is_err() {
            break;
        }
    }
}

/// The key `live` (see `InstrumentsUi.live`) keeps its image under, with
/// `instrument` the plugin picked (from 1).
fn key_of(live: i32, instrument: i32) -> Option<String> {
    match live {
        3 => PLUGINS.with_borrow(|p| {
            usize::try_from(instrument - 1)
                .ok()
                .and_then(|i| p.get(i).map(|p| p.key.clone()))
        }),
        1 => Some(INPUTS[0].into()),
        2 => Some(INPUTS[1].into()),
        4 => Some(INPUTS[2].into()),
        5 => Some(INPUTS[3].into()),
        MIXER => Some(INPUTS[4].into()),
        _ => None,
    }
}

fn load(dir: Option<&Path>, key: &str) -> Option<Image> {
    let path = find(dir?, key)?;
    match Image::load_from_path(&path) {
        Ok(image) => Some(image),
        Err(e) => {
            eprintln!("instrument art {}: {e:?}", path.display());
            None
        }
    }
}

fn set_list(images: Vec<Option<Image>>, set: impl Fn(ModelRc<Image>, ModelRc<bool>)) {
    let has: Vec<bool> = images.iter().map(Option::is_some).collect();
    let images: Vec<Image> = images.into_iter().map(Option::unwrap_or_default).collect();
    set(
        ModelRc::new(VecModel::from(images)),
        ModelRc::new(VecModel::from(has)),
    );
}

/// Shows each plugin's and input's image again.
fn refresh(app: &AppWindow) {
    let ui = app.global::<InstrumentsUi>();
    let dir = root();
    let plugins: Vec<Plugin> = PLUGINS.with_borrow(|p| p.clone());
    let own: Vec<Option<Image>> = plugins
        .iter()
        .map(|p| load(dir.as_deref(), &p.key))
        .collect();
    ui.set_plugin_own_art(ModelRc::new(VecModel::from(
        own.iter().map(Option::is_some).collect::<Vec<bool>>(),
    )));
    let shown = own
        .into_iter()
        .zip(&plugins)
        .map(|(own, p)| own.or_else(|| brought(p)))
        .collect();
    set_list(shown, |art, has| {
        ui.set_plugin_art(art);
        ui.set_plugin_has_art(has);
    });
    let inputs = INPUTS.iter().map(|k| load(dir.as_deref(), k)).collect();
    set_list(inputs, |art, has| {
        ui.set_input_art(art);
        ui.set_input_has_art(has);
    });
}

/// The ticked plugins changed: in the picker's order.
pub fn show_plugins(app: &AppWindow, plugins: Vec<Plugin>) {
    PLUGINS.set(plugins);
    refresh(app);
}

/// Keeps `image` as `live`'s and shows it.
fn set(app: &AppWindow, live: i32, instrument: i32, image: &Path) {
    let (Some(dir), Some(key)) = (root(), key_of(live, instrument)) else {
        return;
    };
    match keep(&dir, &key, image) {
        Ok(to) => eprintln!("instrument art for {key}: {}", to.display()),
        Err(e) => eprintln!("instrument art for {key}: {e}"),
    }
    refresh(app);
}

/// Hooks up the drop, choose and remove callbacks. `take_files` gives the
/// files of the last drop from outside (see `crate::file_drop`).
pub fn install(app: &AppWindow, take_files: impl Fn() -> Vec<PathBuf> + 'static) {
    let ui = app.global::<InstrumentsUi>();
    let weak = app.as_weak();
    ui.on_drop_art(move |deck, live, instrument| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let files = take_files();
        match files.iter().find(|p| is_image(p)) {
            Some(image) => set(&app, live, instrument, image),
            // Not an image: a track to load, as on any deck.
            None if !files.is_empty() => {
                let mut data = DataTransfer::default();
                data.set_file_paths(files);
                app.global::<TrackDrag>().invoke_drop(deck, data);
            }
            None => {}
        }
    });
    let weak = app.as_weak();
    ui.on_choose_art(move |live, instrument| {
        let weak = weak.clone();
        crate::prefs::choose_file("Choose an image", move |path| {
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(app) = weak.upgrade() {
                    if is_image(&path) {
                        set(&app, live, instrument, &path);
                    }
                }
            });
        });
    });
    let weak = app.as_weak();
    ui.on_remove_art(move |live, instrument| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        if let (Some(dir), Some(key)) = (root(), key_of(live, instrument)) {
            forget(&dir, &key);
        }
        refresh(&app);
    });
    refresh(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_is_kept_under_its_key_in_place_of_the_last() {
        let dir = std::env::temp_dir().join(format!("djsugar-art-{}", std::process::id()));
        let src = dir.join("src");
        std::fs::create_dir_all(&src).unwrap();
        let png = src.join("Juno.PNG");
        let jpg = src.join("juno.jpg");
        std::fs::write(&png, b"png").unwrap();
        std::fs::write(&jpg, b"jpg").unwrap();
        let art = dir.join("art");
        let key = "clap:/Library/Audio/Plug-Ins/CLAP/Juno-6 V.clap#com.arturia.Juno-6 V";
        assert!(find(&art, key).is_none());
        let kept = keep(&art, key, &png).unwrap();
        let name = kept.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            name.starts_with("com.arturia.Juno-6_V-") && name.ends_with(".png"),
            "{name}"
        );
        assert_ne!(
            stem(key),
            stem("clap:/elsewhere/Juno-6 V.clap#com.arturia.Juno-6 V")
        );
        assert_eq!(find(&art, key), Some(kept.clone()));
        // A new one replaces it, whatever its type.
        let kept = keep(&art, key, &jpg).unwrap();
        assert_eq!(find(&art, key), Some(kept));
        assert_eq!(std::fs::read_dir(&art).unwrap().count(), 1);
        forget(&art, key);
        assert!(find(&art, key).is_none());
        assert!(is_image(&png) && !is_image(&src.join("track.mp3")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plugins_nks_art_is_found_by_maker_and_name() {
        let root = std::env::temp_dir().join(format!("djsugar-nks-{}", std::process::id()));
        let product = root.join("arturia").join("jun-6 v");
        std::fs::create_dir_all(&product).unwrap();
        assert_eq!(nks_art(&root, "Arturia", "Jun-6 V"), None);
        std::fs::write(product.join("VB_artwork.png"), b"vb").unwrap();
        assert_eq!(
            nks_art(&root, "Arturia", "Jun-6 V"),
            Some(product.join("VB_artwork.png"))
        );
        std::fs::write(product.join("MST_artwork.png"), b"mst").unwrap();
        assert_eq!(
            nks_art(&root, "ARTURIA", "Jun 6 V"),
            Some(product.join("MST_artwork.png"))
        );
        assert_eq!(nks_art(&root, "Arturia", "Mini V"), None);
        assert_eq!(nks_art(&root, "", "Jun-6 V"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_icns_gives_its_largest_png() {
        let entry = |kind: &[u8; 4], data: &[u8]| {
            let mut e = kind.to_vec();
            e.extend_from_slice(&((data.len() + 8) as u32).to_be_bytes());
            e.extend_from_slice(data);
            e
        };
        let small = b"\x89PNG\r\n\x1a\nsmall".to_vec();
        let big = b"\x89PNG\r\n\x1a\nthe big one".to_vec();
        let body = [
            entry(b"is32", b"old pixels, not a png"),
            entry(b"ic07", &small),
            entry(b"ic10", &big),
        ]
        .concat();
        let mut file = b"icns".to_vec();
        file.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
        file.extend_from_slice(&body);
        assert_eq!(icns_png(&file), Some(&big[..]));
        assert_eq!(icns_png(b"nope"), None);
        assert_eq!(
            bundle("clap:/Library/Audio/Plug-Ins/CLAP/Jun.clap#com.x.jun"),
            Some(PathBuf::from("/Library/Audio/Plug-Ins/CLAP/Jun.clap"))
        );
        assert_eq!(bundle("au:aumu:jun6:Artu"), None);
    }
}
