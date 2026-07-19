use wasmtime::{Caller, Engine, Linker, Store, StoreLimits, StoreLimitsBuilder};

use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::ProcessBlock;

use crate::WasmResourceLimits;
use crate::abi::{
    IMPORT_AUDIO_READ, IMPORT_AUDIO_WRITE, IMPORT_FRAME_COUNT, IMPORT_MODULE, IMPORT_PARAM_GET,
};

#[derive(Debug)]
pub(super) struct HostAudio {
    pub(super) frame_count: u32,
    pub(super) audio_in: Vec<Vec<f32>>,
    pub(super) audio_out: Vec<Vec<f32>>,
    pub(super) params: Vec<f64>,
    pub(super) store_limits: StoreLimits,
}

impl HostAudio {
    pub(super) fn new(limits: WasmResourceLimits) -> Self {
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

pub(super) fn build_audio_linker(engine: &Engine) -> Result<Linker<HostAudio>> {
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

pub(super) fn refill_fuel(store: &mut Store<HostAudio>, fuel: u64) -> Result<()> {
    store
        .set_fuel(fuel)
        .map_err(|err| Error::Eval(format!("wasm fuel refill failed: {err}")))
}

pub(super) fn silence_block(block: &mut ProcessBlock<'_>, frames: usize) {
    for output in block.out_audio.iter_mut() {
        if output.len() >= frames {
            output[..frames].fill(0.0);
        }
    }
}
