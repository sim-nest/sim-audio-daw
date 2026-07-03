use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{
    coreaudio_backend_symbol, coreaudio_transport_symbol, macos_audio_backend_priority,
    macos_midi_backend_priority,
};

const COREAUDIO_LIB_ID: &str = "stream-coreaudio";

/// Host-registered lib exporting the CoreAudio stream-host cards, built on the
/// shared [`SurfacePackLib`] substrate.
pub struct CoreAudioLib;

impl Lib for CoreAudioLib {
    fn manifest(&self) -> LibManifest {
        coreaudio_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        coreaudio_pack().load(cx, linker)
    }
}

/// Installs [`CoreAudioLib`] into `cx` exactly once.
///
/// Repeated calls are idempotent: the lib's surface cards are registered on the
/// first install and skipped thereafter.
pub fn install_stream_coreaudio_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &CoreAudioLib)?;
    Ok(())
}

fn coreaudio_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "CoreAudioBackend"),
        Symbol::qualified("stream", "CoreAudioNativeFallback"),
        Symbol::qualified("stream", "MacosAudioBackendPriority"),
        Symbol::qualified("stream", "MacosMidiBackendPriority"),
    ]
}

fn coreaudio_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "CoreAudioBackend" => "CoreAudio host PCM backend card",
        "CoreAudioNativeFallback" => "CoreAudio native fallback policy card",
        "MacosAudioBackendPriority" => "macOS audio backend priority card",
        "MacosMidiBackendPriority" => "macOS MIDI backend priority card",
        _ => "CoreAudio card",
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
                SurfaceField::Symbol(coreaudio_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(coreaudio_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("audio-priority"),
                SurfaceField::Symbols(macos_audio_backend_priority()),
            ),
            (
                Symbol::new("midi-priority"),
                SurfaceField::Symbols(macos_midi_backend_priority()),
            ),
            (
                Symbol::new("native-policy"),
                SurfaceField::Str(
                    "Use native CoreAudio when PortAudio or RtAudio is insufficient".to_owned(),
                ),
            ),
        ],
    }
}

fn coreaudio_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(COREAUDIO_LIB_ID),
            values: coreaudio_symbols()
                .into_iter()
                .map(coreaudio_value_spec)
                .collect(),
        },
    }
}
