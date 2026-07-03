use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{JackClient, jack_backend_symbol, jack_clock_symbol, jack_transport_symbol};

const JACK_LIB_ID: &str = "stream-jack";

/// Host-registered lib exporting the JACK stream-host cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct JackLib;

impl Lib for JackLib {
    fn manifest(&self) -> LibManifest {
        jack_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        jack_pack().load(cx, linker)
    }
}

/// Installs the JACK stream-host lib into `cx`, registering its cards once.
///
/// Repeated calls are idempotent via the shared `install_once` guard.
///
/// # Errors
///
/// Returns an error if loading the lib into the context fails.
pub fn install_stream_jack_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &JackLib)?;
    Ok(())
}

fn jack_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "JackBackend"),
        Symbol::qualified("stream", "JackSimClient"),
        Symbol::qualified("stream", "JackClientPorts"),
        Symbol::qualified("stream", "JackTransportClock"),
    ]
}

fn jack_value_spec(symbol: Symbol) -> Result<SurfaceValueSpec> {
    let role = match symbol.name.as_ref() {
        "JackBackend" => "JACK host PCM backend card",
        "JackSimClient" => "JACK SIM client card",
        "JackClientPorts" => "JACK routable audio and MIDI port card",
        "JackTransportClock" => "JACK transport sample-frame clock card",
        _ => "JACK card",
    };
    let sim_client = JackClient::sim_default()?;
    Ok(SurfaceValueSpec {
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
                SurfaceField::Symbol(jack_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(jack_transport_symbol()),
            ),
            (
                Symbol::new("clock"),
                SurfaceField::Symbol(jack_clock_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("client"),
                SurfaceField::Symbol(sim_client.id().clone()),
            ),
            (
                Symbol::new("ports"),
                SurfaceField::Symbols(
                    sim_client
                        .ports()
                        .into_iter()
                        .map(|port| port.id().clone())
                        .collect(),
                ),
            ),
        ],
    })
}

fn jack_pack() -> SurfacePackLib {
    let values = jack_symbols()
        .into_iter()
        .map(jack_value_spec)
        .collect::<Result<Vec<_>>>()
        .expect("JACK SIM default client is well-formed");
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(JACK_LIB_ID),
            values,
        },
    }
}
