//! Audio Unit effects (macOS only) inside the helper, through AudioToolbox. Effects are found in the system's component registry, which lists them without loading their code. An instance is made on the
//! deck's tempo through the host callbacks. Parameters are in the unit's own range. An id is `type:subtype:manufacturer` as four-character codes, as `auval` prints them (e.g. `aufx:dely:appl` for Apple's AUDelay).
//! deck's tempo through the host callbacks. Parameters are in the unit's own range. An id is `type:subtype:manufacturer` as four-character codes, as `auval` prints them (e.g. `aufx:dely:appl` for Apple's

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::{c_char, c_void};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::client::Transport;
use crate::descriptor::Descriptor;
use crate::effects::{arrange, EditorSize, Effect, Info, Loaded, MainThread, Param, ParentWindow};
use crate::shm::{Midi, CHANNELS};

type OSStatus = i32;
type AudioComponent = *mut c_void;
type AudioUnit = *mut c_void;
type CFStringRef = *const c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AudioComponentDescription {
    componentType: u32,
    componentSubType: u32,
    componentManufacturer: u32,
    componentFlags: u32,
    componentFlagsMask: u32,
}

#[repr(C)]
struct AudioStreamBasicDescription {
    mSampleRate: f64,
    mFormatID: u32,
    mFormatFlags: u32,
    mBytesPerPacket: u32,
    mFramesPerPacket: u32,
    mBytesPerFrame: u32,
    mChannelsPerFrame: u32,
    mBitsPerChannel: u32,
    mReserved: u32,
}

#[repr(C)]
struct SMPTETime {
    mSubframes: i16,
    mSubframeDivisor: i16,
    mCounter: u32,
    mType: u32,
    mFlags: u32,
    mHours: i16,
    mMinutes: i16,
    mSeconds: i16,
    mFrames: i16,
}

#[repr(C)]
struct AudioTimeStamp {
    mSampleTime: f64,
    mHostTime: u64,
    mRateScalar: f64,
    mWordClockTime: u64,
    mSMPTETime: SMPTETime,
    mFlags: u32,
    mReserved: u32,
}

#[repr(C)]
struct AudioBuffer {
    mNumberChannels: u32,
    mDataByteSize: u32,
    mData: *mut c_void,
}

/// An AudioBufferList with room for two (non-interleaved) channels.
#[repr(C)]
struct AudioBufferList2 {
    mNumberBuffers: u32,
    mBuffers: [AudioBuffer; 2],
}

type RenderCallback =
    unsafe extern "C" fn(*mut c_void, *mut u32, *const AudioTimeStamp, u32, u32, *mut AudioBufferList2) -> OSStatus;

#[repr(C)]
struct AURenderCallbackStruct {
    inputProc: RenderCallback,
    inputProcRefCon: *mut c_void,
}

type BeatAndTempoProc = unsafe extern "C" fn(*mut c_void, *mut f64, *mut f64) -> OSStatus;

#[repr(C)]
struct HostCallbackInfo {
    hostUserData: *mut c_void,
    beatAndTempoProc: Option<BeatAndTempoProc>,
    musicalTimeLocationProc: *const c_void,
    transportStateProc: *const c_void,
    transportStateProc2: *const c_void,
}

#[repr(C)]
struct AudioUnitParameterInfo {
    name: [c_char; 52],
    unitName: CFStringRef,
    clumpID: u32,
    cfNameString: CFStringRef,
    unit: u32,
    minValue: f32,
    maxValue: f32,
    defaultValue: f32,
    flags: u32,
}

#[link(name = "AudioToolbox", kind = "framework")]
extern "C" {
    fn AudioComponentFindNext(inComponent: AudioComponent, inDesc: *const AudioComponentDescription) -> AudioComponent;
    fn AudioComponentGetDescription(inComponent: AudioComponent, outDesc: *mut AudioComponentDescription) -> OSStatus;
    fn AudioComponentCopyName(inComponent: AudioComponent, outName: *mut CFStringRef) -> OSStatus;
    fn AudioComponentGetVersion(inComponent: AudioComponent, outVersion: *mut u32) -> OSStatus;
    fn AudioComponentInstanceNew(inComponent: AudioComponent, outInstance: *mut AudioUnit) -> OSStatus;
    fn AudioUnitSetProperty(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        inData: *const c_void,
        inDataSize: u32,
    ) -> OSStatus;
    fn AudioUnitGetProperty(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        outData: *mut c_void,
        ioDataSize: *mut u32,
    ) -> OSStatus;
    fn AudioUnitGetPropertyInfo(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        outDataSize: *mut u32,
        outWritable: *mut u8,
    ) -> OSStatus;
    fn AudioUnitInitialize(inUnit: AudioUnit) -> OSStatus;
    fn AudioUnitSetParameter(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        inValue: f32,
        inBufferOffsetInFrames: u32,
    ) -> OSStatus;
    fn AudioUnitGetParameter(
        inUnit: AudioUnit,
        inID: u32,
        inScope: u32,
        inElement: u32,
        outValue: *mut f32,
    ) -> OSStatus;
    fn AudioUnitRender(
        inUnit: AudioUnit,
        ioActionFlags: *mut u32,
        inTimeStamp: *const AudioTimeStamp,
        inOutputBusNumber: u32,
        inNumberFrames: u32,
        ioData: *mut AudioBufferList2,
    ) -> OSStatus;
    fn MusicDeviceMIDIEvent(
        inUnit: AudioUnit,
        inStatus: u32,
        inData1: u32,
        inData2: u32,
        inOffsetSampleFrame: u32,
    ) -> OSStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: isize, enc: u32) -> u8;
    fn CFRelease(cf: *const c_void);
    fn CFRunLoopRunInMode(mode: CFStringRef, seconds: f64, return_after: u8) -> i32;
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFPropertyListCreateData(
        allocator: *const c_void,
        list: *const c_void,
        format: isize,
        options: usize,
        error: *mut *const c_void,
    ) -> *const c_void;
    fn CFPropertyListCreateWithData(
        allocator: *const c_void,
        data: *const c_void,
        options: usize,
        format: *mut isize,
        error: *mut *const c_void,
    ) -> *const c_void;
    fn CFDataCreate(allocator: *const c_void, bytes: *const u8, length: isize) -> *const c_void;
    fn CFDataGetLength(data: *const c_void) -> isize;
    fn CFDataGetBytePtr(data: *const c_void) -> *const u8;
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
}

/// One of a unit's factory presets, as `kAudioUnitProperty_FactoryPresets`
/// lists them and `kAudioUnitProperty_PresentPreset` takes one.
#[repr(C)]
#[derive(Clone, Copy)]
struct AUPreset {
    presetNumber: i32,
    presetName: CFStringRef,
}

/// How a factory preset's reference starts (see [`presets`]).
const FACTORY: &str = "au-factory:";

/// `kCFPropertyListBinaryFormat_v1_0`.
const kCFPropertyListBinaryFormat_v1_0: isize = 200;
/// The unit's whole state, as a property list (what `.aupreset` files hold).
const kAudioUnitProperty_ClassInfo: u32 = 0;

const kCFStringEncodingUTF8: u32 = 0x0800_0100;

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

const kAudioUnitType_Effect: u32 = fourcc(b"aufx");
const kAudioUnitType_MusicEffect: u32 = fourcc(b"aumf");
const kAudioUnitType_MusicDevice: u32 = fourcc(b"aumu");
const kAudioFormatLinearPCM: u32 = fourcc(b"lpcm");
const kAudioFormatFlagIsFloat: u32 = 1;
const kAudioFormatFlagIsPacked: u32 = 1 << 3;
const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

const kAudioUnitScope_Global: u32 = 0;
const kAudioUnitScope_Input: u32 = 1;
const kAudioUnitScope_Output: u32 = 2;

const kAudioUnitProperty_ParameterList: u32 = 3;
const kAudioUnitProperty_ParameterInfo: u32 = 4;
const kAudioUnitProperty_StreamFormat: u32 = 8;
const kAudioUnitProperty_Latency: u32 = 12;
const kAudioUnitProperty_MaximumFramesPerSlice: u32 = 14;
const kAudioUnitProperty_SetRenderCallback: u32 = 23;
const kAudioUnitProperty_FactoryPresets: u32 = 24;
const kAudioUnitProperty_HostCallbacks: u32 = 27;
const kAudioUnitProperty_PresentPreset: u32 = 36;

const kAudioUnitParameterFlag_HasCFNameString: u32 = 1 << 27;
const kAudioUnitParameterFlag_IsReadable: u32 = 1 << 30;
const kAudioUnitParameterFlag_IsWritable: u32 = 1 << 31;
const kAudioTimeStampSampleTimeValid: u32 = 1;

fn code(c: u32) -> String {
    c.to_be_bytes().iter().map(|&b| b as char).collect()
}

fn parse_code(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    (b.len() == 4).then(|| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// `type:subtype:manufacturer`.
fn desc_id(d: &AudioComponentDescription) -> String {
    format!("{}:{}:{}", code(d.componentType), code(d.componentSubType), code(d.componentManufacturer))
}

fn parse_id(id: &str) -> Option<AudioComponentDescription> {
    let mut p = id.splitn(3, ':');
    Some(AudioComponentDescription {
        componentType: parse_code(p.next()?)?,
        componentSubType: parse_code(p.next()?)?,
        componentManufacturer: parse_code(p.next()?)?,
        ..Default::default()
    })
}

unsafe fn cf_string(s: CFStringRef) -> String {
    if s.is_null() {
        return String::new();
    }
    let mut buf = [0 as c_char; 256];
    if CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, kCFStringEncodingUTF8) == 0 {
        return String::new();
    }
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Lists every installed effect and instrument, from the registry. No
/// plugin code runs; it still happens in the helper, like every scan.
pub fn scan() -> Vec<Descriptor> {
    let mut found = Vec::new();
    // SAFETY: plain registry queries with valid out-pointers.
    unsafe {
        for (ty, feature) in [
            (kAudioUnitType_Effect, "audio-effect"),
            (kAudioUnitType_MusicEffect, "audio-effect"),
            (kAudioUnitType_MusicDevice, "instrument"),
        ] {
            let want = AudioComponentDescription { componentType: ty, ..Default::default() };
            let mut c = AudioComponentFindNext(ptr::null_mut(), &want);
            while !c.is_null() {
                let mut d = AudioComponentDescription::default();
                AudioComponentGetDescription(c, &mut d);
                let mut name: CFStringRef = ptr::null();
                AudioComponentCopyName(c, &mut name);
                // "Manufacturer: Name", as the registry gives it.
                let full = cf_string(name);
                if !name.is_null() {
                    CFRelease(name);
                }
                let (vendor, name) = match full.split_once(": ") {
                    Some((v, n)) => (v.to_string(), n.to_string()),
                    None => (String::new(), full),
                };
                let mut v = 0u32;
                AudioComponentGetVersion(c, &mut v);
                found.push(Descriptor {
                    id: desc_id(&d),
                    name,
                    vendor,
                    version: format!("{}.{}.{}", v >> 16, (v >> 8) & 0xff, v & 0xff),
                    features: vec![feature.into()],
                });
                c = AudioComponentFindNext(c, &want);
            }
        }
    }
    found
}

/// Keeps AUv3 extensions' messages flowing on the helper's main thread,
/// and shows the unit's view.
struct AuMain {
    unit: AudioUnit,
    view: Option<editor::View>,
    /// The shared parameters' ids, for reading their values back.
    param_ids: Vec<u32>,
}

impl MainThread for AuMain {
    fn load_preset(&mut self, reference: &str) -> Result<(), String> {
        let number: i32 =
            reference.strip_prefix(FACTORY).and_then(|n| n.parse().ok()).ok_or("not one of its presets")?;
        let mut result = Err("it no longer has that preset".to_string());
        // SAFETY: the unit lives as long as the helper; the preset (and its
        // name) is the unit's own, alive while it is set.
        unsafe {
            let unit = self.unit;
            each_factory_preset(unit, |p| {
                if p.presetNumber == number {
                    let r = AudioUnitSetProperty(
                        unit,
                        kAudioUnitProperty_PresentPreset,
                        kAudioUnitScope_Global,
                        0,
                        p as *const AUPreset as *const c_void,
                        std::mem::size_of::<AUPreset>() as u32,
                    );
                    result = check(r, "loading the preset");
                }
            });
        }
        result
    }

    fn param_values(&mut self, out: &mut [f64]) -> bool {
        for (o, id) in out.iter_mut().zip(&self.param_ids) {
            let mut v = 0.0f32;
            // SAFETY: the unit lives as long as the helper; reading a
            // parameter is allowed from any thread.
            if unsafe { AudioUnitGetParameter(self.unit, *id, kAudioUnitScope_Global, 0, &mut v) } == 0 {
                *o = v as f64;
            }
        }
        true
    }

    fn save_state(&mut self) -> Result<Vec<u8>, String> {
        // SAFETY: the unit lives as long as the helper; every object made
        // here is released before returning.
        unsafe {
            let mut list: *const c_void = std::ptr::null();
            let mut size = std::mem::size_of::<*const c_void>() as u32;
            let r = AudioUnitGetProperty(
                self.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &mut list as *mut _ as *mut c_void,
                &mut size,
            );
            if r != 0 || list.is_null() {
                return Err(format!("it can't save its sound ({r})"));
            }
            let data = CFPropertyListCreateData(
                std::ptr::null(),
                list,
                kCFPropertyListBinaryFormat_v1_0,
                0,
                std::ptr::null_mut(),
            );
            CFRelease(list);
            if data.is_null() {
                return Err("its sound couldn't be written down".into());
            }
            let (ptr, len) = (CFDataGetBytePtr(data), CFDataGetLength(data));
            let bytes = if ptr.is_null() || len <= 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr, len as usize).to_vec()
            };
            CFRelease(data);
            Ok(bytes)
        }
    }

    fn load_state(&mut self, bytes: &[u8]) -> Result<(), String> {
        // SAFETY: as in `save_state`.
        unsafe {
            let data = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize);
            if data.is_null() {
                return Err("out of memory".into());
            }
            let list =
                CFPropertyListCreateWithData(std::ptr::null(), data, 0, std::ptr::null_mut(), std::ptr::null_mut());
            CFRelease(data);
            if list.is_null() {
                return Err("the saved sound is damaged".into());
            }
            let r = AudioUnitSetProperty(
                self.unit,
                kAudioUnitProperty_ClassInfo,
                kAudioUnitScope_Global,
                0,
                &list as *const _ as *const c_void,
                std::mem::size_of::<*const c_void>() as u32,
            );
            CFRelease(list);
            if r != 0 {
                return Err(format!("it didn't take the saved sound ({r})"));
            }
            Ok(())
        }
    }

    fn idle(&mut self) {
        // SAFETY: runs the main thread's run loop briefly; called on it.
        unsafe {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.0, 1);
        }
    }

    fn open_editor(&mut self, parent: ParentWindow, _scale: f64) -> Result<EditorSize, String> {
        let ParentWindow::Cocoa(parent) = parent else {
            return Err("Audio Units only show on macOS".into());
        };
        let unit = self.unit;
        // SAFETY: the unit lives as long as the helper; on the main thread.
        let view = objc2::rc::autoreleasepool(|_| unsafe { editor::View::open(unit, parent) })?;
        let size = view.size();
        self.view = Some(view);
        Ok(size)
    }

    fn close_editor(&mut self) {
        self.view = None;
    }

    fn editor_can_resize(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.resizable)
    }

    fn editor_resized(&mut self, size: EditorSize) -> EditorSize {
        match &mut self.view {
            Some(v) => v.resize(size),
            None => size,
        }
    }

    fn take_editor_resize(&mut self) -> Option<EditorSize> {
        self.view.as_mut()?.take_own_resize()
    }
}

/// An Audio Unit's view in the helper's window: its own Cocoa view, its
/// AUv3 view controller's view, or else Apple's generic view of its
/// parameters (as Logic shows for units without one).
mod editor {
    use super::*;
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{class, msg_send};
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::cell::RefCell;
    use std::rc::Rc;

    // AudioUnitProperties.h
    const kAudioUnitProperty_CocoaUI: u32 = 31;
    const kAudioUnitProperty_RequestViewController: u32 = 56;

    #[repr(C)]
    struct CocoaViewInfo {
        bundle: *const c_void,
        class: [CFStringRef; 1],
    }

    pub struct View {
        view: Retained<AnyObject>,
        /// Keeps an AUv3 view controller (and so its view) alive.
        _controller: Option<Retained<AnyObject>>,
        pub resizable: bool,
        /// The view's size as last seen, to notice it resizing itself.
        last: EditorSize,
    }

    impl View {
        /// # Safety
        /// `unit` is a live unit and `parent` a live `NSView`; main thread.
        pub unsafe fn open(unit: AudioUnit, parent: *mut c_void) -> Result<View, String> {
            let (view, controller, resizable) = if let Some(v) = cocoa_view(unit) {
                (v, None, false)
            } else if let Some((v, c)) = requested_view(unit) {
                (v, Some(c), true)
            } else {
                eprintln!("editor: no view of the Audio Unit's own; showing its parameters");
                (generic_view(unit)?, None, true)
            };
            let parent = parent as *mut AnyObject;
            let frame: NSRect = msg_send![&*view, frame];
            let mut size = frame.size;
            if size.width < 1.0 || size.height < 1.0 {
                size = NSSize::new(480.0, 320.0);
            }
            let _: () = msg_send![&*view, setFrameOrigin: NSPoint::new(0.0, 0.0)];
            let _: () = msg_send![&*view, setFrameSize: size];
            let _: () = msg_send![parent, addSubview: &*view];
            let last = EditorSize { width: size.width.round() as u32, height: size.height.round() as u32 };
            Ok(View { view, _controller: controller, resizable, last })
        }

        pub fn size(&self) -> EditorSize {
            self.last
        }

        fn current(&self) -> EditorSize {
            // SAFETY: a live view, on the main thread.
            let frame: NSRect = unsafe { msg_send![&*self.view, frame] };
            EditorSize {
                width: frame.size.width.round().max(1.0) as u32,
                height: frame.size.height.round().max(1.0) as u32,
            }
        }

        pub fn resize(&mut self, size: EditorSize) -> EditorSize {
            if self.resizable {
                let s = NSSize::new(size.width as f64, size.height as f64);
                // SAFETY: as above.
                unsafe {
                    let _: () = msg_send![&*self.view, setFrameSize: s];
                }
            }
            self.last = self.current();
            self.last
        }

        /// A size the view gave itself (custom views resize on their own).
        pub fn take_own_resize(&mut self) -> Option<EditorSize> {
            let now = self.current();
            (now != self.last).then(|| {
                self.last = now;
                now
            })
        }
    }

    impl Drop for View {
        fn drop(&mut self) {
            // SAFETY: as above.
            unsafe {
                let _: () = msg_send![&*self.view, removeFromSuperview];
            }
        }
    }

    /// The unit's own Cocoa view, from the factory class it names.
    unsafe fn cocoa_view(unit: AudioUnit) -> Option<Retained<AnyObject>> {
        let mut size = 0u32;
        let mut writable = 0u8;
        if AudioUnitGetPropertyInfo(
            unit,
            kAudioUnitProperty_CocoaUI,
            kAudioUnitScope_Global,
            0,
            &mut size,
            &mut writable,
        ) != 0
            || (size as usize) < std::mem::size_of::<CocoaViewInfo>()
        {
            eprintln!("editor: the Audio Unit names no Cocoa view");
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if AudioUnitGetProperty(
            unit,
            kAudioUnitProperty_CocoaUI,
            kAudioUnitScope_Global,
            0,
            buf.as_mut_ptr() as *mut c_void,
            &mut size,
        ) != 0
        {
            eprintln!("editor: the Audio Unit's Cocoa view can't be read");
            return None;
        }
        let info = &*(buf.as_ptr() as *const CocoaViewInfo);
        let classes = (size as usize - std::mem::size_of::<*const c_void>()) / std::mem::size_of::<CFStringRef>();
        let view = (|| {
            if info.bundle.is_null() || classes == 0 || info.class[0].is_null() {
                eprintln!("editor: the Audio Unit's Cocoa view is empty");
                return None;
            }
            let bundle: Option<Retained<AnyObject>> =
                msg_send![class!(NSBundle), bundleWithURL: info.bundle as *mut AnyObject];
            let Some(bundle) = bundle else {
                eprintln!("editor: the Audio Unit's view bundle can't be opened");
                return None;
            };
            let _: bool = msg_send![&*bundle, load];
            let class: *const AnyClass = msg_send![&*bundle, classNamed: info.class[0] as *mut AnyObject];
            if class.is_null() {
                eprintln!("editor: the Audio Unit's view class isn't in its bundle");
                return None;
            }
            let factory: Option<Retained<AnyObject>> = msg_send![&*class, new];
            let Some(factory) = factory else {
                eprintln!("editor: the Audio Unit's view factory can't be made");
                return None;
            };
            // Some factories give no view for a zero size; ask again with one.
            for size in [NSSize::new(0.0, 0.0), NSSize::new(800.0, 500.0)] {
                let view: Option<Retained<AnyObject>> = msg_send![
                    &*factory,
                    uiViewForAudioUnit: unit as *mut AnyObject,
                    withSize: size
                ];
                if view.is_some() {
                    return view;
                }
            }
            eprintln!("editor: the Audio Unit's view factory gave no view");
            None
        })();
        if !info.bundle.is_null() {
            CFRelease(info.bundle);
        }
        for c in info.class.iter().take(classes.min(1)) {
            if !c.is_null() {
                CFRelease(*c);
            }
        }
        view
    }

    /// An AUv3 unit's view, from its view controller.
    unsafe fn requested_view(unit: AudioUnit) -> Option<(Retained<AnyObject>, Retained<AnyObject>)> {
        let got: Rc<RefCell<Option<Retained<AnyObject>>>> = Rc::default();
        let done = Rc::new(std::cell::Cell::new(false));
        let block = {
            let got = got.clone();
            let done = done.clone();
            block2::RcBlock::new(move |vc: *mut AnyObject| {
                *got.borrow_mut() = Retained::retain(vc);
                done.set(true);
            })
        };
        let ptr: *const block2::Block<dyn Fn(*mut AnyObject)> = &*block;
        if AudioUnitSetProperty(
            unit,
            kAudioUnitProperty_RequestViewController,
            kAudioUnitScope_Global,
            0,
            &ptr as *const _ as *const c_void,
            std::mem::size_of::<*const c_void>() as u32,
        ) != 0
        {
            return None;
        }
        // The answer may come later, through the main run loop.
        let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !done.get() && std::time::Instant::now() < end {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, 1);
        }
        let controller = got.borrow_mut().take()?;
        let view: Option<Retained<AnyObject>> = msg_send![&*controller, view];
        Some((view?, controller))
    }

    /// Apple's generic view of a unit's parameters (CoreAudioKit).
    unsafe fn generic_view(unit: AudioUnit) -> Result<Retained<AnyObject>, String> {
        let path = NSString::from_str("/System/Library/Frameworks/CoreAudioKit.framework");
        let bundle: Option<Retained<AnyObject>> = msg_send![class!(NSBundle), bundleWithPath: &*path];
        if let Some(b) = &bundle {
            let _: bool = msg_send![&**b, load];
        }
        let class = AnyClass::get(c"AUGenericView").ok_or("it has no window of its own")?;
        let view: Allocated<AnyObject> = msg_send![class, alloc];
        // AUViewTitleDisplayFlag | AUViewPropertiesDisplayFlag |
        // AUViewParametersDisplayFlag
        let flags: u32 = 1 | 2 | 4;
        let view: Option<Retained<AnyObject>> = msg_send![
            view,
            initWithAudioUnit: unit as *mut AnyObject,
            displayFlags: flags
        ];
        view.ok_or_else(|| "it has no window of its own".into())
    }
}

/// What the render callback reads; boxed so its address stays put.
struct Feed {
    input: [Vec<f32>; CHANNELS],
    frames: usize,
    bpm: AtomicU64,
    beat: AtomicU64,
}

unsafe extern "C" fn render_input(
    ref_con: *mut c_void,
    _flags: *mut u32,
    _ts: *const AudioTimeStamp,
    _bus: u32,
    frames: u32,
    data: *mut AudioBufferList2,
) -> OSStatus {
    let feed = &*(ref_con as *const Feed);
    let list = &mut *data;
    let n = (frames as usize).min(feed.frames);
    for ch in 0..(list.mNumberBuffers as usize).min(CHANNELS) {
        let b = &mut list.mBuffers[ch];
        if b.mData.is_null() {
            // The unit asks for our buffer rather than giving one.
            b.mData = feed.input[ch].as_ptr() as *mut c_void;
        } else {
            let dst = std::slice::from_raw_parts_mut(b.mData as *mut f32, n);
            dst.copy_from_slice(&feed.input[ch][..n]);
        }
        b.mDataByteSize = (n * 4) as u32;
    }
    0
}

unsafe extern "C" fn beat_and_tempo(user: *mut c_void, beat: *mut f64, tempo: *mut f64) -> OSStatus {
    let feed = &*(user as *const Feed);
    if !beat.is_null() {
        *beat = f64::from_bits(feed.beat.load(Ordering::Relaxed));
    }
    if !tempo.is_null() {
        *tempo = f64::from_bits(feed.bpm.load(Ordering::Relaxed));
    }
    0
}

/// Calls `f` with each of `unit`'s factory presets, while the
/// list the unit gave is alive (the names belong to it). #
/// Safety `unit` must be a live unit.
unsafe fn each_factory_preset(unit: AudioUnit, mut f: impl FnMut(&AUPreset)) {
    let mut list: *const c_void = ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;
    let r = AudioUnitGetProperty(
        unit,
        kAudioUnitProperty_FactoryPresets,
        kAudioUnitScope_Global,
        0,
        &mut list as *mut _ as *mut c_void,
        &mut size,
    );
    if r != 0 || list.is_null() {
        return;
    }
    for i in 0..CFArrayGetCount(list) {
        let p = CFArrayGetValueAtIndex(list, i) as *const AUPreset;
        if !p.is_null() {
            f(&*p);
        }
    }
    // The caller owns the list it is given.
    CFRelease(list);
}

/// Unit `id`'s factory presets, as (name, reference for
/// [`MainThread::load_preset`]). Runs in a throwaway helper
/// (`djsugar-plugin-host --presets --au`), never in DJ Sugar.
pub fn presets(id: &str) -> Result<Vec<(String, String)>, String> {
    let want = parse_id(id).ok_or_else(|| format!("bad Audio Unit id {id:?}"))?;
    // SAFETY: AudioToolbox calls with valid pointers; the unit lives until
    // the helper exits.
    unsafe {
        let comp = AudioComponentFindNext(ptr::null_mut(), &want);
        if comp.is_null() {
            return Err(format!("no Audio Unit {id:?} is installed"));
        }
        let mut unit: AudioUnit = ptr::null_mut();
        check(AudioComponentInstanceNew(comp, &mut unit), "starting it")?;
        let mut found = Vec::new();
        each_factory_preset(unit, |p| {
            found.push((cf_string(p.presetName), format!("{FACTORY}{}", p.presetNumber)));
        });
        Ok(found)
    }
}

fn check(r: OSStatus, what: &str) -> Result<(), String> {
    if r == 0 {
        Ok(())
    } else {
        Err(format!("{what} failed ({r})"))
    }
}

/// Loads the effect `id` (`type:subtype:manufacturer`).
pub fn load(id: &str, sample_rate: u32, max_frames: usize, picks: &[(usize, u32)]) -> Result<Loaded, String> {
    let want = parse_id(id).ok_or_else(|| format!("bad Audio Unit id {id:?}"))?;
    let name = scan()
        .into_iter()
        .find(|d| d.id == id)
        .map(|d| d.name)
        .ok_or_else(|| format!("no Audio Unit {id:?} is installed"))?;
    // SAFETY: AudioToolbox calls with valid pointers, in the documented
    // order (format, max frames, callbacks, initialize).
    unsafe {
        let comp = AudioComponentFindNext(ptr::null_mut(), &want);
        if comp.is_null() {
            return Err(format!("no Audio Unit {id:?} is installed"));
        }
        let mut unit: AudioUnit = ptr::null_mut();
        check(AudioComponentInstanceNew(comp, &mut unit), &format!("starting {name}"))?;
        let format = AudioStreamBasicDescription {
            mSampleRate: sample_rate as f64,
            mFormatID: kAudioFormatLinearPCM,
            mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked | kAudioFormatFlagIsNonInterleaved,
            mBytesPerPacket: 4,
            mFramesPerPacket: 1,
            mBytesPerFrame: 4,
            mChannelsPerFrame: 2,
            mBitsPerChannel: 32,
            mReserved: 0,
        };
        let size = std::mem::size_of::<AudioStreamBasicDescription>() as u32;
        // An instrument has no input: it plays the notes it is sent.
        let instrument = want.componentType == kAudioUnitType_MusicDevice;
        for scope in [kAudioUnitScope_Input, kAudioUnitScope_Output] {
            if instrument && scope == kAudioUnitScope_Input {
                continue;
            }
            let r = AudioUnitSetProperty(
                unit,
                kAudioUnitProperty_StreamFormat,
                scope,
                0,
                &format as *const _ as *const c_void,
                size,
            );
            if r != 0 {
                return Err(format!("{name} is not a stereo effect"));
            }
        }
        let max = max_frames as u32;
        AudioUnitSetProperty(
            unit,
            kAudioUnitProperty_MaximumFramesPerSlice,
            kAudioUnitScope_Global,
            0,
            &max as *const u32 as *const c_void,
            4,
        );
        let feed = Box::new(Feed {
            input: [vec![0.0; max_frames], vec![0.0; max_frames]],
            frames: 0,
            bpm: AtomicU64::new(0),
            beat: AtomicU64::new(0),
        });
        let feed_ptr = &*feed as *const Feed as *mut c_void;
        let cb = AURenderCallbackStruct { inputProc: render_input, inputProcRefCon: feed_ptr };
        if !instrument {
            check(
                AudioUnitSetProperty(
                    unit,
                    kAudioUnitProperty_SetRenderCallback,
                    kAudioUnitScope_Input,
                    0,
                    &cb as *const _ as *const c_void,
                    std::mem::size_of::<AURenderCallbackStruct>() as u32,
                ),
                &format!("connecting {name}"),
            )?;
        }
        let host = HostCallbackInfo {
            hostUserData: feed_ptr,
            beatAndTempoProc: Some(beat_and_tempo),
            musicalTimeLocationProc: ptr::null(),
            transportStateProc: ptr::null(),
            transportStateProc2: ptr::null(),
        };
        // Not every unit takes host callbacks; that's fine.
        AudioUnitSetProperty(
            unit,
            kAudioUnitProperty_HostCallbacks,
            kAudioUnitScope_Global,
            0,
            &host as *const _ as *const c_void,
            std::mem::size_of::<HostCallbackInfo>() as u32,
        );
        check(AudioUnitInitialize(unit), &format!("initializing {name}"))?;

        let mut latency = 0.0f64;
        let mut lsize = 8u32;
        AudioUnitGetProperty(
            unit,
            kAudioUnitProperty_Latency,
            kAudioUnitScope_Global,
            0,
            &mut latency as *mut f64 as *mut c_void,
            &mut lsize,
        );

        // Parameters, in the unit's own range.
        let mut params = Vec::new();
        let mut psize = 0u32;
        if AudioUnitGetPropertyInfo(
            unit,
            kAudioUnitProperty_ParameterList,
            kAudioUnitScope_Global,
            0,
            &mut psize,
            ptr::null_mut(),
        ) == 0
            && psize > 0
        {
            let mut ids = vec![0u32; psize as usize / 4];
            AudioUnitGetProperty(
                unit,
                kAudioUnitProperty_ParameterList,
                kAudioUnitScope_Global,
                0,
                ids.as_mut_ptr() as *mut c_void,
                &mut psize,
            );
            for pid in ids {
                let mut info: AudioUnitParameterInfo = std::mem::zeroed();
                let mut isize_ = std::mem::size_of::<AudioUnitParameterInfo>() as u32;
                if AudioUnitGetProperty(
                    unit,
                    kAudioUnitProperty_ParameterInfo,
                    kAudioUnitScope_Global,
                    pid,
                    &mut info as *mut _ as *mut c_void,
                    &mut isize_,
                ) != 0
                {
                    continue;
                }
                let rw = kAudioUnitParameterFlag_IsReadable | kAudioUnitParameterFlag_IsWritable;
                if info.flags & rw != rw {
                    continue;
                }
                let pname = if info.flags & kAudioUnitParameterFlag_HasCFNameString != 0 {
                    let n = cf_string(info.cfNameString);
                    CFRelease(info.cfNameString);
                    n
                } else {
                    let b: Vec<u8> = info.name.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
                    String::from_utf8_lossy(&b).into_owned()
                };
                let mut value = info.defaultValue;
                AudioUnitGetParameter(unit, pid, kAudioUnitScope_Global, 0, &mut value);
                params.push((
                    pid,
                    Param {
                        id: pid,
                        name: pname,
                        min: info.minValue as f64,
                        max: info.maxValue as f64,
                        default: info.defaultValue as f64,
                        value: value as f64,
                    },
                ));
            }
        }

        // The DJ's picks first, then the plugin's own order.
        let listed = arrange(&mut params, &[], picks);

        let effect = AuEffect {
            unit: Unit(unit),
            feed,
            output: [vec![0.0; max_frames], vec![0.0; max_frames]],
            ids: params.iter().map(|(id, _)| *id).collect(),
            sent: params.iter().map(|(_, p)| p.value as f32).collect(),
            sample_time: 0.0,
        };
        Ok(Loaded {
            effect: Box::new(effect),
            main: Some(Box::new(AuMain { unit, view: None, param_ids: params.iter().map(|(id, _)| *id).collect() })),
            info: Info {
                name,
                params: params.into_iter().map(|(_, p)| p).collect(),
                all: listed,
                latency: (latency * sample_rate as f64).round() as u32,
            },
        })
    }
}

struct Unit(AudioUnit);
// The unit is rendered only from the helper's audio thread.
unsafe impl Send for Unit {}

struct AuEffect {
    unit: Unit,
    feed: Box<Feed>,
    output: [Vec<f32>; CHANNELS],
    ids: Vec<u32>,
    sent: Vec<f32>,
    sample_time: f64,
}

impl Effect for AuEffect {
    fn process(
        &mut self,
        input: [&[f32]; CHANNELS],
        output: [&mut [f32]; CHANNELS],
        params: &[f32],
        transport: &Transport,
        notes: &[Midi],
    ) {
        let frames = input[0].len();
        let unit = self.unit.0;
        for n in notes {
            // SAFETY: a live unit; effects ignore MIDI they don't take.
            unsafe {
                MusicDeviceMIDIEvent(unit, n.data[0] as u32, n.data[1] as u32, n.data[2] as u32, n.frame);
            }
        }
        for (i, id) in self.ids.iter().enumerate() {
            if params[i] != self.sent[i] {
                self.sent[i] = params[i];
                // SAFETY: a live unit and one of its own parameter ids.
                unsafe {
                    AudioUnitSetParameter(unit, *id, kAudioUnitScope_Global, 0, params[i], 0);
                }
            }
        }
        for (ch, data) in input.iter().enumerate() {
            self.feed.input[ch][..frames].copy_from_slice(data);
        }
        self.feed.frames = frames;
        self.feed.bpm.store(transport.bpm.to_bits(), Ordering::Relaxed);
        self.feed.beat.store(transport.beat.to_bits(), Ordering::Relaxed);
        let mut list = AudioBufferList2 {
            mNumberBuffers: 2,
            mBuffers: [
                AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: (frames * 4) as u32,
                    mData: self.output[0].as_mut_ptr() as *mut c_void,
                },
                AudioBuffer {
                    mNumberChannels: 1,
                    mDataByteSize: (frames * 4) as u32,
                    mData: self.output[1].as_mut_ptr() as *mut c_void,
                },
            ],
        };
        // SAFETY: zeroed is a valid AudioTimeStamp; only the sample time is set.
        let mut ts: AudioTimeStamp = unsafe { std::mem::zeroed() };
        ts.mSampleTime = self.sample_time;
        ts.mFlags = kAudioTimeStampSampleTimeValid;
        let mut flags = 0u32;
        // SAFETY: the buffers hold `frames` floats each and outlive the call.
        let ok = unsafe { AudioUnitRender(unit, &mut flags, &ts, 0, frames as u32, &mut list) } == 0;
        self.sample_time += frames as f64;
        for (ch, o) in output.into_iter().enumerate() {
            // A unit may answer with its own buffers instead of ours.
            let src = list.mBuffers[ch].mData as *const f32;
            if ok && !src.is_null() {
                // SAFETY: the unit filled `frames` floats there.
                unsafe { o.copy_from_slice(std::slice::from_raw_parts(src, frames)) };
            } else {
                o.copy_from_slice(input[ch]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shm::MAX_PARAMS;

    #[test]
    fn ids_round_trip() {
        let d = parse_id("aufx:dely:appl").unwrap();
        assert_eq!(desc_id(&d), "aufx:dely:appl");
        assert!(parse_id("aufx:dely").is_none());
    }

    #[test]
    fn apples_delay_is_listed() {
        // Every Mac has Apple's own effects.
        let found = scan();
        assert!(found.iter().any(|d| d.id == "aufx:dely:appl"), "{found:?}");
    }

    #[test]
    fn apples_delay_loads_and_processes() {
        let mut loaded = load("aufx:dely:appl", 48_000, 512, &[]).expect("AUDelay loads");
        assert!(!loaded.info.params.is_empty());
        let l = vec![0.25f32; 256];
        let r = vec![-0.25f32; 256];
        let mut ol = vec![0.0; 256];
        let mut or = vec![0.0; 256];
        let params: Vec<f32> = loaded.info.params.iter().map(|p| p.value as f32).collect();
        let mut all = [0.0f32; MAX_PARAMS];
        all[..params.len()].copy_from_slice(&params);
        loaded.effect.process([&l, &r], [&mut ol, &mut or], &all, &Transport::default(), &[]);
        assert!(ol.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn apples_synth_plays_a_note() {
        // Apple's DLS synth comes with every Mac.
        let mut loaded = load("aumu:dls :appl", 48_000, 512, &[]).expect("DLSMusicDevice loads");
        let silent = vec![0.0f32; 512];
        let mut ol = vec![0.0; 512];
        let mut or = vec![0.0; 512];
        let all = [0.0f32; MAX_PARAMS];
        let mut heard = 0.0f32;
        for block in 0..40 {
            let notes = if block == 0 { vec![Midi::note_on(0, 0, 60, 120)] } else { Vec::new() };
            loaded.effect.process([&silent, &silent], [&mut ol, &mut or], &all, &Transport::default(), &notes);
            heard = heard.max(ol.iter().fold(0.0f32, |m, x| m.max(x.abs())));
        }
        assert!(heard > 0.001, "the note is heard: {heard}");
    }
}
