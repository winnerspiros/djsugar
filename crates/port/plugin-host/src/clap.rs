//! CLAP effects inside the helper, through `clack-host`. The plugin instance lives on the helper's main thread; its audio processor is
//! sent to the helper's audio thread, where `ClapEffect` runs one block per wake. Parameters arrive through the shared block as plain
//! values and are sent to the plugin as events when they change; the routed deck's tempo and beat position go in as a transport event.

use std::cell::{Cell, RefCell};
use std::ffi::{CStr, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use clack_extensions::audio_ports::{AudioPortInfoBuffer, PluginAudioPorts};
use clack_extensions::gui::{GuiApiType, GuiConfiguration, GuiSize, HostGui, HostGuiImpl, PluginGui, Window};
use clack_extensions::latency::PluginLatency;
use clack_extensions::params::{ParamInfoBuffer, ParamInfoFlags, PluginParams};
#[cfg(all(unix, not(target_os = "macos")))]
use clack_extensions::posix_fd::{FdFlags, HostPosixFd, HostPosixFdImpl, PluginPosixFd};
use clack_extensions::preset_discovery::indexer::IndexerImpl;
use clack_extensions::preset_discovery::metadata_receiver::MetadataReceiverImpl;
use clack_extensions::preset_discovery::prelude::{
    FileType, Flags, Location, LocationInfo, PluginPresetLoad, Provider, Soundpack, Timestamp, UniversalPluginId,
};
use clack_extensions::remote_controls::{PluginRemoteControls, RemoteControlsPageBuffer};
use clack_extensions::state::PluginState;
use clack_extensions::timer::{HostTimer, HostTimerImpl, PluginTimer, TimerId};
use clack_host::events::event_types::{
    MidiEvent, NoteOffEvent, NoteOnEvent, ParamValueEvent, TransportEvent, TransportFlags,
};
use clack_host::events::Match;
use clack_host::events::{EventFlags, EventHeader};
use clack_host::prelude::*;
use clack_host::utils::{BeatTime, SecondsTime};

use crate::client::Transport;
use crate::descriptor::Descriptor;
use crate::effects::{arrange, EditorSize, Effect, Info, Loaded, MainThread, Param, ParentWindow};
use crate::shm::{Midi, CHANNELS, MAX_EVENTS, MAX_PARAMS};

fn text(s: Option<&CStr>) -> String {
    s.map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

fn load_entry(file: &Path) -> Result<PluginEntry, String> {
    // SAFETY: loading a plugin runs its code; that is why this happens in a
    // helper process that DJ Sugar can lose without harm.
    unsafe { PluginEntry::load(file) }.map_err(|e| format!("can't load {}: {e}", file.display()))
}

/// Lists the plugins in one `.clap` file. Runs in a throwaway helper
/// (`djsugar-plugin-host --scan`), never in DJ Sugar.
pub fn scan(file: &Path) -> Result<Vec<Descriptor>, String> {
    let entry = load_entry(file)?;
    let factory = entry.get_plugin_factory().ok_or("the file has no plugin factory")?;
    Ok(factory
        .plugin_descriptors()
        .map(|d| Descriptor {
            id: text(d.id()),
            name: text(d.name()),
            vendor: text(d.vendor()),
            version: text(d.version()),
            features: d.features().map(|f| f.to_string_lossy().into_owned()).collect(),
        })
        .collect())
}

/// How deep under a declared preset folder files are looked for.
const PRESET_DEPTH: usize = 4;
/// At most this many presets for one plugin.
const MOST_PRESETS: usize = 2000;
/// Splits a preset reference's parts: (kind, path, load key).
const SEP: char = '\u{1f}';

/// What a preset provider declares: its preset files' types and where it
/// keeps presets.
#[derive(Default)]
struct Declared {
    /// Extensions, lower case; an empty one matches every file.
    types: Vec<String>,
    /// `None` for presets inside the plugin itself, else a file or folder.
    locations: Vec<Option<std::path::PathBuf>>,
}

impl IndexerImpl for Declared {
    fn declare_filetype(&mut self, file_type: FileType) -> Result<(), HostError> {
        self.types.push(file_type.file_extension.map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default());
        Ok(())
    }

    fn declare_location(&mut self, location: LocationInfo) -> Result<(), HostError> {
        self.locations
            .push(location.location.file_path().map(|p| std::path::PathBuf::from(p.to_string_lossy().into_owned())));
        Ok(())
    }

    fn declare_soundpack(&mut self, _soundpack: Soundpack) -> Result<(), HostError> {
        Ok(())
    }
}

/// The presets a provider describes at one location, for plugin `id`.
struct Receiver<'a> {
    id: &'a str,
    /// The location's reference, before the load key.
    at: String,
    /// A name for a preset without one: the file's.
    fallback: String,
    /// The preset being described: (name, load key, plugin ids).
    current: Option<(String, String, Vec<String>)>,
    found: Vec<(String, String)>,
}

impl Receiver<'_> {
    fn finish(&mut self) {
        if let Some((name, key, ids)) = self.current.take() {
            if ids.is_empty() || ids.iter().any(|i| i == self.id) {
                let name = if name.is_empty() { self.fallback.clone() } else { name };
                self.found.push((name, format!("{}{SEP}{key}", self.at)));
            }
        }
    }
}

impl MetadataReceiverImpl for Receiver<'_> {
    fn on_error(&mut self, _code: i32, _message: Option<&CStr>) {}

    fn begin_preset(&mut self, name: Option<&CStr>, load_key: Option<&CStr>) -> Result<(), HostError> {
        self.finish();
        self.current =
            Some((text(name), load_key.map(|k| k.to_string_lossy().into_owned()).unwrap_or_default(), Vec::new()));
        Ok(())
    }

    fn add_plugin_id(&mut self, plugin_id: UniversalPluginId) {
        if plugin_id.abi.to_bytes() == b"clap" {
            if let Some((_, _, ids)) = &mut self.current {
                ids.push(plugin_id.id.to_string_lossy().into_owned());
            }
        }
    }

    fn set_soundpack_id(&mut self, _soundpack_id: &CStr) {}
    fn set_flags(&mut self, _flags: Flags) {}
    fn add_creator(&mut self, _creator: &CStr) {}
    fn set_description(&mut self, _description: &CStr) {}
    fn set_timestamps(&mut self, _created: Option<Timestamp>, _modified: Option<Timestamp>) {}
    fn add_feature(&mut self, _feature: &CStr) {}
    fn add_extra_info(&mut self, _key: &CStr, _value: &CStr) {}
}

/// The files under `dir` (or `dir` itself, a file) of the `types` given.
fn preset_files(dir: &Path, types: &[String], depth: usize, out: &mut Vec<std::path::PathBuf>) {
    if dir.is_file() {
        out.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= MOST_PRESETS {
            return;
        }
        if path.is_dir() {
            if depth > 0 {
                preset_files(&path, types, depth - 1, out);
            }
            continue;
        }
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if types.is_empty() || types.iter().any(|t| t.is_empty() || *t == ext) {
            out.push(path);
        }
    }
}

/// Plugin `id`'s presets, from the file's preset discovery providers, as
/// (name, reference for [`MainThread::load_preset`]). Runs in a throwaway
/// helper (`djsugar-plugin-host --presets`), never in DJ Sugar.
pub fn presets(file: &Path, id: &str) -> Result<Vec<(String, String)>, String> {
    let entry = load_entry(file)?;
    let host = host_info()?;
    let ids: Vec<CString> = entry
        .get_factory::<clack_extensions::preset_discovery::prelude::PresetDiscoveryFactory>()
        .map(|f| f.provider_descriptors().filter_map(|d| d.id().map(CStr::to_owned)).collect())
        .unwrap_or_default();
    let mut found = Vec::new();
    for provider_id in ids {
        let Ok(mut provider) = Provider::instantiate(Declared::default(), &entry, &provider_id, &host) else {
            continue;
        };
        let types = provider.indexer().types.clone();
        let locations = provider.indexer().locations.clone();
        for location in locations {
            let receiver =
                |at: String, fallback: String| Receiver { id, at, fallback, current: None, found: Vec::new() };
            match location {
                None => {
                    let mut r = receiver(format!("plugin{SEP}"), String::new());
                    provider.get_metadata(Location::Plugin, &mut r);
                    r.finish();
                    found.extend(r.found);
                }
                Some(dir) => {
                    let mut files = Vec::new();
                    preset_files(&dir, &types, PRESET_DEPTH, &mut files);
                    for f in files {
                        let Ok(path) = CString::new(f.to_string_lossy().into_owned()) else {
                            continue;
                        };
                        let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        let mut r = receiver(format!("file{SEP}{}", f.display()), name);
                        provider.get_metadata(Location::File { path: &path }, &mut r);
                        r.finish();
                        found.extend(r.found);
                    }
                }
            }
            if found.len() >= MOST_PRESETS {
                break;
            }
        }
    }
    found.truncate(MOST_PRESETS);
    Ok(found)
}

fn host_info() -> Result<HostInfo, String> {
    HostInfo::new("DJ Sugar", "DJ Sugar", "https://github.com/winnerspiros/djsugar", env!("CARGO_PKG_VERSION"))
        .map_err(|e| e.to_string())
}

struct Host;

#[derive(Default)]
struct HostShared {
    callback: AtomicBool,
    /// A size the editor asked its window to take.
    resize: Mutex<Option<GuiSize>>,
    /// The plugin closed (or lost) its editor.
    gui_closed: AtomicBool,
}

impl<'a> SharedHandler<'a> for HostShared {
    fn request_restart(&self) {}
    fn request_process(&self) {}
    fn request_callback(&self) {
        self.callback.store(true, Ordering::Release);
    }
}

impl HostGuiImpl for HostShared {
    fn resize_hints_changed(&self) {}

    fn request_resize(&self, new_size: GuiSize) -> Result<(), HostError> {
        *self.resize.lock().unwrap_or_else(|e| e.into_inner()) = Some(new_size);
        Ok(())
    }

    fn request_show(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn request_hide(&self) -> Result<(), HostError> {
        Ok(())
    }

    fn closed(&self, _was_destroyed: bool) {
        self.gui_closed.store(true, Ordering::Release);
    }
}

/// Timers (and, on Linux, file descriptors) the plugin asked for, served
/// from the helper's main loop.
#[derive(Default)]
struct HostMain {
    timers: RefCell<Vec<ClapTimer>>,
    next_timer: Cell<u32>,
    #[cfg(all(unix, not(target_os = "macos")))]
    fds: RefCell<Vec<(i32, FdFlags)>>,
}

struct ClapTimer {
    id: u32,
    period: Duration,
    next: Instant,
}

impl<'a> MainThreadHandler<'a> for HostMain {}

impl HostTimerImpl for HostMain {
    fn register_timer(&self, period_ms: u32) -> Result<TimerId, HostError> {
        let id = self.next_timer.get();
        self.next_timer.set(id.wrapping_add(1));
        let period = Duration::from_millis(period_ms.max(10) as u64);
        self.timers.borrow_mut().push(ClapTimer { id, period, next: Instant::now() + period });
        Ok(TimerId(id))
    }

    fn unregister_timer(&self, timer_id: TimerId) -> Result<(), HostError> {
        self.timers.borrow_mut().retain(|t| t.id != timer_id.0);
        Ok(())
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl HostPosixFdImpl for HostMain {
    fn register_fd(&self, fd: i32, flags: FdFlags) -> Result<(), HostError> {
        let mut fds = self.fds.borrow_mut();
        fds.retain(|(f, _)| *f != fd);
        fds.push((fd, flags));
        Ok(())
    }

    fn modify_fd(&self, fd: i32, flags: FdFlags) -> Result<(), HostError> {
        self.register_fd(fd, flags)
    }

    fn unregister_fd(&self, fd: i32) -> Result<(), HostError> {
        self.fds.borrow_mut().retain(|(f, _)| *f != fd);
        Ok(())
    }
}

impl HostHandlers for Host {
    type Shared<'a> = HostShared;
    type MainThread<'a> = HostMain;
    type AudioProcessor<'a> = ();

    fn declare_extensions(builder: &mut HostExtensions<Self>, _shared: &Self::Shared<'_>) {
        builder.register::<HostGui>().register::<HostTimer>();
        #[cfg(all(unix, not(target_os = "macos")))]
        builder.register::<HostPosixFd>();
    }
}

/// Keeps the instance (and the library) alive on the main thread, and
/// runs its editor.
struct ClapMain {
    instance: PluginInstance<Host>,
    _entry: PluginEntry,
    /// The editor, while it is open.
    gui: Option<PluginGui>,
    /// Whether the open editor can be resized.
    resizable: bool,
    /// The shared parameters' ids, for reading their values back.
    param_ids: Vec<ClapId>,
}

impl ClapMain {
    /// Runs the plugin's timers that are due.
    fn run_timers(&mut self) {
        let now = Instant::now();
        let due: Vec<u32> = self.instance.access_handler(|h: &HostMain| {
            let mut timers = h.timers.borrow_mut();
            timers
                .iter_mut()
                .filter(|t| t.next <= now)
                .map(|t| {
                    t.next = now + t.period;
                    t.id
                })
                .collect()
        });
        if due.is_empty() {
            return;
        }
        let handle = self.instance.plugin_handle();
        if let Some(timer) = handle.get_extension::<PluginTimer>() {
            for id in due {
                timer.on_timer(&handle, TimerId(id));
            }
        }
    }

    /// Hands the plugin the file descriptors that are ready (Linux).
    #[cfg(all(unix, not(target_os = "macos")))]
    fn run_fds(&mut self) {
        let fds: Vec<(i32, FdFlags)> = self.instance.access_handler(|h: &HostMain| h.fds.borrow().clone());
        if fds.is_empty() {
            return;
        }
        let mut polls: Vec<libc::pollfd> = fds
            .iter()
            .map(|(fd, flags)| {
                let mut events = 0;
                if flags.contains(FdFlags::READ) {
                    events |= libc::POLLIN;
                }
                if flags.contains(FdFlags::WRITE) {
                    events |= libc::POLLOUT;
                }
                libc::pollfd { fd: *fd, events, revents: 0 }
            })
            .collect();
        // SAFETY: `polls` is a valid array of its length; no waiting.
        let n = unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 0) };
        if n <= 0 {
            return;
        }
        let handle = self.instance.plugin_handle();
        let Some(ext) = handle.get_extension::<PluginPosixFd>() else {
            return;
        };
        for p in polls.iter().filter(|p| p.revents != 0) {
            let mut flags = FdFlags::empty();
            if p.revents & libc::POLLIN != 0 {
                flags |= FdFlags::READ;
            }
            if p.revents & libc::POLLOUT != 0 {
                flags |= FdFlags::WRITE;
            }
            if p.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                flags |= FdFlags::ERROR;
            }
            ext.on_fd(&handle, p.fd, flags);
        }
    }
}

impl MainThread for ClapMain {
    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let handle = self.instance.plugin_handle();
        let state = handle.get_extension::<PluginState>().ok_or("it can't save its sound")?;
        let mut data = Vec::new();
        state.save(&handle, &mut data).map_err(|e| format!("saving its sound failed: {e}"))?;
        Ok(data)
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        let handle = self.instance.plugin_handle();
        let state = handle.get_extension::<PluginState>().ok_or("it can't load a saved sound")?;
        state.load(&handle, &mut &data[..]).map_err(|e| format!("loading the sound failed: {e}"))
    }

    fn load_preset(&mut self, reference: &str) -> Result<(), String> {
        let mut parts = reference.splitn(3, SEP);
        let (kind, path, key) = (parts.next(), parts.next(), parts.next());
        let path = CString::new(path.unwrap_or_default()).map_err(|_| "a bad preset path")?;
        let key = CString::new(key.unwrap_or_default()).map_err(|_| "a bad preset key")?;
        let location = match kind {
            Some("plugin") => Location::Plugin,
            Some("file") => Location::File { path: &path },
            _ => return Err("not one of its presets".into()),
        };
        let handle = self.instance.plugin_handle();
        let ext = handle.get_extension::<PluginPresetLoad>().ok_or("it can't load its own presets")?;
        let key = (!key.is_empty()).then_some(key.as_c_str());
        ext.load_from_location(&handle, location, key).map_err(|_| "it couldn't load that preset".to_string())
    }

    fn param_values(&mut self, out: &mut [f64]) -> bool {
        let handle = self.instance.plugin_handle();
        let Some(ext) = handle.get_extension::<PluginParams>() else {
            return false;
        };
        for (o, id) in out.iter_mut().zip(&self.param_ids) {
            if let Some(v) = ext.get_value(&handle, *id) {
                *o = v;
            }
        }
        true
    }

    fn idle(&mut self) {
        let asked = self.instance.access_shared_handler(|s| s.callback.swap(false, Ordering::AcqRel));
        if asked {
            self.instance.call_on_main_thread_callback();
        }
        self.run_timers();
        #[cfg(all(unix, not(target_os = "macos")))]
        self.run_fds();
    }

    fn next_idle(&self) -> Option<Duration> {
        let now = Instant::now();
        self.instance.access_handler(|h: &HostMain| {
            h.timers.borrow().iter().map(|t| t.next.saturating_duration_since(now)).min()
        })
    }

    fn open_editor(&mut self, parent: ParentWindow, scale: f64) -> Result<EditorSize, String> {
        let handle = self.instance.plugin_handle();
        let gui = handle.get_extension::<PluginGui>().ok_or("it has no window of its own")?;
        let (api, window) = match parent {
            ParentWindow::Cocoa(view) => (GuiApiType::COCOA, Window::from_generic_ptr(GuiApiType::COCOA, view)),
            ParentWindow::Win32(hwnd) => (GuiApiType::WIN32, Window::from_generic_ptr(GuiApiType::WIN32, hwnd)),
            ParentWindow::X11(id) => (GuiApiType::X11, Window::from_x11_handle(id as _)),
        };
        let config = GuiConfiguration { api_type: api, is_floating: false };
        if !gui.is_api_supported(&handle, config) {
            return Err("its window can't be shown inside DJ Sugar's".into());
        }
        gui.create(&handle, config).map_err(|_| "its window failed to open")?;
        if api != GuiApiType::COCOA {
            // Cocoa sizes are in points; elsewhere the plugin scales itself.
            let _ = gui.set_scale(&handle, scale);
        }
        // SAFETY: the window stays until `close_editor` destroys the editor.
        if unsafe { gui.set_parent(&handle, window) }.is_err() {
            gui.destroy(&handle);
            return Err("its window failed to open".into());
        }
        let size = gui.get_size(&handle).unwrap_or(GuiSize { width: 640, height: 400 });
        let _ = gui.show(&handle);
        self.resizable = gui.can_resize(&handle);
        self.gui = Some(gui);
        Ok(EditorSize { width: size.width, height: size.height })
    }

    fn close_editor(&mut self) {
        if let Some(gui) = self.gui.take() {
            let handle = self.instance.plugin_handle();
            let _ = gui.hide(&handle);
            gui.destroy(&handle);
        }
        self.instance.access_shared_handler(|s| {
            s.gui_closed.store(false, Ordering::Release);
            s.resize.lock().unwrap_or_else(|e| e.into_inner()).take();
        });
    }

    fn editor_can_resize(&self) -> bool {
        self.gui.is_some() && self.resizable
    }

    fn editor_resized(&mut self, size: EditorSize) -> EditorSize {
        let Some(gui) = self.gui else {
            return size;
        };
        let handle = self.instance.plugin_handle();
        let asked = GuiSize { width: size.width, height: size.height };
        let took = if gui.can_resize(&handle) {
            let s = gui.adjust_size(&handle, asked).unwrap_or(asked);
            let _ = gui.set_size(&handle, s);
            s
        } else {
            gui.get_size(&handle).unwrap_or(asked)
        };
        EditorSize { width: took.width, height: took.height }
    }

    fn take_editor_resize(&mut self) -> Option<EditorSize> {
        self.instance
            .access_shared_handler(|s| s.resize.lock().unwrap_or_else(|e| e.into_inner()).take())
            .map(|s| EditorSize { width: s.width, height: s.height })
    }

    fn take_editor_closed(&mut self) -> bool {
        self.instance.access_shared_handler(|s| s.gui_closed.swap(false, Ordering::AcqRel))
    }
}

/// Loads plugin `id` from `file` and activates it for blocks of up
/// to `max_frames`. The parameters on the plugin's remote controls
/// pages, page by page, each once; none if it has no pages.
fn remote_controls(handle: &PluginMainThreadHandle) -> Vec<ClapId> {
    let mut ids = Vec::new();
    if let Some(ext) = handle.get_extension::<PluginRemoteControls>() {
        let mut buf = RemoteControlsPageBuffer::new();
        for i in 0..ext.count(handle) {
            if let Some(page) = ext.get(handle, i, &mut buf) {
                for id in page.param_ids.into_iter().flatten() {
                    if !ids.contains(&id) {
                        ids.push(id);
                    }
                }
            }
        }
    }
    ids
}

pub fn load(
    file: &Path,
    id: &str,
    sample_rate: u32,
    max_frames: usize,
    picks: &[(usize, u32)],
) -> Result<Loaded, String> {
    let entry = load_entry(file)?;
    let factory = entry.get_plugin_factory().ok_or("the file has no plugin factory")?;
    let desc = factory
        .plugin_descriptors()
        .find(|d| d.id().is_some_and(|i| i.to_bytes() == id.as_bytes()))
        .ok_or_else(|| format!("{} has no plugin {id:?}", file.display()))?;
    let name = text(desc.name());
    let cid = CString::new(id).map_err(|_| "bad plugin id")?;
    let host = host_info()?;
    let mut instance =
        PluginInstance::<Host>::new(|_| HostShared::default(), |_| HostMain::default(), &entry, &cid, &host)
            .map_err(|e| format!("{name} failed to start: {e}"))?;

    // A stereo output, and a stereo input unless it is an instrument,
    // which has none.
    let handle = instance.plugin_handle();
    let ports = handle.get_extension::<PluginAudioPorts>().ok_or_else(|| format!("{name} has no audio ports"))?;
    let mut buf = AudioPortInfoBuffer::new();
    let has_input = ports.count(&handle, true) >= 1;
    for is_input in [true, false] {
        if is_input && !has_input {
            continue;
        }
        let ok = ports.count(&handle, is_input) >= 1
            && ports.get(&handle, 0, is_input, &mut buf).is_some_and(|p| p.channel_count == 2);
        if !ok {
            return Err(format!("{name} is not a stereo effect or instrument"));
        }
    }

    // Its first parameters, as the slot's parameters: the DJ's picks, then
    // those on its remote controls pages, in the pages' order (what its
    // maker picked for hardware knobs), then the rest in its own order.
    let mut params = Vec::new();
    let mut listed = Vec::new();
    if let Some(ext) = handle.get_extension::<PluginParams>() {
        let mut all = Vec::new();
        let mut buf = ParamInfoBuffer::new();
        for i in 0..ext.count(&handle) {
            let Some(p) = ext.get_info(&handle, i, &mut buf) else {
                continue;
            };
            if p.flags.intersects(ParamInfoFlags::IS_HIDDEN | ParamInfoFlags::IS_READONLY) {
                continue;
            }
            let value = ext.get_value(&handle, p.id).unwrap_or(p.default_value);
            all.push((
                p.id,
                Param {
                    id: p.id.get(),
                    name: String::from_utf8_lossy(p.name).into_owned(),
                    min: p.min_value,
                    max: p.max_value,
                    default: p.default_value,
                    value,
                },
            ));
        }
        let pages: Vec<u32> = remote_controls(&handle).iter().map(|id| id.get()).collect();
        listed = arrange(&mut all, &pages, picks);
        params = all;
    }

    let processor = instance
        .activate(
            |_, _| (),
            PluginAudioConfiguration {
                sample_rate: sample_rate as f64,
                min_frames_count: 1,
                max_frames_count: max_frames as u32,
            },
        )
        .map_err(|e| format!("{name} failed to activate: {e}"))?;
    let handle = instance.plugin_handle();
    let latency = handle.get_extension::<PluginLatency>().map(|l| l.get(&handle)).unwrap_or(0);

    let effect = ClapEffect {
        processor: processor.into(),
        input: [vec![0.0; max_frames], vec![0.0; max_frames]],
        output: [vec![0.0; max_frames], vec![0.0; max_frames]],
        in_ports: AudioPorts::with_capacity(CHANNELS, 1),
        has_input,
        out_ports: AudioPorts::with_capacity(CHANNELS, 1),
        events: EventBuffer::with_capacity(MAX_PARAMS + MAX_EVENTS + 4),
        out_events: EventBuffer::with_capacity(64),
        ids: params.iter().map(|(id, _)| *id).collect(),
        sent: params.iter().map(|(_, p)| p.value as f32).collect(),
        steady: 0,
    };
    Ok(Loaded {
        effect: Box::new(effect),
        main: Some(Box::new(ClapMain {
            instance,
            _entry: entry,
            gui: None,
            resizable: false,
            param_ids: params.iter().map(|(id, _)| *id).collect(),
        })),
        info: Info { name, params: params.into_iter().map(|(_, p)| p).collect(), all: listed, latency },
    })
}

struct ClapEffect {
    processor: PluginAudioProcessor<Host>,
    input: [Vec<f32>; CHANNELS],
    output: [Vec<f32>; CHANNELS],
    in_ports: AudioPorts,
    /// False for an instrument, which takes notes instead of audio.
    has_input: bool,
    out_ports: AudioPorts,
    events: EventBuffer,
    out_events: EventBuffer,
    ids: Vec<ClapId>,
    /// The values last sent to the plugin.
    sent: Vec<f32>,
    steady: u64,
}

fn transport_event(t: &Transport) -> TransportEvent {
    let mut flags = TransportFlags::HAS_TEMPO | TransportFlags::HAS_BEATS_TIMELINE | TransportFlags::HAS_TIME_SIGNATURE;
    if t.playing {
        flags |= TransportFlags::IS_PLAYING;
    }
    let bar = (t.beat / 4.0).floor();
    TransportEvent {
        header: EventHeader::new_core(0, EventFlags::empty()),
        flags,
        song_pos_beats: BeatTime::from_float(t.beat),
        song_pos_seconds: SecondsTime::from_int(0),
        tempo: t.bpm,
        tempo_inc: 0.0,
        loop_start_beats: BeatTime::from_int(0),
        loop_end_beats: BeatTime::from_int(0),
        loop_start_seconds: SecondsTime::from_int(0),
        loop_end_seconds: SecondsTime::from_int(0),
        bar_start: BeatTime::from_float(bar * 4.0),
        bar_number: bar as i32,
        time_signature_numerator: 4,
        time_signature_denominator: 4,
    }
}

impl Effect for ClapEffect {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
        notes: &[Midi],
    ) {
        let frames = input[0].len();
        // A plugin that can't start or fails a block plays dry, as a late
        // block does.
        if self.processor.ensure_processing_started().is_err() {
            for (i, o) in input.iter().zip(output) {
                o.copy_from_slice(i);
            }
            return;
        }
        self.events.clear();
        for (i, id) in self.ids.iter().enumerate() {
            let v = params[i];
            if v != self.sent[i] {
                self.sent[i] = v;
                self.events.push(&ParamValueEvent::new(0, *id, Pckn::match_all(), v as f64));
            }
        }
        // Notes in CLAP's own events, which every instrument takes; other
        // MIDI (controllers, pitch bend) as it is.
        for n in notes {
            let pckn = |ch: u8, key: u8| Pckn::new(0u16, ch as u16, key as u16, Match::All);
            if let Some((ch, key, vel)) = n.as_note_on() {
                self.events.push(&NoteOnEvent::new(n.frame, pckn(ch, key), vel as f64));
            } else if let Some((ch, key)) = n.as_note_off() {
                self.events.push(&NoteOffEvent::new(n.frame, pckn(ch, key), 0.0));
            } else {
                self.events.push(&MidiEvent::new(n.frame, 0, n.data));
            }
        }
        for (ch, data) in input.iter().enumerate() {
            self.input[ch][..frames].copy_from_slice(data);
        }
        let transport = (transport.bpm > 0.0).then(|| transport_event(transport));
        let [in_l, in_r] = &mut self.input;
        let ports = usize::from(self.has_input);
        let inputs = self.in_ports.with_input_buffers(
            [AudioPortBuffer {
                latency: 0,
                channels: AudioPortBufferType::f32_input_only(
                    [&mut in_l[..frames], &mut in_r[..frames]].into_iter().map(InputChannel::variable),
                ),
            }]
            .into_iter()
            .take(ports),
        );
        let [out_l, out_r] = &mut self.output;
        let mut outputs = self.out_ports.with_output_buffers([AudioPortBuffer {
            latency: 0,
            channels: AudioPortBufferType::f32_output_only([&mut out_l[..frames], &mut out_r[..frames]].into_iter()),
        }]);
        self.out_events.clear();
        let ok = match self.processor.as_started_mut() {
            Ok(p) => p
                .process(
                    &inputs,
                    &mut outputs,
                    &InputEvents::from_buffer(&self.events),
                    &mut OutputEvents::from_buffer(&mut self.out_events),
                    Some(self.steady),
                    transport.as_ref(),
                )
                .is_ok(),
            Err(_) => false,
        };
        self.steady += frames as u64;
        for (ch, o) in output.into_iter().enumerate() {
            if ok {
                o.copy_from_slice(&self.output[ch][..frames]);
            } else {
                o.copy_from_slice(input[ch]);
            }
        }
    }
}
