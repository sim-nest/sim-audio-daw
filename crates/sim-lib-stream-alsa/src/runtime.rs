use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{alsa_backend_symbol, alsa_transport_symbol};

const ALSA_LIB_ID: &str = "stream-alsa";

/// Host-registered lib exporting the ALSA stream-host cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct AlsaLib;

impl Lib for AlsaLib {
    fn manifest(&self) -> LibManifest {
        alsa_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        alsa_pack().load(cx, linker)
    }
}

/// Installs the [`AlsaLib`] into `cx`, idempotently.
///
/// Uses `install_once`, so repeated calls register the lib at most once.
pub fn install_stream_alsa_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &AlsaLib)?;
    Ok(())
}

fn alsa_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "AlsaBackend"),
        Symbol::qualified("stream", "AlsaDefaultPcm"),
        Symbol::qualified("stream", "AlsaHwPcm"),
        Symbol::qualified("stream", "AlsaPlugHwPcm"),
        Symbol::qualified("stream", "AlsaSequencerFollowUp"),
    ]
}

fn alsa_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "AlsaBackend" => "ALSA host PCM backend card",
        "AlsaDefaultPcm" => "ALSA default PCM fallback card",
        "AlsaHwPcm" => "ALSA hw:* PCM naming card",
        "AlsaPlugHwPcm" => "ALSA plughw:* PCM naming card",
        "AlsaSequencerFollowUp" => "ALSA sequencer MIDI follow-up card",
        _ => "ALSA card",
    };
    SurfaceValueSpec {
        symbol: symbol.clone(),
        fields: vec![
            (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
            (
                Symbol::new("layer"),
                SurfaceField::Str("stream-host".to_owned()),
            ),
            (Symbol::new("kind"), SurfaceField::Str("plugin".to_owned())),
            (
                Symbol::new("backend"),
                SurfaceField::Symbol(alsa_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(alsa_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("pcm-names"),
                SurfaceField::Strs(vec![
                    "default".to_owned(),
                    "hw:*".to_owned(),
                    "plughw:*".to_owned(),
                ]),
            ),
            (
                Symbol::new("sequencer-midi"),
                SurfaceField::Str(
                    "follow-up: keep ALSA sequencer MIDI in a MIDI adapter".to_owned(),
                ),
            ),
        ],
    }
}

fn alsa_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(ALSA_LIB_ID),
            values: alsa_symbols().into_iter().map(alsa_value_spec).collect(),
        },
    }
}
