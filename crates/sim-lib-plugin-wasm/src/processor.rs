//! Wasmtime-backed audio plugin processor.

#[cfg(feature = "wasm-plugin")]
use wasmtime::{
    Caller, Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

use sim_kernel::{Error, Result};
#[cfg(feature = "wasm-plugin")]
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia, PrepareConfig, ProcessBlock};
use sim_lib_plugin_core::PluginDescriptor;
#[cfg(feature = "wasm-plugin")]
use sim_lib_plugin_core::{
    ParameterDescriptor, PluginFormat, PluginId, PluginInstance, PluginState,
};

use crate::WasmResourceLimits;
#[cfg(feature = "wasm-plugin")]
use crate::abi::{
    EXPORT_MANIFEST_PTR, EXPORT_PREPARE, EXPORT_PROCESS, EXPORT_RESET, IMPORT_AUDIO_READ,
    IMPORT_AUDIO_WRITE, IMPORT_FRAME_COUNT, IMPORT_MODULE, IMPORT_PARAM_GET, WasmAudioManifest,
};

#[cfg(feature = "wasm-plugin")]
const LOAD_FUEL: u64 = 10_000_000;

/// Host ceiling on the audio channel count a plugin manifest may declare.
///
/// The manifest is guest-supplied text; the host sizes per-channel buffers from
/// it in [`WasmPluginProcessor::prepare`], so an unbounded count would let a
/// hostile plugin drive an arbitrary host allocation. The cap is validated when
/// the descriptor is built, before any host buffer is allocated.
#[cfg(feature = "wasm-plugin")]
const MAX_PLUGIN_CHANNELS: u16 = 512;

#[cfg(feature = "wasm-plugin")]
#[derive(Debug)]
struct HostAudio {
    frame_count: u32,
    audio_in: Vec<Vec<f32>>,
    audio_out: Vec<Vec<f32>>,
    params: Vec<f64>,
    store_limits: StoreLimits,
}

#[cfg(feature = "wasm-plugin")]
impl HostAudio {
    fn new(limits: WasmResourceLimits) -> Self {
        Self {
            frame_count: 0,
            audio_in: Vec::new(),
            audio_out: Vec::new(),
            params: Vec::new(),
            store_limits: StoreLimitsBuilder::new()
                .memory_size(limits.max_memory_bytes())
                .trap_on_grow_failure(true)
                .build(),
        }
    }
}

/// WebAssembly-backed plugin instance exposed through the shared plugin API.
pub struct WasmPluginProcessor {
    #[cfg(feature = "wasm-plugin")]
    store: Store<HostAudio>,
    #[cfg(feature = "wasm-plugin")]
    fn_prepare: TypedFunc<(f64, u32), ()>,
    #[cfg(feature = "wasm-plugin")]
    fn_reset: TypedFunc<(), ()>,
    #[cfg(feature = "wasm-plugin")]
    fn_process: TypedFunc<(), i32>,
    descriptor: PluginDescriptor,
    last_error: Option<String>,
    #[cfg(feature = "wasm-plugin")]
    state: PluginState,
    #[cfg(feature = "wasm-plugin")]
    limits: WasmResourceLimits,
}

impl WasmPluginProcessor {
    /// Returns this plugin's descriptor.
    pub fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    /// Returns and clears the last Wasm backend error observed through a
    /// non-`Result` trait entry point.
    pub fn take_last_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    #[cfg(feature = "wasm-plugin")]
    fn remember_error(&mut self, err: &Error) {
        self.last_error = Some(err.to_string());
    }

    #[cfg(feature = "wasm-plugin")]
    fn clear_last_error(&mut self) {
        self.last_error = None;
    }

    /// Sets one host-side parameter value.
    ///
    /// # Errors
    ///
    /// Returns an error when `id` is outside the manifest-declared parameter
    /// range, or when the crate is built without the `wasm-plugin` feature.
    pub fn set_param(&mut self, id: u32, value: f64) -> Result<()> {
        self.set_param_inner(id, value)
    }

    #[cfg(feature = "wasm-plugin")]
    fn set_param_inner(&mut self, id: u32, value: f64) -> Result<()> {
        let Some(slot) = self.store.data_mut().params.get_mut(id as usize) else {
            return Err(Error::Eval(format!("wasm plugin parameter {id} is absent")));
        };
        *slot = value;
        self.state.set_param(id, value);
        Ok(())
    }

    #[cfg(not(feature = "wasm-plugin"))]
    fn set_param_inner(&mut self, _id: u32, _value: f64) -> Result<()> {
        Err(Error::Eval(
            "wasm plugin runtime feature is not enabled".to_owned(),
        ))
    }
}

#[cfg(feature = "wasm-plugin")]
impl WasmPluginProcessor {
    /// Instantiates a wasm audio plugin from module bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the module is invalid, missing required exports, or
    /// declares an invalid plugin descriptor.
    pub fn from_bytes(wasm: &[u8]) -> Result<Self> {
        Self::from_bytes_with_limits(wasm, WasmResourceLimits::default())
    }

    /// Instantiates a wasm audio plugin from module bytes with resource limits.
    ///
    /// # Errors
    ///
    /// Returns an error when the module is invalid, missing required exports,
    /// exceeds memory limits, or declares an invalid plugin descriptor.
    pub fn from_bytes_with_limits(wasm: &[u8], limits: WasmResourceLimits) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = Engine::new(&config)
            .map_err(|err| Error::Eval(format!("wasm engine init failed: {err}")))?;
        let module = Module::new(&engine, wasm)
            .map_err(|err| Error::Eval(format!("wasm module invalid: {err}")))?;
        let linker = build_audio_linker(&engine)?;
        let host = HostAudio::new(limits);
        let mut store = Store::new(&engine, host);
        store.limiter(|host| &mut host.store_limits);
        refill_fuel(&mut store, LOAD_FUEL)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|err| Error::Eval(format!("wasm instantiate failed: {err}")))?;
        refill_fuel(&mut store, LOAD_FUEL)?;

        let manifest_ptr_fn: TypedFunc<(), u32> = instance
            .get_typed_func(&mut store, EXPORT_MANIFEST_PTR)
            .map_err(|err| Error::Eval(format!("missing {EXPORT_MANIFEST_PTR}: {err}")))?;
        let ptr = manifest_ptr_fn
            .call(&mut store, ())
            .map_err(|err| Error::Eval(format!("{EXPORT_MANIFEST_PTR} trapped: {err}")))?
            as usize;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Error::Eval("wasm plugin has no exported memory".to_owned()))?;
        let mem_data = memory.data(&store);
        let raw_bytes = mem_data
            .get(ptr..ptr + WasmAudioManifest::SIZE)
            .ok_or_else(|| Error::Eval("manifest pointer is out of bounds".to_owned()))?;
        let manifest = WasmAudioManifest::from_bytes(raw_bytes)?;
        let descriptor = descriptor_from_manifest(&manifest)?;
        store.data_mut().params = vec![1.0; manifest.param_count as usize];

        let fn_prepare = instance
            .get_typed_func::<(f64, u32), ()>(&mut store, EXPORT_PREPARE)
            .map_err(|err| Error::Eval(format!("missing {EXPORT_PREPARE}: {err}")))?;
        let fn_reset = instance
            .get_typed_func::<(), ()>(&mut store, EXPORT_RESET)
            .map_err(|err| Error::Eval(format!("missing {EXPORT_RESET}: {err}")))?;
        let fn_process = instance
            .get_typed_func::<(), i32>(&mut store, EXPORT_PROCESS)
            .map_err(|err| Error::Eval(format!("missing {EXPORT_PROCESS}: {err}")))?;

        Ok(Self {
            store,
            fn_prepare,
            fn_reset,
            fn_process,
            descriptor,
            last_error: None,
            state: PluginState::new(),
            limits,
        })
    }

    /// Processes one block and reports wasm traps as eval errors.
    ///
    /// On error, the output lanes in `block` are silenced before returning.
    ///
    /// # Errors
    ///
    /// Returns an eval error when the plugin returns a nonzero status or traps
    /// during `sim_audio_process`, including fuel exhaustion.
    pub fn process_checked(&mut self, block: &mut ProcessBlock<'_>) -> Result<()> {
        let frames = block.frames as usize;
        {
            let host = self.store.data_mut();
            host.frame_count = block.frames;
            for (ch, input) in block.in_audio.iter().enumerate() {
                if let Some(lane) = host.audio_in.get_mut(ch)
                    && lane.len() >= frames
                    && input.len() >= frames
                {
                    lane[..frames].copy_from_slice(&input[..frames]);
                }
            }
            for lane in &mut host.audio_out {
                if lane.len() >= frames {
                    lane[..frames].fill(0.0);
                }
            }
        }

        refill_fuel(&mut self.store, self.limits.fuel_per_process)?;
        match self.fn_process.call(&mut self.store, ()) {
            Ok(0) => {
                let host = self.store.data();
                for (ch, output) in block.out_audio.iter_mut().enumerate() {
                    if let Some(lane) = host.audio_out.get(ch)
                        && lane.len() >= frames
                        && output.len() >= frames
                    {
                        output[..frames].copy_from_slice(&lane[..frames]);
                    }
                }
                self.clear_last_error();
                Ok(())
            }
            Ok(code) => {
                silence_block(block, frames);
                let err = Error::Eval(format!("wasm plugin process returned status {code}"));
                self.remember_error(&err);
                Err(err)
            }
            Err(err) => {
                silence_block(block, frames);
                let err = Error::Eval(format!("wasm plugin process trapped: {err}"));
                self.remember_error(&err);
                Err(err)
            }
        }
    }
}

#[cfg(not(feature = "wasm-plugin"))]
impl WasmPluginProcessor {
    /// Reports that the wasm runtime feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an eval error because no runtime is available.
    pub fn from_bytes(wasm: &[u8]) -> Result<Self> {
        Self::from_bytes_with_limits(wasm, WasmResourceLimits::default())
    }

    /// Reports that the wasm runtime feature is disabled.
    ///
    /// # Errors
    ///
    /// Always returns an eval error because no runtime is available.
    pub fn from_bytes_with_limits(_wasm: &[u8], _limits: WasmResourceLimits) -> Result<Self> {
        Err(Error::Eval(
            "wasm plugin runtime feature is not enabled".to_owned(),
        ))
    }
}

#[cfg(feature = "wasm-plugin")]
fn check_plugin_channels(which: &str, channels: u16) -> Result<()> {
    if channels > MAX_PLUGIN_CHANNELS {
        return Err(Error::Eval(format!(
            "wasm plugin declares {channels} {which} channels, exceeding the host maximum of {MAX_PLUGIN_CHANNELS}"
        )));
    }
    Ok(())
}

#[cfg(feature = "wasm-plugin")]
fn descriptor_from_manifest(manifest: &WasmAudioManifest) -> Result<PluginDescriptor> {
    check_plugin_channels("input", manifest.audio_in_channels)?;
    check_plugin_channels("output", manifest.audio_out_channels)?;
    let plugin_id = PluginId::new(PluginFormat::Wasm, manifest.stable_id_str().to_owned())?;
    let mut descriptor = PluginDescriptor::new(
        plugin_id,
        manifest.name_str().to_owned(),
        manifest.vendor_str().to_owned(),
        "0.1.0".to_owned(),
    )?;
    if manifest.audio_in_channels > 0 {
        descriptor.ports.push(PortDecl::new(
            "audio-in",
            PortMedia::Audio,
            PortDir::In,
            manifest.audio_in_channels,
        ));
    }
    if manifest.audio_out_channels > 0 {
        descriptor.ports.push(PortDecl::new(
            "audio-out",
            PortMedia::Audio,
            PortDir::Out,
            manifest.audio_out_channels,
        ));
    }
    for id in 0..u32::from(manifest.param_count) {
        descriptor.parameters.push(ParameterDescriptor::new(
            id,
            format!("param-{id}"),
            format!("Param {id}"),
            0.0,
            1.0,
            1.0,
        )?);
    }
    Ok(descriptor)
}

#[cfg(feature = "wasm-plugin")]
fn build_audio_linker(engine: &Engine) -> Result<Linker<HostAudio>> {
    let mut linker = Linker::new(engine);
    linker
        .func_wrap(
            IMPORT_MODULE,
            IMPORT_FRAME_COUNT,
            |caller: Caller<'_, HostAudio>| caller.data().frame_count,
        )
        .map_err(|err| Error::Eval(err.to_string()))?;
    linker
        .func_wrap(
            IMPORT_MODULE,
            IMPORT_AUDIO_READ,
            |caller: Caller<'_, HostAudio>, ch: u32, frame: u32| -> f32 {
                caller
                    .data()
                    .audio_in
                    .get(ch as usize)
                    .and_then(|lane| lane.get(frame as usize))
                    .copied()
                    .unwrap_or(0.0)
            },
        )
        .map_err(|err| Error::Eval(err.to_string()))?;
    linker
        .func_wrap(
            IMPORT_MODULE,
            IMPORT_AUDIO_WRITE,
            |mut caller: Caller<'_, HostAudio>, ch: u32, frame: u32, value: f32| {
                if let Some(lane) = caller.data_mut().audio_out.get_mut(ch as usize)
                    && let Some(sample) = lane.get_mut(frame as usize)
                {
                    *sample = value;
                }
            },
        )
        .map_err(|err| Error::Eval(err.to_string()))?;
    linker
        .func_wrap(
            IMPORT_MODULE,
            IMPORT_PARAM_GET,
            |caller: Caller<'_, HostAudio>, id: u32| -> f64 {
                caller
                    .data()
                    .params
                    .get(id as usize)
                    .copied()
                    .unwrap_or(1.0)
            },
        )
        .map_err(|err| Error::Eval(err.to_string()))?;
    Ok(linker)
}

#[cfg(feature = "wasm-plugin")]
fn refill_fuel(store: &mut Store<HostAudio>, fuel: u64) -> Result<()> {
    store
        .set_fuel(fuel)
        .map_err(|err| Error::Eval(format!("wasm fuel refill failed: {err}")))
}

#[cfg(feature = "wasm-plugin")]
fn silence_block(block: &mut ProcessBlock<'_>, frames: usize) {
    for output in block.out_audio.iter_mut() {
        if output.len() >= frames {
            output[..frames].fill(0.0);
        }
    }
}

#[cfg(feature = "wasm-plugin")]
impl PluginInstance for WasmPluginProcessor {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    fn set_state(&mut self, state: PluginState) {
        let mut applied = PluginState::new();
        let mut rejected = Vec::new();
        for (&id, &value) in state.params() {
            match self.set_param(id, value) {
                Ok(()) => applied.set_param(id, value),
                Err(err) => rejected.push(err.to_string()),
            }
        }
        self.state = applied;
        if rejected.is_empty() {
            self.clear_last_error();
        } else {
            self.last_error = Some(format!(
                "wasm plugin state restore skipped {} parameter(s): {}",
                rejected.len(),
                rejected.join("; ")
            ));
        }
    }

    fn prepare(&mut self, cfg: PrepareConfig) {
        let _ = refill_fuel(&mut self.store, LOAD_FUEL);
        let _ = self.fn_prepare.call(
            &mut self.store,
            (f64::from(cfg.sample_rate_hz), cfg.max_block_frames),
        );
        let ch_in = self
            .descriptor
            .ports
            .iter()
            .filter(|port| port.media == PortMedia::Audio && port.dir == PortDir::In)
            .map(|port| port.channels as usize)
            .sum::<usize>();
        let ch_out = self
            .descriptor
            .ports
            .iter()
            .filter(|port| port.media == PortMedia::Audio && port.dir == PortDir::Out)
            .map(|port| port.channels as usize)
            .sum::<usize>();
        let frames = cfg.max_block_frames as usize;
        self.store.data_mut().audio_in = vec![vec![0.0; frames]; ch_in];
        self.store.data_mut().audio_out = vec![vec![0.0; frames]; ch_out];
    }

    fn reset(&mut self) {
        let _ = refill_fuel(&mut self.store, LOAD_FUEL);
        let _ = self.fn_reset.call(&mut self.store, ());
    }

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        let _ = self.process_checked(block);
    }

    fn take_last_error(&mut self) -> Option<String> {
        WasmPluginProcessor::take_last_error(self)
    }
}

#[cfg(all(test, feature = "wasm-plugin"))]
mod tests {
    use super::{MAX_PLUGIN_CHANNELS, descriptor_from_manifest};
    use crate::abi::WasmAudioManifest;

    fn manifest_with_channels(audio_in: u16, audio_out: u16) -> WasmAudioManifest {
        let mut name = [0u8; 64];
        name[..4].copy_from_slice(b"test");
        let mut stable_id = [0u8; 64];
        stable_id[..8].copy_from_slice(b"sim.test");
        WasmAudioManifest {
            audio_in_channels: audio_in,
            audio_out_channels: audio_out,
            param_count: 0,
            _pad: 0,
            name,
            vendor: [0u8; 32],
            stable_id,
        }
    }

    #[test]
    fn manifest_within_channel_cap_builds_descriptor() {
        let manifest = manifest_with_channels(2, MAX_PLUGIN_CHANNELS);
        let descriptor = descriptor_from_manifest(&manifest).expect("channel count within cap");
        assert_eq!(descriptor.ports.len(), 2);
    }

    #[test]
    fn manifest_over_channel_cap_is_rejected() {
        let manifest = manifest_with_channels(u16::MAX, 2);
        let err = descriptor_from_manifest(&manifest)
            .expect_err("manifest over the channel cap must be rejected");
        assert!(format!("{err}").contains("exceeding the host maximum"));
    }

    #[test]
    fn manifest_over_output_channel_cap_is_rejected() {
        let manifest = manifest_with_channels(2, MAX_PLUGIN_CHANNELS + 1);
        let err = descriptor_from_manifest(&manifest)
            .expect_err("output channel count over the cap must be rejected");
        assert!(format!("{err}").contains("output channels"));
    }
}
