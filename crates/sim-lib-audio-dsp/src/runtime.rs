use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const AUDIO_DSP_LIB_ID: &str = "audio-dsp";

/// Host-registered lib exporting the reusable DSP processor cards, built on the
/// shared [`SurfacePackLib`] substrate.
pub struct AudioDspLib;

impl Lib for AudioDspLib {
    fn manifest(&self) -> LibManifest {
        audio_dsp_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        audio_dsp_pack().load(cx, linker)
    }
}

/// Installs [`AudioDspLib`] into the context once, idempotently.
pub fn install_audio_dsp_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &AudioDspLib)?;
    Ok(())
}

/// Returns the namespaced symbols for every DSP processor card this lib exports.
pub fn audio_dsp_symbols() -> Vec<Symbol> {
    [
        "SmoothValue",
        "SmoothedGain",
        "Gain",
        "Pan",
        "DcBlocker",
        "OnePoleFilter",
        "BiquadFilter",
        "StateVariableFilter",
        "DelayProcessor",
        "FractionalDelay",
        "CombFilter",
        "AllPassFilter",
        "Chorus",
        "Flanger",
        "Vibrato",
        "Waveshaper",
        "SoftClipper",
        "Compressor",
        "Limiter",
        "Gate",
        "OversamplingWrapper",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(AUDIO_DSP_LIB_ID, name))
    .collect()
}

fn audio_dsp_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(AUDIO_DSP_LIB_ID),
            values: audio_dsp_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(AUDIO_DSP_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("audio-graph-processor".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str(
                                "reusable pure Rust DSP processor descriptor".to_owned(),
                            ),
                        ),
                        (
                            Symbol::new("contract"),
                            SurfaceField::Str("Processor".to_owned()),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
