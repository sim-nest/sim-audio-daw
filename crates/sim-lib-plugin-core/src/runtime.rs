use sim_kernel::{Cx, Result, Symbol};
use sim_lib_core::{SurfaceField, SurfacePackLib, SurfacePackSpec, SurfaceValueSpec, install_once};

const PLUGIN_CORE_LIB_ID: &str = "plugin-core";

/// Installs the plugin-core surface pack into the runtime context.
///
/// Registers one descriptor card per plugin-core export symbol; installing more
/// than once is idempotent.
///
/// # Errors
///
/// Returns an error when the surface pack fails to install.
pub fn install_plugin_core_lib(cx: &mut Cx) -> Result<()> {
    install_once(cx, &plugin_core_pack())?;
    Ok(())
}

/// Returns the qualified export symbols the plugin-core lib publishes.
pub fn plugin_core_symbols() -> Vec<Symbol> {
    [
        "PluginDescriptor",
        "ParameterDescriptor",
        "PluginState",
        "HostedPluginProcessor",
        "ProcessorPlugin",
    ]
    .into_iter()
    .map(|name| Symbol::qualified(PLUGIN_CORE_LIB_ID, name))
    .collect()
}

/// The plugin-core surface pack: one uniform descriptor card per export symbol.
fn plugin_core_pack() -> SurfacePackLib {
    SurfacePackLib {
        spec: SurfacePackSpec {
            lib_id: Symbol::new(PLUGIN_CORE_LIB_ID),
            values: plugin_core_symbols()
                .into_iter()
                .map(|symbol| SurfaceValueSpec {
                    symbol: symbol.clone(),
                    fields: vec![
                        (Symbol::new("symbol"), SurfaceField::Symbol(symbol)),
                        (
                            Symbol::new("layer"),
                            SurfaceField::Str(PLUGIN_CORE_LIB_ID.to_owned()),
                        ),
                        (
                            Symbol::new("kind"),
                            SurfaceField::Str("plugin-descriptor".to_owned()),
                        ),
                        (
                            Symbol::new("role"),
                            SurfaceField::Str(
                                "common plugin descriptor and adapter surface".to_owned(),
                            ),
                        ),
                    ],
                })
                .collect(),
        },
    }
}
