//! Format-dispatching plugin loader.

use std::path::Path;
#[cfg(any(feature = "clap-host", all(feature = "lv2-host", target_os = "linux")))]
use std::sync::Arc;

use sim_kernel::{Error, Result};
#[cfg(feature = "wasm-plugin")]
use sim_lib_plugin_core::AudioPluginCapability;
use sim_lib_plugin_core::{CapabilitySet, PluginInstance};

#[cfg(feature = "wasm-plugin")]
use crate::WasmPluginProcessor;
use crate::WasmResourceLimits;

#[cfg(feature = "clap-host")]
type ClapRoute = dyn Fn(&Path, &CapabilitySet) -> Result<RoutedPlugin> + Send + Sync + 'static;
#[cfg(all(feature = "lv2-host", target_os = "linux"))]
type Lv2Route = dyn Fn(&Path, &CapabilitySet) -> Result<RoutedPlugin> + Send + Sync + 'static;

/// Format-agnostic plugin instance returned by [`PluginRouter`].
pub type RoutedPlugin = Box<dyn PluginInstance>;

/// Dispatches plugin load requests to the wasm, CLAP, or LV2 loader.
#[derive(Clone)]
pub struct PluginRouter {
    limits: WasmResourceLimits,
    #[cfg(feature = "clap-host")]
    clap_route: Option<Arc<ClapRoute>>,
    #[cfg(all(feature = "lv2-host", target_os = "linux"))]
    lv2_route: Option<Arc<Lv2Route>>,
}

impl PluginRouter {
    /// Builds a router with default wasm loading and no native provider
    /// overrides.
    pub fn new(limits: WasmResourceLimits) -> Self {
        PluginRouterBuilder::new(limits).build()
    }

    /// Starts configuring a plugin router.
    pub fn builder(limits: WasmResourceLimits) -> PluginRouterBuilder {
        PluginRouterBuilder::new(limits)
    }

    /// Loads a plugin by inspecting `path`'s extension.
    ///
    /// # Errors
    ///
    /// Returns an eval error when the extension is unrecognised, when the
    /// selected feature is disabled, when no native provider is configured for
    /// a native route, or when the selected loader rejects the plugin.
    pub fn load(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        match path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("wasm") => self.load_wasm(path, caps),
            Some("clap") => self.load_clap(path, caps),
            Some("lv2") => self.load_lv2(path, caps),
            other => Err(Error::Eval(format!(
                "unrecognised plugin extension {other:?}; expected .wasm, .clap, or .lv2"
            ))),
        }
    }

    #[cfg(feature = "wasm-plugin")]
    fn load_wasm(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        caps.require(AudioPluginCapability::WasmPlugin)?;
        let bytes = std::fs::read(path)
            .map_err(|err| Error::Eval(format!("cannot read {}: {err}", path.display())))?;
        let plugin = WasmPluginProcessor::from_bytes_with_limits(&bytes, self.limits)?;
        Ok(Box::new(plugin))
    }

    #[cfg(not(feature = "wasm-plugin"))]
    fn load_wasm(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        let _ = (path, caps, self.limits);
        Err(Error::Eval("wasm-plugin feature not enabled".to_owned()))
    }

    #[cfg(feature = "clap-host")]
    fn load_clap(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        let Some(route) = &self.clap_route else {
            return Err(Error::Eval("clap-host provider not configured".to_owned()));
        };
        route(path, caps)
    }

    #[cfg(not(feature = "clap-host"))]
    fn load_clap(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        let _ = (path, caps);
        Err(Error::Eval("clap-host feature not enabled".to_owned()))
    }

    #[cfg(all(feature = "lv2-host", target_os = "linux"))]
    fn load_lv2(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        let Some(route) = &self.lv2_route else {
            return Err(Error::Eval("lv2-host provider not configured".to_owned()));
        };
        route(path, caps)
    }

    #[cfg(not(all(feature = "lv2-host", target_os = "linux")))]
    fn load_lv2(&self, path: &Path, caps: &CapabilitySet) -> Result<RoutedPlugin> {
        let _ = (path, caps);
        Err(Error::Eval(
            "lv2-host feature not enabled or not Linux".to_owned(),
        ))
    }
}

/// Configures native provider overrides for [`PluginRouter`].
pub struct PluginRouterBuilder {
    limits: WasmResourceLimits,
    #[cfg(feature = "clap-host")]
    clap_route: Option<Arc<ClapRoute>>,
    #[cfg(all(feature = "lv2-host", target_os = "linux"))]
    lv2_route: Option<Arc<Lv2Route>>,
}

impl PluginRouterBuilder {
    /// Builds a router configuration with default wasm resource limits.
    pub fn new(limits: WasmResourceLimits) -> Self {
        Self {
            limits,
            #[cfg(feature = "clap-host")]
            clap_route: None,
            #[cfg(all(feature = "lv2-host", target_os = "linux"))]
            lv2_route: None,
        }
    }

    /// Installs a CLAP provider override used for `.clap` paths.
    #[cfg(feature = "clap-host")]
    pub fn with_clap_provider<P>(mut self, provider: P) -> Self
    where
        P: sim_lib_plugin_clap::native::ClapHostProvider + Send + Sync + 'static,
        P::Plugin: 'static,
    {
        self.clap_route = Some(Arc::new(move |path, caps| {
            let spec = sim_lib_plugin_core::PluginLoadSpec::new(
                sim_lib_plugin_core::PluginFormat::Clap,
                path.to_string_lossy().into_owned(),
            )?;
            let plugin = crate::native_fallback::load_clap_plugin(caps, &spec, &provider)?;
            Ok(Box::new(plugin))
        }));
        self
    }

    /// Installs an LV2 provider override used for `.lv2` bundle paths.
    #[cfg(all(feature = "lv2-host", target_os = "linux"))]
    pub fn with_lv2_provider<P>(mut self, provider: P) -> Self
    where
        P: sim_lib_plugin_lv2::native::Lv2HostProvider + Send + Sync + 'static,
        P::Plugin: 'static,
    {
        self.lv2_route = Some(Arc::new(move |path, caps| {
            let plugin = crate::native_fallback::load_lv2_plugin(
                caps,
                path.to_string_lossy().as_ref(),
                &provider,
            )?;
            Ok(Box::new(plugin))
        }));
        self
    }

    /// Finishes router construction.
    pub fn build(self) -> PluginRouter {
        PluginRouter {
            limits: self.limits,
            #[cfg(feature = "clap-host")]
            clap_route: self.clap_route,
            #[cfg(all(feature = "lv2-host", target_os = "linux"))]
            lv2_route: self.lv2_route,
        }
    }
}
