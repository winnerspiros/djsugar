//! A tiny CLAP effect for the plugin-host tests: a stereo gain with one
//! parameter ("Gain", 0 to 2, default 1) and a reported latency of 64 frames,
//! plus a second parameter ("Tempo out") that copies the host's tempo into the
//! left channel's first sample, and the key of the block's first note-on
//! into its second, so tests can check the transport and notes arrive.
//!
//! Its state (what a host saves and loads) is the gain, as 8 bytes.
//!
//! Its own presets, through CLAP preset discovery and preset load: "Loud"
//! (gain 1.5) and "Quiet" (0.25) inside the plugin, a preset for another
//! plugin a host should leave out, and any `.gain` file (the gain as text)
//! in the folder `DJSUGAR_TEST_PRESETS` names.
//!
//! It is a real `.clap` binary, loaded through the same path as third-party
//! plugins.

use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicU64, Ordering};

use clack_extensions::audio_ports::{
    AudioPortFlags, AudioPortInfo, AudioPortInfoWriter, AudioPortType, PluginAudioPorts,
    PluginAudioPortsImpl,
};
use clack_extensions::latency::{PluginLatency, PluginLatencyImpl};
use clack_extensions::params::{
    ParamDisplayWriter, ParamInfo, ParamInfoFlags, ParamInfoWriter, PluginAudioProcessorParams,
    PluginMainThreadParams, PluginParams,
};
use clack_extensions::preset_discovery::prelude::{
    FileType, Flags, Indexer, IndexerInfo, Location, LocationInfo, MetadataReceiver,
    PluginPresetLoad, PluginPresetLoadImpl, PresetDiscoveryFactoryImpl,
    PresetDiscoveryFactoryWrapper, ProviderDescriptor, ProviderImpl, ProviderInstance,
    UniversalPluginId,
};
use clack_extensions::remote_controls::{
    PluginRemoteControls, PluginRemoteControlsImpl, RemoteControlsPage, RemoteControlsPageWriter,
};
use clack_extensions::state::{PluginState, PluginStateImpl};
use clack_plugin::entry::prelude::*;
use clack_plugin::events::event_types::{NoteOnEvent, ParamValueEvent, TransportEvent};
use clack_plugin::events::Match;
use clack_plugin::prelude::*;
use clack_plugin::stream::{InputStream, OutputStream};
use clack_plugin::utils::Cookie;

const GAIN: u32 = 1;
const TEMPO_OUT: u32 = 2;
/// Does nothing: listed first, but last on its remote controls page, so a
/// host that follows the page puts it after the others.
const TONE: u32 = 3;

pub struct TestGain;

pub struct Shared {
    gain: AtomicU64,
    tempo_out: AtomicU64,
}

impl Shared {
    fn apply(&self, events: &InputEvents) {
        for e in events {
            if let Some(p) = e.as_event::<ParamValueEvent>() {
                match p.param_id().map(|i| i.get()) {
                    Some(GAIN) => self.gain.store(p.value().to_bits(), Ordering::Relaxed),
                    Some(TEMPO_OUT) => self.tempo_out.store(p.value().to_bits(), Ordering::Relaxed),
                    _ => {}
                }
            }
        }
    }

    fn get(&self, id: u32) -> Option<f64> {
        match id {
            GAIN => Some(f64::from_bits(self.gain.load(Ordering::Relaxed))),
            TEMPO_OUT => Some(f64::from_bits(self.tempo_out.load(Ordering::Relaxed))),
            TONE => Some(0.5),
            _ => None,
        }
    }
}

impl<'a> PluginShared<'a> for Shared {}

pub struct MainThread<'a> {
    shared: &'a Shared,
}

impl<'a> PluginMainThread<'a, Shared> for MainThread<'a> {}

impl Plugin for TestGain {
    type AudioProcessor<'a> = Processor<'a>;
    type Shared<'a> = Shared;
    type MainThread<'a> = MainThread<'a>;

    fn declare_extensions(builder: &mut PluginExtensions<Self>, _shared: Option<&Shared>) {
        builder
            .register::<PluginAudioPorts>()
            .register::<PluginParams>()
            .register::<PluginLatency>()
            .register::<PluginState>()
            .register::<PluginRemoteControls>()
            .register::<PluginPresetLoad>();
    }
}

impl DefaultPluginFactory for TestGain {
    fn get_descriptor() -> PluginDescriptor {
        use clack_plugin::plugin::features::{AUDIO_EFFECT, STEREO};
        PluginDescriptor::new("io.github.winnerspiros.djsugar.test-gain", "DJ Sugar Test Gain")
            .with_vendor("DJ Sugar")
            .with_features([AUDIO_EFFECT, STEREO])
    }

    fn new_shared(_host: HostSharedHandle<'_>) -> Result<Shared, PluginError> {
        Ok(Shared {
            gain: AtomicU64::new(1.0f64.to_bits()),
            tempo_out: AtomicU64::new(0.0f64.to_bits()),
        })
    }

    fn new_main_thread<'a>(
        _host: HostMainThreadHandle<'a>,
        shared: &'a Shared,
    ) -> Result<MainThread<'a>, PluginError> {
        Ok(MainThread { shared })
    }
}

pub struct Processor<'a> {
    shared: &'a Shared,
}

impl<'a> PluginAudioProcessor<'a, Shared, MainThread<'a>> for Processor<'a> {
    fn activate(
        _host: HostAudioProcessorHandle<'a>,
        _main: &MainThread<'a>,
        shared: &'a Shared,
        _config: PluginAudioConfiguration,
    ) -> Result<Self, PluginError> {
        Ok(Self { shared })
    }

    fn process(
        &mut self,
        process: Process,
        mut audio: Audio,
        events: Events,
    ) -> Result<ProcessStatus, PluginError> {
        self.shared.apply(events.input);
        let gain = self.shared.get(GAIN).unwrap_or(1.0) as f32;
        let tempo = process.transport.map(|t: &TransportEvent| t.tempo);
        let tempo_out = self.shared.get(TEMPO_OUT).unwrap_or(0.0) > 0.5;
        let note = events
            .input
            .iter()
            .find_map(|e| e.as_event::<NoteOnEvent>().map(|n| n.key()));
        for mut port in &mut audio {
            let Some(pairs) = port.channels()?.into_f32() else {
                continue;
            };
            for (ch, pair) in pairs.into_iter().enumerate() {
                let out: &mut [f32] = match pair {
                    ChannelPair::InputOnly(_) => continue,
                    ChannelPair::OutputOnly(buf) => {
                        buf.fill(0.0);
                        continue;
                    }
                    ChannelPair::InputOutput(i, o) => {
                        for (x, y) in i.iter().zip(o.iter_mut()) {
                            *y = x * gain;
                        }
                        o
                    }
                    ChannelPair::InPlace(buf) => {
                        for s in buf.iter_mut() {
                            *s *= gain;
                        }
                        buf
                    }
                };
                if tempo_out && ch == 0 && out.len() > 1 {
                    out[0] = tempo.unwrap_or(-1.0) as f32;
                    out[1] = match note {
                        Some(Match::Specific(k)) => k as f32,
                        _ => -1.0,
                    };
                }
            }
        }
        Ok(ProcessStatus::Continue)
    }
}

impl PluginAudioProcessorParams for Processor<'_> {
    fn flush(&mut self, input: &InputEvents, _output: &mut OutputEvents) {
        self.shared.apply(input);
    }
}

impl PluginAudioPortsImpl for MainThread<'_> {
    fn count(&self, _is_input: bool) -> u32 {
        1
    }

    fn get(&self, index: u32, is_input: bool, writer: &mut AudioPortInfoWriter) {
        if index == 0 {
            writer.set(&AudioPortInfo {
                id: ClapId::new(0),
                name: if is_input { b"In" } else { b"Out" },
                channel_count: 2,
                flags: AudioPortFlags::IS_MAIN,
                port_type: Some(AudioPortType::STEREO),
                in_place_pair: None,
            });
        }
    }
}

impl PluginMainThreadParams for MainThread<'_> {
    fn count(&self) -> u32 {
        3
    }

    fn get_info(&self, index: u32, info: &mut ParamInfoWriter) {
        let (id, name, max, default): (u32, &[u8], f64, f64) = match index {
            0 => (TONE, b"Tone", 1.0, 0.5),
            1 => (GAIN, b"Gain", 2.0, 1.0),
            2 => (TEMPO_OUT, b"Tempo out", 1.0, 0.0),
            _ => return,
        };
        info.set(&ParamInfo {
            id: ClapId::new(id),
            flags: ParamInfoFlags::IS_AUTOMATABLE,
            cookie: Cookie::empty(),
            name,
            module: b"",
            min_value: 0.0,
            max_value: max,
            default_value: default,
        });
    }

    fn get_value(&self, id: ClapId) -> Option<f64> {
        self.shared.get(id.get())
    }

    fn value_to_text(
        &self,
        _id: ClapId,
        value: f64,
        writer: &mut ParamDisplayWriter,
    ) -> std::fmt::Result {
        use std::fmt::Write;
        write!(writer, "{value:.2}")
    }

    fn text_to_value(&self, _id: ClapId, text: &std::ffi::CStr) -> Option<f64> {
        text.to_str().ok()?.trim().parse().ok()
    }

    fn flush(&self, input: &InputEvents, _output: &mut OutputEvents) {
        self.shared.apply(input);
    }
}

impl PluginLatencyImpl for MainThread<'_> {
    fn get(&self) -> u32 {
        64
    }
}

/// The plugin factory and the preset discovery factory.
pub struct TestEntry {
    plugins: PluginFactoryWrapper<Plugins>,
    presets: PresetDiscoveryFactoryWrapper<Presets>,
}

impl Entry for TestEntry {
    fn new(_path: Option<&CStr>) -> Result<Self, EntryLoadError> {
        Ok(Self {
            plugins: PluginFactoryWrapper::new(Plugins(TestGain::get_descriptor())),
            presets: PresetDiscoveryFactoryWrapper::new(Presets(ProviderDescriptor::new(
                "io.github.winnerspiros.djsugar.test-gain.presets",
                "DJ Sugar Test Gain presets",
            ))),
        })
    }

    fn declare_factories<'a>(&'a self, builder: &mut EntryFactories<'a>) {
        builder
            .register_factory(&self.plugins)
            .register_factory(&self.presets);
    }
}

pub struct Plugins(PluginDescriptor);

impl PluginFactoryImpl for Plugins {
    fn plugin_count(&self) -> u32 {
        1
    }

    fn plugin_descriptor(&self, index: u32) -> Option<&PluginDescriptor> {
        (index == 0).then_some(&self.0)
    }

    fn create_plugin<'a>(
        &'a self,
        host_info: HostInfo<'a>,
        plugin_id: &CStr,
    ) -> Option<PluginInstance<'a>> {
        (plugin_id == self.0.id().unwrap_or_default()).then(|| {
            PluginInstance::new::<TestGain>(
                host_info,
                &self.0,
                TestGain::new_shared,
                TestGain::new_main_thread,
            )
        })
    }
}

pub struct Presets(ProviderDescriptor);

impl PresetDiscoveryFactoryImpl for Presets {
    fn provider_count(&self) -> u32 {
        1
    }

    fn provider_descriptor(&self, index: u32) -> Option<&ProviderDescriptor> {
        (index == 0).then_some(&self.0)
    }

    fn create_provider<'a>(
        &'a self,
        host_info: IndexerInfo<'a>,
        provider_id: &CStr,
    ) -> Option<ProviderInstance<'a>> {
        (Some(provider_id) == self.0.id()).then(|| {
            ProviderInstance::new(host_info, &self.0, |mut indexer: Indexer<'a>| {
                let _ = indexer.declare_location(LocationInfo {
                    name: c"Built in",
                    flags: Flags::IS_FACTORY_CONTENT,
                    location: Location::Plugin,
                });
                if let Some(dir) = std::env::var_os("DJSUGAR_TEST_PRESETS") {
                    let dir = CString::new(dir.to_string_lossy().into_owned()).unwrap_or_default();
                    let _ = indexer.declare_filetype(FileType {
                        name: c"Gain",
                        description: None,
                        file_extension: Some(c"gain"),
                    });
                    let _ = indexer.declare_location(LocationInfo {
                        name: c"Mine",
                        flags: Flags::IS_USER_CONTENT,
                        location: Location::File { path: &dir },
                    });
                }
                Ok(Provider)
            })
        })
    }
}

pub struct Provider;

impl ProviderImpl<'_> for Provider {
    fn get_metadata(
        &mut self,
        location: Location,
        receiver: &mut MetadataReceiver,
    ) -> Result<(), PluginError> {
        let me = UniversalPluginId::clap(c"io.github.winnerspiros.djsugar.test-gain");
        let fail = |_| PluginError::Message("the host refused a preset");
        match location {
            Location::Plugin => {
                for (name, key) in [(c"Loud", c"loud"), (c"Quiet", c"quiet")] {
                    receiver
                        .begin_preset(Some(name), Some(key))
                        .map_err(fail)?
                        .add_plugin_id(me);
                }
                receiver
                    .begin_preset(Some(c"Not mine"), Some(c"other"))
                    .map_err(fail)?
                    .add_plugin_id(UniversalPluginId::clap(c"io.github.winnerspiros.djsugar.else"));
            }
            Location::File { .. } => {
                receiver
                    .begin_preset(None, None)
                    .map_err(fail)?
                    .add_plugin_id(me);
            }
        }
        Ok(())
    }
}

clack_export_entry!(TestEntry);

impl PluginPresetLoadImpl for MainThread<'_> {
    fn load_from_location(
        &self,
        location: Location,
        load_key: Option<&CStr>,
    ) -> Result<(), PluginError> {
        let gain: f64 = match location {
            Location::Plugin => match load_key.map(CStr::to_bytes) {
                Some(b"loud") => 1.5,
                Some(b"quiet") => 0.25,
                _ => return Err(PluginError::Message("no such preset")),
            },
            Location::File { path } => std::fs::read_to_string(&*path.to_string_lossy())
                .ok()
                .and_then(|t| t.trim().parse().ok())
                .ok_or(PluginError::Message("a bad preset file"))?,
        };
        self.shared.gain.store(gain.to_bits(), Ordering::Relaxed);
        Ok(())
    }
}

impl PluginStateImpl for MainThread<'_> {
    fn save(&self, output: &mut OutputStream) -> Result<(), PluginError> {
        use std::io::Write;
        let gain = self.shared.gain.load(Ordering::Relaxed);
        output
            .write_all(&gain.to_le_bytes())
            .map_err(|_| PluginError::Message("can't write the state"))
    }

    fn load(&self, input: &mut InputStream) -> Result<(), PluginError> {
        use std::io::Read;
        let mut bytes = [0; 8];
        input
            .read_exact(&mut bytes)
            .map_err(|_| PluginError::Message("the state is too short"))?;
        self.shared
            .gain
            .store(u64::from_le_bytes(bytes), Ordering::Relaxed);
        Ok(())
    }
}

impl PluginRemoteControlsImpl for MainThread<'_> {
    fn count(&self) -> u32 {
        1
    }

    fn get(&self, index: u32, writer: &mut RemoteControlsPageWriter) {
        if index == 0 {
            writer.set(&RemoteControlsPage {
                section_name: b"",
                page_id: ClapId::new(1),
                page_name: b"Main",
                param_ids: [GAIN, TEMPO_OUT, TONE, 0, 0, 0, 0, 0]
                    .map(|id| (id > 0).then(|| ClapId::new(id))),
                is_for_preset: false,
            });
        }
    }
}
