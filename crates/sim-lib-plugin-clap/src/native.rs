//! Provider seam for capability-gated CLAP fallback loading.

use sim_kernel::{Error, Result};
use sim_lib_audio_dsp::Gain;
use sim_lib_plugin_core::{PluginFormat, PluginInstance, PluginLoadSpec};

use crate::{ClapExportedProcessor, export_gain_as_clap};

/// Backend that can instantiate a CLAP-shaped plugin from a load request.
pub trait ClapHostProvider {
    /// The plugin instance produced by this provider.
    type Plugin: PluginInstance;

    /// Instantiates the requested CLAP plugin.
    ///
    /// # Errors
    ///
    /// Returns an error when `spec` names an unsupported format or location.
    fn instantiate(&self, spec: &PluginLoadSpec) -> Result<Self::Plugin>;
}

/// Deterministic CLAP host provider for loader and graph tests.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureClapHostProvider {
    location: String,
    gain: f32,
}

impl FixtureClapHostProvider {
    /// Builds a fixture provider exposing `fixture://gain`.
    pub fn gain() -> Self {
        Self {
            location: "fixture://gain".to_owned(),
            gain: 0.5,
        }
    }

    /// Returns this provider with a different fixture gain.
    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    /// Returns this provider with a different accepted fixture location.
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = location.into();
        self
    }

    /// Returns the fixture location accepted by this provider.
    pub fn location(&self) -> &str {
        &self.location
    }
}

impl ClapHostProvider for FixtureClapHostProvider {
    type Plugin = ClapExportedProcessor<Gain>;

    fn instantiate(&self, spec: &PluginLoadSpec) -> Result<Self::Plugin> {
        spec.require_format(PluginFormat::Clap)?;
        if spec.location() != self.location {
            return Err(Error::Eval(format!(
                "CLAP fixture location '{}' is not available",
                spec.location()
            )));
        }
        export_gain_as_clap(self.gain)
    }
}
