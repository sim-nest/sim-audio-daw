use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{linux_audio_backend_priority, pipewire_backend_symbol, pipewire_transport_symbol};

const PIPEWIRE_LIB_ID: &str = "stream-pipewire";

/// Host-registered lib exporting the PipeWire stream-host cards, built on the
/// shared [`SurfacePackLib`] substrate.
pub struct PipeWireLib;

impl Lib for PipeWireLib {
    fn manifest(&self) -> LibManifest {
        pipewire_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        pipewire_pack().load(cx, linker)
    }
}

/// Installs [`PipeWireLib`] into `cx` exactly once.
///
/// Idempotent via `install_once`: repeated calls leave the registered lib
/// untouched.
pub fn install_stream_pipewire_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &PipeWireLib)?;
    Ok(())
}

fn pipewire_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "PipeWireBackend"),
        Symbol::qualified("stream", "PipeWireDefaultNode"),
        Symbol::qualified("stream", "PipeWireSimClientPorts"),
        Symbol::qualified("stream", "LinuxAudioBackendPriority"),
    ]
}

fn pipewire_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "PipeWireBackend" => "PipeWire host PCM backend card",
        "PipeWireDefaultNode" => "PipeWire default desktop audio node card",
        "PipeWireSimClientPorts" => "PipeWire SIM client visible ports card",
        "LinuxAudioBackendPriority" => "Linux simple audio backend priority card",
        _ => "PipeWire card",
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
                SurfaceField::Symbol(pipewire_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(pipewire_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("sim-client"),
                SurfaceField::Str("SIM".to_owned()),
            ),
            (
                Symbol::new("priority"),
                SurfaceField::Symbols(linux_audio_backend_priority()),
            ),
        ],
    }
}

fn pipewire_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(PIPEWIRE_LIB_ID),
            values: pipewire_symbols()
                .into_iter()
                .map(pipewire_value_spec)
                .collect(),
        },
    }
}
