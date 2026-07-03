use sim_kernel::{Cx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const LV2_PLUGIN_LIB_ID: &str = "plugin-lv2";

/// Installs the `plugin-lv2` surface pack into `cx`, idempotently.
///
/// Registers one adapter card per export symbol so the LV2 host and export
/// adapters are discoverable from the runtime. Repeated calls are a no-op.
///
/// # Errors
///
/// Returns an error when registration into the context fails.
pub fn install_lv2_plugin_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &lv2_plugin_pack())?;
    Ok(())
}

/// Returns the qualified [`Symbol`] for each `plugin-lv2` runtime export.
pub fn lv2_plugin_symbols() -> Vec<Symbol> {
    [
        "Lv2Port",
        "Lv2PluginDescriptor",
        "Lv2StatePatch",
        "Lv2HostProcessor",
        "Lv2ExportedProcessor",
        "Lv2GainFixture",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(LV2_PLUGIN_LIB_ID, name))
    .collect()
}

/// The plugin-lv2 surface pack: one uniform adapter card per export symbol.
fn lv2_plugin_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(LV2_PLUGIN_LIB_ID),
            values: lv2_plugin_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(LV2_PLUGIN_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("lv2-plugin-adapter".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str("LV2-shaped host and export adapter".to_owned()),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
