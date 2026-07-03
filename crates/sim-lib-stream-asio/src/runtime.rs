use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

use crate::{asio_backend_symbol, asio_sdk_build_requirements, asio_transport_symbol};

const ASIO_LIB_ID: &str = "stream-asio";

/// Host-registered lib exporting the ASIO stream-host cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct AsioLib;

impl Lib for AsioLib {
    fn manifest(&self) -> LibManifest {
        asio_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        asio_pack().load(cx, linker)
    }
}

/// Installs the [`AsioLib`] into `cx` exactly once, registering its ASIO
/// stream-host surface cards.
pub fn install_stream_asio_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &AsioLib)?;
    Ok(())
}

fn asio_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("stream", "AsioBackend"),
        Symbol::qualified("stream", "AsioSdkBuildRequirements"),
        Symbol::qualified("stream", "AsioBufferSwitchBridge"),
    ]
}

fn asio_value_spec(symbol: Symbol) -> SurfaceValueSpec {
    let role = match symbol.name.as_ref() {
        "AsioBackend" => "ASIO host audio backend card",
        "AsioSdkBuildRequirements" => "ASIO optional SDK build requirements card",
        "AsioBufferSwitchBridge" => "ASIO buffer switch graph bridge card",
        _ => "ASIO card",
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
                SurfaceField::Symbol(asio_backend_symbol()),
            ),
            (
                Symbol::new("transport"),
                SurfaceField::Symbol(asio_transport_symbol()),
            ),
            (Symbol::new("role"), SurfaceField::Str(role.to_owned())),
            (
                Symbol::new("requirements"),
                SurfaceField::Strs(
                    asio_sdk_build_requirements()
                        .into_iter()
                        .map(|item| item.to_owned())
                        .collect(),
                ),
            ),
        ],
    }
}

fn asio_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(ASIO_LIB_ID),
            values: asio_symbols().into_iter().map(asio_value_spec).collect(),
        },
    }
}
