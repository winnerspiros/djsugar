//! VST3 effects inside the helper, through the `vst3` crate's bindings. The component and its edit controller stay on the helper's main thread; the audio processor runs
//! on the helper's audio thread. Parameters are VST3's normalized values (0 to 1); changes go to the processor as parameter queues, and the deck's tempo and beat go in
//! the process context. [`scan_moduleinfo`] reads a bundle's `moduleinfo.json` without loading any plugin code; [`scan`] loads the module (in a throwaway helper).

#![allow(non_upper_case_globals)]
// The bindings' constant types differ between platforms (u32 here, i32 on
// Windows), so some casts are only needed on some of them.
#![allow(clippy::unnecessary_cast)]

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use vst3::ComRef;
use vst3::Steinberg::Vst::ViewType;
use vst3::Steinberg::Vst::{
    AudioBusBuffers, AudioBusBuffers__type0, BusDirections_, Event, Event_::EventTypes_, Event__type0, IAudioProcessor,
    IAudioProcessorTrait, IComponent, IComponentHandler, IComponentHandlerTrait, IComponentTrait, IConnectionPoint,
    IConnectionPointTrait, IEditController, IEditControllerTrait, IEventList, IEventListTrait, IHostApplication,
    IHostApplicationTrait, IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait,
    MediaTypes_, NoteOffEvent, NoteOnEvent, ParamID, ParamValue, ParameterInfo, ParameterInfo_::ParameterFlags_,
    ProcessContext, ProcessContext_::StatesAndFlags_, ProcessData, ProcessModes_, ProcessSetup, SpeakerArr,
    SpeakerArrangement, String128, SymbolicSampleSizes_,
};
use vst3::Steinberg::{
    kInvalidArgument, kResultFalse, kResultOk, kResultTrue, tresult, FUnknown, IBStream, IBStreamTrait,
    IBStream_::IStreamSeekMode_, IPluginBaseTrait, IPluginFactory, IPluginFactory2, IPluginFactory2Trait,
    IPluginFactoryTrait, PClassInfo, PClassInfo2, TUID,
};
use vst3::Steinberg::{
    kPlatformTypeHWND, kPlatformTypeNSView, kPlatformTypeX11EmbedWindowID, IPlugFrame, IPlugFrameTrait, IPlugView,
    IPlugViewTrait, ViewRect,
};
use vst3::{Class, ComPtr, ComWrapper, Interface};

use crate::client::Transport;
use crate::descriptor::{features, Descriptor, AUDIO_MODULE};
use crate::effects::{arrange, EditorSize, Effect, Info, Loaded, MainThread, Param, ParentWindow};
use crate::presets::{pack_states, unpack_states};
use crate::shm::{Midi, CHANNELS, MAX_EVENTS, MAX_PARAMS};

fn cstr(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn wstr(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn copy_wstring(src: &str, dst: &mut [u16]) {
    let mut n = 0;
    for (s, d) in src.encode_utf16().zip(dst.iter_mut()) {
        *d = s;
        n += 1;
    }
    if let Some(end) = dst.get_mut(n.min(dst.len().saturating_sub(1))) {
        *end = 0;
    }
}

/// A class id as the 32 hex digits VST3 tools show.
pub fn cid_to_hex(cid: &TUID) -> String {
    cid.iter().map(|&b| format!("{:02X}", b as u8)).collect()
}

fn hex_to_cid(hex: &str) -> Option<TUID> {
    if hex.len() != 32 {
        return None;
    }
    let mut cid = [0 as c_char; 16];
    for (i, c) in cid.iter_mut().enumerate() {
        *c = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()? as c_char;
    }
    Some(cid)
}

/// The library inside a `.vst3` bundle, for this machine. A plain file is
/// taken as the library itself.
pub fn library_path(bundle: &Path) -> PathBuf {
    if !bundle.is_dir() {
        return bundle.to_path_buf();
    }
    let stem = bundle.file_stem().unwrap_or_default();
    let contents = bundle.join("Contents");
    if cfg!(target_os = "macos") {
        contents.join("MacOS").join(stem)
    } else if cfg!(windows) {
        let arch = if cfg!(target_arch = "aarch64") { "arm64-win" } else { "x86_64-win" };
        contents.join(arch).join(bundle.file_name().unwrap_or_default())
    } else {
        let arch = format!("{}-linux", std::env::consts::ARCH);
        let mut name = stem.to_os_string();
        name.push(".so");
        contents.join(arch).join(name)
    }
}

/// A loaded module: the library and its factory. The library must outlive
/// every object the factory made, so this is kept until the helper exits.
struct Module {
    factory: ComPtr<IPluginFactory>,
    _lib: libloading::Library,
}

#[cfg(target_os = "macos")]
mod cf {
    use std::ffi::c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        pub fn CFURLCreateFromFileSystemRepresentation(
            alloc: *const c_void,
            buf: *const u8,
            len: isize,
            is_dir: u8,
        ) -> *const c_void;
        pub fn CFBundleCreate(alloc: *const c_void, url: *const c_void) -> *mut c_void;
    }
}

fn load_module(bundle: &Path) -> Result<Module, String> {
    let lib_path = library_path(bundle);
    let fail = |e: libloading::Error| format!("can't load {}: {e}", lib_path.display());
    // SAFETY: loading a plugin runs its code; this only happens in a
    // helper process that DJ Sugar can lose without harm.
    #[cfg(target_os = "linux")]
    let (lib, handle) = unsafe {
        let unix = libloading::os::unix::Library::new(&lib_path).map_err(fail)?;
        let handle = unix.into_raw();
        let lib: libloading::Library = libloading::os::unix::Library::from_raw(handle).into();
        (lib, handle)
    };
    #[cfg(not(target_os = "linux"))]
    let lib = unsafe { libloading::Library::new(&lib_path) }.map_err(fail)?;
    // SAFETY: the entry points have the signatures the VST3 SDK defines.
    unsafe {
        #[cfg(target_os = "linux")]
        if let Ok(entry) = lib.get::<unsafe extern "system" fn(*mut c_void) -> bool>(b"ModuleEntry\0") {
            // The library's handle; plugins use it to find their bundle.
            if !entry(handle) {
                return Err("the plugin's ModuleEntry failed".into());
            }
        }
        #[cfg(windows)]
        if let Ok(entry) = lib.get::<unsafe extern "system" fn() -> bool>(b"InitDll\0") {
            if !entry() {
                return Err("the plugin's InitDll failed".into());
            }
        }
        #[cfg(target_os = "macos")]
        if let Ok(entry) = lib.get::<unsafe extern "system" fn(*mut c_void) -> bool>(b"bundleEntry\0") {
            use std::os::unix::ffi::OsStrExt;
            let p = bundle.as_os_str().as_bytes();
            let url = cf::CFURLCreateFromFileSystemRepresentation(ptr::null(), p.as_ptr(), p.len() as isize, 1);
            let b = cf::CFBundleCreate(ptr::null(), url);
            if !entry(b) {
                return Err("the plugin's bundleEntry failed".into());
            }
        }
        let get = lib
            .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0")
            .map_err(|_| "not a VST3 plugin (no GetPluginFactory)".to_string())?;
        let factory = ComPtr::from_raw(get()).ok_or("the plugin has no factory")?;
        Ok(Module { factory, _lib: lib })
    }
}

/// Lists the audio processors in a bundle by loading it (in a throwaway
/// helper, `djsugar-plugin-host --scan`).
pub fn scan(bundle: &Path) -> Result<Vec<Descriptor>, String> {
    let module = load_module(bundle)?;
    let f = &module.factory;
    let f2 = f.cast::<IPluginFactory2>();
    let mut found = Vec::new();
    // SAFETY: plain calls into the factory with buffers of the right size.
    unsafe {
        for i in 0..f.countClasses() {
            if let Some(f2) = &f2 {
                let mut info: PClassInfo2 = std::mem::zeroed();
                if f2.getClassInfo2(i, &mut info) == kResultOk && cstr(&info.category) == AUDIO_MODULE {
                    found.push(Descriptor {
                        id: cid_to_hex(&info.cid),
                        name: cstr(&info.name),
                        vendor: cstr(&info.vendor),
                        version: cstr(&info.version),
                        features: features(&cstr(&info.subCategories)),
                    });
                }
                continue;
            }
            let mut info: PClassInfo = std::mem::zeroed();
            if f.getClassInfo(i, &mut info) == kResultOk && cstr(&info.category) == AUDIO_MODULE {
                found.push(Descriptor {
                    id: cid_to_hex(&info.cid),
                    name: cstr(&info.name),
                    vendor: String::new(),
                    version: String::new(),
                    features: Vec::new(),
                });
            }
        }
    }
    Ok(found)
}

// ---- What DJ Sugar offers the plugin --------------------------------------

struct HostApp;

impl Class for HostApp {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut String128) -> tresult {
        copy_wstring("DJ Sugar", &mut *name);
        kResultOk
    }

    unsafe fn createInstance(&self, _cid: *mut TUID, _iid: *mut TUID, obj: *mut *mut c_void) -> tresult {
        *obj = ptr::null_mut();
        kResultFalse
    }
}

/// Edits made in the plugin's own editor, on their way to its processor.
type Edits = Arc<Mutex<Vec<(ParamID, ParamValue)>>>;

/// Takes the edits made in the plugin's own editor: a plugin with a
/// separate controller only hears of them through the host.
struct Handler {
    edits: Edits,
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn performEdit(&self, id: ParamID, value: ParamValue) -> tresult {
        let mut edits = self.edits.lock().unwrap_or_else(|e| e.into_inner());
        edits.retain(|(i, _)| *i != id);
        // Bounded: the audio thread takes them every block.
        if edits.len() < EDITS {
            edits.push((id, value));
        }
        kResultOk
    }
    unsafe fn endEdit(&self, _id: ParamID) -> tresult {
        kResultOk
    }
    unsafe fn restartComponent(&self, _flags: i32) -> tresult {
        kResultOk
    }
}

/// A memory stream, for handing the component's state to its controller.
struct Stream {
    data: RefCell<Vec<u8>>,
    pos: Cell<usize>,
}

impl Class for Stream {
    type Interfaces = (IBStream,);
}

impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, n: i32, read: *mut i32) -> tresult {
        let data = self.data.borrow();
        let pos = self.pos.get().min(data.len());
        let n = (n.max(0) as usize).min(data.len() - pos);
        ptr::copy_nonoverlapping(data.as_ptr().add(pos), buffer as *mut u8, n);
        self.pos.set(pos + n);
        if !read.is_null() {
            *read = n as i32;
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, n: i32, written: *mut i32) -> tresult {
        let n = n.max(0) as usize;
        let mut data = self.data.borrow_mut();
        let pos = self.pos.get();
        if data.len() < pos + n {
            data.resize(pos + n, 0);
        }
        ptr::copy_nonoverlapping(buffer as *const u8, data.as_mut_ptr().add(pos), n);
        self.pos.set(pos + n);
        if !written.is_null() {
            *written = n as i32;
        }
        kResultOk
    }

    unsafe fn seek(&self, pos: i64, mode: i32, result: *mut i64) -> tresult {
        let len = self.data.borrow().len() as i64;
        let base = match mode as vst3::Steinberg::IBStream_::IStreamSeekMode {
            IStreamSeekMode_::kIBSeekSet => 0,
            IStreamSeekMode_::kIBSeekCur => self.pos.get() as i64,
            IStreamSeekMode_::kIBSeekEnd => len,
            _ => return kInvalidArgument,
        };
        let p = (base + pos).clamp(0, len);
        self.pos.set(p as usize);
        if !result.is_null() {
            *result = p;
        }
        kResultOk
    }

    unsafe fn tell(&self, pos: *mut i64) -> tresult {
        if !pos.is_null() {
            *pos = self.pos.get() as i64;
        }
        kResultOk
    }
}

/// One parameter's change in a block: a single point at the start.
struct Queue {
    id: Cell<ParamID>,
    value: Cell<f64>,
    points: Cell<i32>,
}

impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> ParamID {
        self.id.get()
    }
    unsafe fn getPointCount(&self) -> i32 {
        self.points.get()
    }
    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut ParamValue) -> tresult {
        if index != 0 || self.points.get() == 0 {
            return kInvalidArgument;
        }
        *offset = 0;
        *value = self.value.get();
        kResultOk
    }
    unsafe fn addPoint(&self, _offset: i32, value: ParamValue, index: *mut i32) -> tresult {
        // Output changes from the plugin: keep the last, DJ Sugar ignores them for now.
        self.value.set(value);
        self.points.set(1);
        if !index.is_null() {
            *index = 0;
        }
        kResultOk
    }
}

/// A block's parameter changes, from a pool made up front so the audio
/// thread never allocates.
struct Changes {
    queues: Vec<ComWrapper<Queue>>,
    ptrs: Vec<*mut IParamValueQueue>,
    count: Cell<usize>,
}

// The pool is only touched by the thread that owns the effect.
unsafe impl Send for Changes {}
unsafe impl Sync for Changes {}

impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}

impl Changes {
    fn new(cap: usize) -> Self {
        let queues: Vec<ComWrapper<Queue>> = (0..cap)
            .map(|_| ComWrapper::new(Queue { id: Cell::new(0), value: Cell::new(0.0), points: Cell::new(0) }))
            .collect();
        let ptrs = queues.iter().map(|q| q.as_com_ref::<IParamValueQueue>().unwrap().as_ptr()).collect();
        Self { queues, ptrs, count: Cell::new(0) }
    }

    fn clear(&self) {
        self.count.set(0);
    }

    fn push(&self, id: ParamID, value: f64) -> Option<usize> {
        let i = self.count.get();
        let q = self.queues.get(i)?;
        q.id.set(id);
        q.value.set(value);
        q.points.set(1);
        self.count.set(i + 1);
        Some(i)
    }
}

impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        self.count.get() as i32
    }
    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index >= 0 && (index as usize) < self.count.get() {
            self.ptrs[index as usize]
        } else {
            ptr::null_mut()
        }
    }
    unsafe fn addParameterData(&self, id: *const ParamID, index: *mut i32) -> *mut IParamValueQueue {
        match self.push(*id, 0.0) {
            Some(i) => {
                self.queues[i].points.set(0);
                if !index.is_null() {
                    *index = i as i32;
                }
                self.ptrs[i]
            }
            None => ptr::null_mut(),
        }
    }
}

/// A block's notes, from a list made up front so the audio thread never
/// allocates.
struct Events {
    list: RefCell<Vec<Event>>,
}

unsafe impl Send for Events {}
unsafe impl Sync for Events {}

impl Class for Events {
    type Interfaces = (IEventList,);
}

impl Events {
    fn new() -> Self {
        Self { list: RefCell::new(Vec::with_capacity(MAX_EVENTS)) }
    }

    /// Notes become VST3 note events; other MIDI isn't sent (VST3 takes
    /// controllers as parameters).
    fn set(&self, notes: &[Midi]) {
        let mut list = self.list.borrow_mut();
        list.clear();
        for n in notes.iter().take(MAX_EVENTS) {
            // SAFETY: an all-zero Event is a valid (empty) event.
            let mut e: Event = unsafe { std::mem::zeroed() };
            e.sampleOffset = n.frame as i32;
            if let Some((ch, key, vel)) = n.as_note_on() {
                e.r#type = EventTypes_::kNoteOnEvent as u16;
                e.__field0 = Event__type0 {
                    noteOn: NoteOnEvent {
                        channel: ch as i16,
                        pitch: key as i16,
                        tuning: 0.0,
                        velocity: vel,
                        length: 0,
                        noteId: -1,
                    },
                };
            } else if let Some((ch, key)) = n.as_note_off() {
                e.r#type = EventTypes_::kNoteOffEvent as u16;
                e.__field0 = Event__type0 {
                    noteOff: NoteOffEvent {
                        channel: ch as i16,
                        pitch: key as i16,
                        velocity: 0.0,
                        noteId: -1,
                        tuning: 0.0,
                    },
                };
            } else {
                continue;
            }
            list.push(e);
        }
    }
}

impl IEventListTrait for Events {
    unsafe fn getEventCount(&self) -> i32 {
        self.list.borrow().len() as i32
    }
    unsafe fn getEvent(&self, index: i32, e: *mut Event) -> tresult {
        match self.list.borrow().get(index as usize) {
            Some(ev) if !e.is_null() => {
                *e = *ev;
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }
    unsafe fn addEvent(&self, _e: *mut Event) -> tresult {
        // Output events from the plugin: DJ Sugar doesn't take them.
        kResultFalse
    }
}

// ---- Loading and processing ----------------------------------------------

/// Edits from the editor that one block can carry, beyond the slot's own.
const EDITS: usize = 64;

/// The window the plugin's view sits in, as the plugin sees it: it asks
/// for a new size through it, and on Linux for its timers and file events.
struct Frame {
    resize: Cell<Option<EditorSize>>,
    #[cfg(all(unix, not(target_os = "macos")))]
    run_loop: run_loop::RunLoop,
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
impl Class for Frame {
    type Interfaces = (IPlugFrame,);
}

#[cfg(all(unix, not(target_os = "macos")))]
impl Class for Frame {
    type Interfaces = (IPlugFrame, vst3::Steinberg::Linux::IRunLoop);
}

impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, _view: *mut IPlugView, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        let r = &*size;
        self.resize.set(Some(EditorSize {
            width: (r.right - r.left).max(1) as u32,
            height: (r.bottom - r.top).max(1) as u32,
        }));
        kResultOk
    }
}

/// Linux has no main loop every plugin can use, so the host runs the
/// plugin's timers and watches its file descriptors (its X11 connection).
#[cfg(all(unix, not(target_os = "macos")))]
mod run_loop {
    use super::*;
    use std::time::{Duration, Instant};
    use vst3::Steinberg::Linux::{
        FileDescriptor, IEventHandler, IEventHandlerTrait, IRunLoopTrait, ITimerHandler, ITimerHandlerTrait,
        TimerInterval,
    };

    #[derive(Default)]
    pub struct RunLoop {
        handlers: RefCell<Vec<(ComPtr<IEventHandler>, FileDescriptor)>>,
        timers: RefCell<Vec<(ComPtr<ITimerHandler>, Duration, Instant)>>,
    }

    impl RunLoop {
        /// Runs the timers that are due and the handlers whose descriptors
        /// are ready.
        pub fn run(&self) {
            let now = Instant::now();
            let due: Vec<ComPtr<ITimerHandler>> = self
                .timers
                .borrow_mut()
                .iter_mut()
                .filter(|(_, _, next)| *next <= now)
                .map(|(t, period, next)| {
                    *next = now + *period;
                    t.clone()
                })
                .collect();
            for t in due {
                // SAFETY: a handler the plugin registered and hasn't removed.
                unsafe { t.onTimer() };
            }
            let handlers = self.handlers.borrow().clone();
            if handlers.is_empty() {
                return;
            }
            let mut polls: Vec<libc::pollfd> =
                handlers.iter().map(|(_, fd)| libc::pollfd { fd: *fd, events: libc::POLLIN, revents: 0 }).collect();
            // SAFETY: a valid array of its length; no waiting.
            if unsafe { libc::poll(polls.as_mut_ptr(), polls.len() as libc::nfds_t, 0) } <= 0 {
                return;
            }
            for ((h, fd), p) in handlers.iter().zip(&polls) {
                if p.revents != 0 {
                    // SAFETY: as for timers.
                    unsafe { h.onFDIsSet(*fd) };
                }
            }
        }

        pub fn next(&self) -> Option<Duration> {
            let now = Instant::now();
            self.timers.borrow().iter().map(|(_, _, next)| next.saturating_duration_since(now)).min()
        }
    }

    impl IRunLoopTrait for super::Frame {
        unsafe fn registerEventHandler(&self, handler: *mut IEventHandler, fd: FileDescriptor) -> tresult {
            let Some(h) = ComRef::from_raw(handler) else {
                return kInvalidArgument;
            };
            self.run_loop.handlers.borrow_mut().push((h.to_com_ptr(), fd));
            kResultOk
        }

        unsafe fn unregisterEventHandler(&self, handler: *mut IEventHandler) -> tresult {
            self.run_loop.handlers.borrow_mut().retain(|(h, _)| h.as_ptr() != handler);
            kResultOk
        }

        unsafe fn registerTimer(&self, handler: *mut ITimerHandler, milliseconds: TimerInterval) -> tresult {
            let Some(h) = ComRef::from_raw(handler) else {
                return kInvalidArgument;
            };
            let period = Duration::from_millis(milliseconds.max(10));
            self.run_loop.timers.borrow_mut().push((h.to_com_ptr(), period, Instant::now() + period));
            kResultOk
        }

        unsafe fn unregisterTimer(&self, handler: *mut ITimerHandler) -> tresult {
            self.run_loop.timers.borrow_mut().retain(|(h, _, _)| h.as_ptr() != handler);
            kResultOk
        }
    }
}

/// Keeps the module, component and controller alive on the main thread,
/// and runs the editor.
struct Vst3Main {
    component: ComPtr<IComponent>,
    controller: Option<ComPtr<IEditController>>,
    /// The controller is its own object, so it is told the component's
    /// state when that changes.
    separate: bool,
    _host: ComWrapper<HostApp>,
    _handler: ComWrapper<Handler>,
    frame: ComWrapper<Frame>,
    /// The editor, while it is open, and whether it can be resized.
    view: Option<(ComPtr<IPlugView>, bool)>,
    /// The shared parameters' ids, for reading their values back.
    param_ids: Vec<ParamID>,
    _module: Module,
}

impl Vst3Main {
    fn stream(data: Vec<u8>) -> ComWrapper<Stream> {
        ComWrapper::new(Stream { data: RefCell::new(data), pos: Cell::new(0) })
    }
}

/// A VST3 sound as saved: the component's state, then the controller's,
/// each after its length (little-endian `u32`).
impl MainThread for Vst3Main {
    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        let component = Self::stream(Vec::new());
        // SAFETY: COM calls on live objects, on the main thread.
        unsafe {
            let s = component.as_com_ref::<IBStream>().unwrap().as_ptr();
            if !ok(self.component.getState(s)) {
                return Err("it can't save its sound".into());
            }
        }
        let controller = Self::stream(Vec::new());
        if let Some(c) = &self.controller {
            // SAFETY: as above.
            unsafe {
                let s = controller.as_com_ref::<IBStream>().unwrap().as_ptr();
                c.getState(s);
            }
        }
        let component = component.data.borrow();
        let controller = controller.data.borrow();
        Ok(pack_states(&component, &controller))
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), String> {
        let (component, controller) = unpack_states(data).ok_or("the saved sound is damaged")?;
        let stream = Self::stream(component.to_vec());
        // SAFETY: COM calls on live objects, on the main thread.
        unsafe {
            let s = stream.as_com_ref::<IBStream>().unwrap().as_ptr();
            if !ok(self.component.setState(s)) {
                return Err("it didn't take the saved sound".into());
            }
            if let Some(c) = &self.controller {
                if self.separate {
                    stream.pos.set(0);
                    c.setComponentState(s);
                }
                let stream = Self::stream(controller.to_vec());
                let s = stream.as_com_ref::<IBStream>().unwrap().as_ptr();
                c.setState(s);
            }
        }
        Ok(())
    }

    fn param_values(&mut self, out: &mut [f64]) -> bool {
        let Some(c) = &self.controller else {
            return false;
        };
        for (o, id) in out.iter_mut().zip(&self.param_ids) {
            // SAFETY: a plain call into the controller, on the main thread.
            *o = unsafe { c.getParamNormalized(*id) };
        }
        true
    }

    fn param_set(&mut self, index: usize, value: f64) {
        // The processor got the change; the controller (and its window)
        // keeps its own copy, so it is told too.
        if let (Some(c), Some(id)) = (&self.controller, self.param_ids.get(index)) {
            // SAFETY: as above.
            unsafe {
                c.setParamNormalized(*id, value);
            }
        }
    }

    fn idle(&mut self) {
        #[cfg(all(unix, not(target_os = "macos")))]
        self.frame.run_loop.run();
    }

    fn next_idle(&self) -> Option<std::time::Duration> {
        #[cfg(all(unix, not(target_os = "macos")))]
        return self.frame.run_loop.next();
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        None
    }

    fn open_editor(&mut self, parent: ParentWindow, _scale: f64) -> Result<EditorSize, String> {
        let controller = self.controller.as_ref().ok_or("it has no window of its own")?;
        let (ptr, kind) = match parent {
            ParentWindow::Cocoa(view) => (view, kPlatformTypeNSView),
            ParentWindow::Win32(hwnd) => (hwnd, kPlatformTypeHWND),
            ParentWindow::X11(id) => (id as usize as *mut c_void, kPlatformTypeX11EmbedWindowID),
        };
        // SAFETY: plain calls into the controller and its view, on the main
        // thread; the parent window outlives the view (see close_editor).
        unsafe {
            let view =
                ComPtr::from_raw(controller.createView(ViewType::kEditor)).ok_or("it has no window of its own")?;
            if view.isPlatformTypeSupported(kind) != kResultTrue {
                return Err("its window can't be shown inside DJ Sugar's".into());
            }
            let frame = self.frame.as_com_ref::<IPlugFrame>().ok_or("no frame")?.as_ptr();
            view.setFrame(frame);
            if !ok(view.attached(ptr, kind)) {
                view.setFrame(ptr::null_mut());
                return Err("its window failed to open".into());
            }
            let mut r: ViewRect = std::mem::zeroed();
            let size = if ok(view.getSize(&mut r)) && r.right > r.left && r.bottom > r.top {
                EditorSize { width: (r.right - r.left) as u32, height: (r.bottom - r.top) as u32 }
            } else {
                EditorSize { width: 640, height: 400 }
            };
            let resizable = view.canResize() == kResultTrue;
            self.view = Some((view, resizable));
            self.frame.resize.set(None);
            Ok(size)
        }
    }

    fn close_editor(&mut self) {
        if let Some((view, _)) = self.view.take() {
            // SAFETY: the view was attached in open_editor.
            unsafe {
                view.removed();
                view.setFrame(ptr::null_mut());
            }
        }
        self.frame.resize.set(None);
    }

    fn editor_can_resize(&self) -> bool {
        self.view.as_ref().is_some_and(|(_, r)| *r)
    }

    fn editor_placed(&mut self) {
        let Some((view, _)) = &self.view else {
            return;
        };
        // SAFETY: plain calls into the open view. Telling it its own size
        // makes it lay itself out in the window as it now is.
        unsafe {
            let mut r: ViewRect = std::mem::zeroed();
            if ok(view.getSize(&mut r)) && r.right > r.left && r.bottom > r.top {
                view.onSize(&mut r);
            }
        }
    }

    fn editor_resized(&mut self, size: EditorSize) -> EditorSize {
        let Some((view, resizable)) = &self.view else {
            return size;
        };
        // SAFETY: plain calls into the open view.
        unsafe {
            let mut r = ViewRect { left: 0, top: 0, right: size.width as i32, bottom: size.height as i32 };
            if *resizable {
                view.checkSizeConstraint(&mut r);
                view.onSize(&mut r);
            } else {
                view.getSize(&mut r);
            }
            EditorSize { width: (r.right - r.left).max(1) as u32, height: (r.bottom - r.top).max(1) as u32 }
        }
    }

    fn take_editor_resize(&mut self) -> Option<EditorSize> {
        let size = self.frame.resize.take()?;
        if let Some((view, _)) = &self.view {
            let mut r = ViewRect { left: 0, top: 0, right: size.width as i32, bottom: size.height as i32 };
            // SAFETY: the plugin asked for this size; it is told it has it.
            unsafe { view.onSize(&mut r) };
        }
        Some(size)
    }
}

fn ok(r: tresult) -> bool {
    r == kResultOk || r == kResultTrue
}

/// Loads class `id` (32 hex digits) from `bundle` and gets it ready for
/// blocks of up to `max_frames`.
pub fn load(
    bundle: &Path,
    id: &str,
    sample_rate: u32,
    max_frames: usize,
    picks: &[(usize, u32)],
) -> Result<Loaded, String> {
    let module = load_module(bundle)?;
    let cid = hex_to_cid(id).ok_or_else(|| format!("bad VST3 class id {id:?}"))?;
    let descriptors = scan(bundle).unwrap_or_default();
    let name = descriptors
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(id))
        .map(|d| d.name.clone())
        .ok_or_else(|| format!("{} has no plugin {id:?}", bundle.display()))?;
    let host = ComWrapper::new(HostApp);
    let host_ptr = host.as_com_ref::<IHostApplication>().unwrap().as_ptr() as *mut FUnknown;
    let edits: Edits = Arc::new(Mutex::new(Vec::with_capacity(EDITS)));
    let handler = ComWrapper::new(Handler { edits: edits.clone() });

    // SAFETY: calls through the plugin's COM interfaces, following the
    // order the VST3 SDK documents for hosts.
    unsafe {
        let mut obj: *mut c_void = ptr::null_mut();
        let r = module.factory.createInstance(cid.as_ptr(), IComponent::IID.as_ptr() as *const c_char, &mut obj);
        let component = ComPtr::from_raw(obj as *mut IComponent)
            .filter(|_| ok(r))
            .ok_or_else(|| format!("{name} failed to start"))?;
        if !ok(component.initialize(host_ptr)) {
            return Err(format!("{name} failed to initialize"));
        }
        let processor = component.cast::<IAudioProcessor>().ok_or_else(|| format!("{name} has no audio processor"))?;

        // The edit controller: the component itself, or a separate class.
        let mut controller = component.cast::<IEditController>();
        let mut separate = false;
        if controller.is_none() {
            let mut ccid: TUID = [0; 16];
            if ok(component.getControllerClassId(&mut ccid)) {
                let mut obj: *mut c_void = ptr::null_mut();
                let r = module.factory.createInstance(
                    ccid.as_ptr(),
                    IEditController::IID.as_ptr() as *const c_char,
                    &mut obj,
                );
                if let Some(c) = ComPtr::from_raw(obj as *mut IEditController).filter(|_| ok(r)) {
                    if ok(c.initialize(host_ptr)) {
                        controller = Some(c);
                        separate = true;
                    }
                }
            }
        }
        if let Some(c) = &controller {
            if separate {
                if let (Some(a), Some(b)) = (component.cast::<IConnectionPoint>(), c.cast::<IConnectionPoint>()) {
                    a.connect(b.as_ptr());
                    b.connect(a.as_ptr());
                }
                // The controller starts from the component's state.
                let stream = ComWrapper::new(Stream { data: RefCell::new(Vec::new()), pos: Cell::new(0) });
                let s = stream.as_com_ref::<IBStream>().unwrap().as_ptr();
                if ok(component.getState(s)) {
                    stream.pos.set(0);
                    c.setComponentState(s);
                }
            }
            c.setComponentHandler(handler.as_com_ref::<IComponentHandler>().unwrap().as_ptr());
        }

        // Stereo in, stereo out.
        let audio = MediaTypes_::kAudio as i32;
        let (inp, out) = (BusDirections_::kInput as i32, BusDirections_::kOutput as i32);
        // An instrument has no audio input; it takes notes on an event bus.
        let has_input = component.getBusCount(audio, inp) >= 1;
        if component.getBusCount(audio, out) < 1 {
            return Err(format!("{name} has no audio output"));
        }
        let mut ins: [SpeakerArrangement; 1] = [SpeakerArr::kStereo];
        let mut outs: [SpeakerArrangement; 1] = [SpeakerArr::kStereo];
        let n_in = i32::from(has_input);
        if !ok(processor.setBusArrangements(ins.as_mut_ptr(), n_in, outs.as_mut_ptr(), 1)) {
            let mut a: SpeakerArrangement = SpeakerArr::kStereo;
            let mut b: SpeakerArrangement = 0;
            let stereo = (!has_input || ok(processor.getBusArrangement(inp, 0, &mut a)))
                && ok(processor.getBusArrangement(out, 0, &mut b))
                && a == SpeakerArr::kStereo
                && b == SpeakerArr::kStereo;
            if !stereo {
                return Err(format!("{name} is not a stereo effect or instrument"));
            }
        }
        if has_input {
            component.activateBus(audio, inp, 0, 1);
        }
        component.activateBus(audio, out, 0, 1);
        let event = MediaTypes_::kEvent as i32;
        if component.getBusCount(event, inp) >= 1 {
            component.activateBus(event, inp, 0, 1);
        }
        if processor.canProcessSampleSize(SymbolicSampleSizes_::kSample32 as i32) != kResultTrue {
            return Err(format!("{name} can't process 32-bit audio"));
        }
        let mut setup = ProcessSetup {
            processMode: ProcessModes_::kRealtime as i32,
            symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
            maxSamplesPerBlock: max_frames as i32,
            sampleRate: sample_rate as f64,
        };
        if !ok(processor.setupProcessing(&mut setup)) {
            return Err(format!("{name} refused {sample_rate} Hz"));
        }
        if !ok(component.setActive(1)) {
            return Err(format!("{name} failed to activate"));
        }
        let latency = processor.getLatencySamples();

        // The parameters a DJ could turn: the DJ's picks first, then the
        // plugin's own order.
        let mut params = Vec::new();
        if let Some(c) = &controller {
            for i in 0..c.getParameterCount() {
                let mut p: ParameterInfo = std::mem::zeroed();
                if !ok(c.getParameterInfo(i, &mut p)) {
                    continue;
                }
                let skip =
                    ParameterFlags_::kIsReadOnly | ParameterFlags_::kIsHidden | ParameterFlags_::kIsProgramChange;
                if p.flags & skip != 0 {
                    continue;
                }
                let value = c.getParamNormalized(p.id);
                params.push((
                    p.id,
                    Param {
                        id: p.id,
                        name: wstr(&p.title),
                        min: 0.0,
                        max: 1.0,
                        default: p.defaultNormalizedValue,
                        value,
                    },
                ));
            }
        }

        let listed = arrange(&mut params, &[], picks);

        let effect = Vst3Effect {
            processor: Send2(processor),
            processing: false,
            input: [vec![0.0; max_frames], vec![0.0; max_frames]],
            output: [vec![0.0; max_frames], vec![0.0; max_frames]],
            changes: ComWrapper::new(Changes::new(MAX_PARAMS + EDITS)),
            edits: edits.clone(),
            out_changes: ComWrapper::new(Changes::new(64)),
            events: ComWrapper::new(Events::new()),
            has_input,
            ids: params.iter().map(|(id, _)| *id).collect(),
            sent: params.iter().map(|(_, p)| p.value as f32).collect(),
            steady: 0,
            sample_rate: sample_rate as f64,
        };
        Ok(Loaded {
            effect: Box::new(effect),
            main: Some(Box::new(Vst3Main {
                component,
                controller,
                separate,
                _host: host,
                _handler: handler,
                frame: ComWrapper::new(Frame {
                    resize: Cell::new(None),
                    #[cfg(all(unix, not(target_os = "macos")))]
                    run_loop: Default::default(),
                }),
                view: None,
                param_ids: params.iter().map(|(id, _)| *id).collect(),
                _module: module,
            })),
            info: Info { name, params: params.into_iter().map(|(_, p)| p).collect(), all: listed, latency },
        })
    }
}

/// The processor moves to the audio thread once and is only used there.
struct Send2<T>(T);
unsafe impl<T> Send for Send2<T> {}

struct Vst3Effect {
    processor: Send2<ComPtr<IAudioProcessor>>,
    processing: bool,
    input: [Vec<f32>; CHANNELS],
    output: [Vec<f32>; CHANNELS],
    changes: ComWrapper<Changes>,
    out_changes: ComWrapper<Changes>,
    events: ComWrapper<Events>,
    /// Edits from the plugin's editor, for the processor.
    edits: Edits,
    /// False for an instrument.
    has_input: bool,
    ids: Vec<ParamID>,
    sent: Vec<f32>,
    steady: i64,
    sample_rate: f64,
}

// ComWrappers here are used only on the audio thread after the move.
unsafe impl Send for Vst3Effect {}

impl Effect for Vst3Effect {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
        notes: &[Midi],
    ) {
        let frames = input[0].len();
        let p = &self.processor.0;
        // SAFETY: the processor is set up for blocks of this size; every
        // buffer below outlives the call.
        let ok = unsafe {
            if !self.processing {
                p.setProcessing(1);
                self.processing = true;
            }
            self.changes.clear();
            for (i, id) in self.ids.iter().enumerate() {
                if params[i] != self.sent[i] {
                    self.sent[i] = params[i];
                    self.changes.push(*id, params[i] as f64);
                }
            }
            // Never waits: the editor's edits come next block if the main
            // thread holds them now.
            if let Ok(mut edits) = self.edits.try_lock() {
                for (id, value) in edits.drain(..) {
                    self.changes.push(id, value);
                }
            }
            self.out_changes.clear();
            self.events.set(notes);
            for (ch, data) in input.iter().enumerate() {
                self.input[ch][..frames].copy_from_slice(data);
            }
            let mut in_ptrs = [self.input[0].as_mut_ptr(), self.input[1].as_mut_ptr()];
            let mut out_ptrs = [self.output[0].as_mut_ptr(), self.output[1].as_mut_ptr()];
            let mut ins = AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 { channelBuffers32: in_ptrs.as_mut_ptr() },
            };
            let mut outs = AudioBusBuffers {
                numChannels: 2,
                silenceFlags: 0,
                __field0: AudioBusBuffers__type0 { channelBuffers32: out_ptrs.as_mut_ptr() },
            };
            let mut ctx: ProcessContext = std::mem::zeroed();
            ctx.sampleRate = self.sample_rate;
            ctx.projectTimeSamples = self.steady;
            ctx.continousTimeSamples = self.steady;
            if transport.bpm > 0.0 {
                let mut state = StatesAndFlags_::kTempoValid
                    | StatesAndFlags_::kProjectTimeMusicValid
                    | StatesAndFlags_::kBarPositionValid
                    | StatesAndFlags_::kTimeSigValid;
                if transport.playing {
                    state |= StatesAndFlags_::kPlaying;
                }
                ctx.state = state as u32;
                ctx.tempo = transport.bpm;
                ctx.projectTimeMusic = transport.beat;
                ctx.barPositionMusic = (transport.beat / 4.0).floor() * 4.0;
                ctx.timeSigNumerator = 4;
                ctx.timeSigDenominator = 4;
            }
            let mut data = ProcessData {
                processMode: ProcessModes_::kRealtime as i32,
                symbolicSampleSize: SymbolicSampleSizes_::kSample32 as i32,
                numSamples: frames as i32,
                numInputs: i32::from(self.has_input),
                numOutputs: 1,
                inputs: if self.has_input { &mut ins } else { ptr::null_mut() },
                outputs: &mut outs,
                inputParameterChanges: self.changes.as_com_ref::<IParameterChanges>().unwrap().as_ptr(),
                outputParameterChanges: self.out_changes.as_com_ref::<IParameterChanges>().unwrap().as_ptr(),
                inputEvents: self.events.as_com_ref::<IEventList>().unwrap().as_ptr(),
                outputEvents: ptr::null_mut(),
                processContext: &mut ctx,
            };
            p.process(&mut data) == kResultOk
        };
        self.steady += frames as i64;
        for (ch, o) in output.into_iter().enumerate() {
            if ok {
                o.copy_from_slice(&self.output[ch][..frames]);
            } else {
                o.copy_from_slice(input[ch]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_pack_and_unpack() {
        let data = pack_states(b"abc", b"");
        assert_eq!(unpack_states(&data), Some((&b"abc"[..], &b""[..])));
        assert_eq!(unpack_states(&data[..5]), None);
    }

    #[test]
    fn class_ids_round_trip() {
        let hex = "6E33225254224A00AA69301AF318797D";
        assert_eq!(cid_to_hex(&hex_to_cid(hex).unwrap()), hex);
        assert!(hex_to_cid("nope").is_none());
    }
}
