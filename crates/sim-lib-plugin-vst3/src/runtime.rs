use sim_kernel::{Cx, Lib, LibManifest, Linker, LoadCx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const VST3_PLUGIN_LIB_ID: &str = "plugin-vst3";

/// Host-registered lib exporting the VST3 adapter cards, built on the shared
/// [`SurfacePackLib`] substrate.
pub struct Vst3PluginLib;

impl Lib for Vst3PluginLib {
    fn manifest(&self) -> LibManifest {
        vst3_plugin_pack().manifest()
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        vst3_plugin_pack().load(cx, linker)
    }
}

/// Installs the [`Vst3PluginLib`] into `cx`, registering its adapter cards once.
///
/// Idempotent: a second call with the same `cx` is a no-op.
pub fn install_vst3_plugin_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &Vst3PluginLib)?;
    Ok(())
}

/// Returns the qualified symbols this lib exports, one per VST3 adapter card.
pub fn vst3_plugin_symbols() -> Vec<Symbol> {
    [
        "Vst3PluginDescriptor",
        "Vst3ParamInfo",
        "Vst3Event",
        "Vst3ParamMap",
        "Vst3ExportedProcessor",
        "Vst3GainFixture",
        "Vst3ScopeDecision",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(VST3_PLUGIN_LIB_ID, name))
    .collect()
}

fn vst3_plugin_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(VST3_PLUGIN_LIB_ID),
            values: vst3_plugin_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(VST3_PLUGIN_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("vst3-plugin-adapter".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str(
                                "VST3-shaped export adapter and SDK scope record".to_owned(),
                            ),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
