use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const CLAP_PLUGIN_LIB_ID: &str = "plugin-clap";

/// Host-registered lib exporting the CLAP adapter cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct ClapPluginLib;

impl Lib for ClapPluginLib {
    fn manifest(&self) -> LibManifest {
        clap_plugin_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        clap_plugin_pack().load(cx, linker)
    }
}

/// Installs [`ClapPluginLib`] into `cx`, idempotently.
///
/// Repeated calls are a no-op via the shared `install_once` guard.
pub fn install_clap_plugin_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &ClapPluginLib)?;
    Ok(())
}

/// Returns the qualified [`Symbol`]s of the cards exported by the CLAP plugin
/// lib (the event, parameter-map, host/export processor, and fixture cards).
pub fn clap_plugin_symbols() -> Vec<Symbol> {
    [
        "ClapEvent",
        "ClapParamMap",
        "ClapHostProcessor",
        "ClapExportedProcessor",
        "ClapGainFixture",
        "ClapSynthFixture",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(CLAP_PLUGIN_LIB_ID, name))
    .collect()
}

fn clap_plugin_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(CLAP_PLUGIN_LIB_ID),
            values: clap_plugin_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(CLAP_PLUGIN_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("clap-plugin-adapter".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str("CLAP-shaped host and export adapter".to_owned()),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
