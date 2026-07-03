use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{portaudio_backend_priority, portaudio_backend_symbol, portaudio_transport_symbol};

const PORTAUDIO_LIB_ID: &str = "stream-portaudio";

/// Host-registered lib exporting the PortAudio stream-host cards, built on the
/// shared [`SurfacePackLib`] substrate.
pub struct PortAudioLib;

impl Lib for PortAudioLib {
    fn manifest(&self) -> LibManifest {
        portaudio_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        portaudio_pack().load(cx, linker)
    }
}

/// Installs [`PortAudioLib`] into `cx` exactly once.
///
/// Repeated calls are idempotent and leave the already-registered exports in
/// place.
pub fn install_stream_portaudio_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &PortAudioLib)?;
    Ok(())
}

fn portaudio_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "PortAudioBackend"),
        Symbol::qualified("stream", "PortAudioDefaultOutput"),
        Symbol::qualified("stream", "PortAudioTestTone"),
    ]
}

fn portaudio_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "PortAudioBackend" => "PortAudio host audio backend card",
        "PortAudioDefaultOutput" => "PortAudio default output open card",
        "PortAudioTestTone" => "PortAudio test-tone plan card",
        _ => "PortAudio card",
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
                SurfaceField::Symbol(portaudio_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(portaudio_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("priority"),
                SurfaceField::Symbols(portaudio_backend_priority()),
            ),
            (
                Symbol::new("capabilities"),
                SurfaceField::Symbols(Vec::new()),
            ),
        ],
    }
}

fn portaudio_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(PORTAUDIO_LIB_ID),
            values: portaudio_symbols()
                .into_iter()
                .map(portaudio_value_spec)
                .collect(),
        },
    }
}
