//! Bluetooth controllers on a phone. The system lists a Bluetooth MIDI device beside the wired ones only once an app
//! has opened it, so the low-energy devices the user paired in the system's settings are opened here and held; from
//! then on they are found and read like any other port.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use jni::errors::Error;
use jni::objects::{GlobalRef, JObject, JObjectArray, JString, JValue};
use jni::JNIEnv;
use jni_min_helper::{android_api_level, android_context, jni_with_env, JObjectGet, JniProxy};

/// A device that did not answer is not asked again sooner than this.
const AGAIN: Duration = Duration::from_secs(30);
/// `BluetoothDevice.DEVICE_TYPE_LE`: headphones and speakers are of the other kinds and are left alone.
const LOW_ENERGY: i32 = 2;
const LISTENER: &str = "android/media/midi/MidiManager$OnDeviceOpenedListener";
const OPEN: &str =
    "(Landroid/bluetooth/BluetoothDevice;Landroid/media/midi/MidiManager$OnDeviceOpenedListener;Landroid/os/Handler;)V";

struct Held {
    asked: Instant,
    device: Option<GlobalRef>,
    /// What the system answers to; kept for as long as it may.
    listener: Option<JniProxy>,
}

fn held() -> &'static Mutex<HashMap<String, Held>> {
    static HELD: OnceLock<Mutex<HashMap<String, Held>>> = OnceLock::new();
    HELD.get_or_init(Mutex::default)
}

/// Opens the paired devices that are not open. Cheap when there is nothing to do, so it runs with every look for ports.
pub(crate) fn wake() {
    if let Err(e) = jni_with_env(open_paired) {
        eprintln!("Bluetooth controllers: {e}");
    }
}

fn service<'a>(env: &mut JNIEnv<'a>, name: &str) -> Result<JObject<'a>, Error> {
    let name = env.new_string(name)?;
    let sig = "(Ljava/lang/String;)Ljava/lang/Object;";
    env.call_method(android_context(), "getSystemService", sig, &[JValue::from(&name)])?.l()
}

fn text(env: &mut JNIEnv<'_>, of: &JObject<'_>, method: &str) -> Result<String, Error> {
    let s: JString<'_> = env.call_method(of, method, "()Ljava/lang/String;", &[])?.l()?.into();
    if s.is_null() {
        return Ok(String::new());
    }
    let text = env.get_string(&s)?.to_string_lossy().into_owned();
    Ok(text)
}

/// Whether the user lets the app reach paired devices; before Android 12 the install granted it.
fn allowed(env: &mut JNIEnv<'_>) -> Result<bool, Error> {
    if android_api_level() < 31 {
        return Ok(true);
    }
    let perm = env.new_string("android.permission.BLUETOOTH_CONNECT")?;
    let sig = "(Ljava/lang/String;)I";
    Ok(env.call_method(android_context(), "checkSelfPermission", sig, &[JValue::from(&perm)])?.i()? == 0)
}

fn open_paired(env: &mut JNIEnv<'_>) -> Result<(), Error> {
    if !allowed(env)? {
        return Ok(());
    }
    let bluetooth = service(env, "bluetooth")?;
    if bluetooth.is_null() {
        return Ok(());
    }
    let adapter = env.call_method(&bluetooth, "getAdapter", "()Landroid/bluetooth/BluetoothAdapter;", &[])?.l()?;
    if adapter.is_null() || !env.call_method(&adapter, "isEnabled", "()Z", &[])?.z()? {
        return Ok(());
    }
    let midi = service(env, "midi")?;
    if midi.is_null() {
        return Ok(());
    }
    let paired = env.call_method(&adapter, "getBondedDevices", "()Ljava/util/Set;", &[])?.l()?;
    if paired.is_null() {
        return Ok(());
    }
    let paired: JObjectArray<'_> = env.call_method(&paired, "toArray", "()[Ljava/lang/Object;", &[])?.l()?.into();
    for i in 0..env.get_array_length(&paired)? {
        let device = env.get_object_array_element(&paired, i)?;
        if env.call_method(&device, "getType", "()I", &[])?.i()? != LOW_ENERGY {
            continue;
        }
        let address = text(env, &device, "getAddress")?;
        if !due(env, &address) {
            continue;
        }
        let listener = JniProxy::build(env, None, [LISTENER].as_slice(), {
            let address = address.clone();
            move |env, method, args| {
                if method.get_method_name(env).unwrap_or_default() == "onDeviceOpened" {
                    let opened = args.first().filter(|d| !d.is_null()).and_then(|d| env.new_global_ref(d).ok());
                    if let Some(h) = held().lock().unwrap().get_mut(&address) {
                        h.device = opened;
                    }
                }
                JniProxy::void(env)
            }
        })?;
        let none = JObject::null();
        let args = [JValue::from(&device), JValue::from(&listener), JValue::from(&none)];
        // Noted before the system is asked: its answer may come first, on another thread.
        held().lock().unwrap().insert(address.clone(), Held { asked: Instant::now(), device: None, listener: None });
        env.call_method(&midi, "openBluetoothDevice", OPEN, &args)?;
        if let Some(h) = held().lock().unwrap().get_mut(&address) {
            h.listener = Some(listener);
        }
    }
    Ok(())
}

/// Whether to open the device at `address` now: not while it is held and still connected, and not right after it
/// was asked. A held one that has gone away is closed, so that it can be opened again when it is back.
fn due(env: &mut JNIEnv<'_>, address: &str) -> bool {
    let mut all = held().lock().unwrap();
    let Some(h) = all.get(address) else { return true };
    if h.asked.elapsed() < AGAIN {
        return false;
    }
    if let Some(device) = &h.device {
        if connected(env, device.as_obj()).unwrap_or(false) {
            return false;
        }
        let _ = env.call_method(device.as_obj(), "close", "()V", &[]);
        let _ = env.exception_clear();
    }
    all.remove(address);
    true
}

/// Whether the system still lists the opened device among its MIDI devices.
fn connected(env: &mut JNIEnv<'_>, device: &JObject<'_>) -> Result<bool, Error> {
    let info = env.call_method(device, "getInfo", "()Landroid/media/midi/MidiDeviceInfo;", &[])?.l()?;
    let id = env.call_method(&info, "getId", "()I", &[])?.i()?;
    let midi = service(env, "midi")?;
    let listed: JObjectArray<'_> =
        env.call_method(&midi, "getDevices", "()[Landroid/media/midi/MidiDeviceInfo;", &[])?.l()?.into();
    for i in 0..env.get_array_length(&listed)? {
        let other = env.get_object_array_element(&listed, i)?;
        if env.call_method(&other, "getId", "()I", &[])?.i()? == id {
            return Ok(true);
        }
    }
    Ok(false)
}
