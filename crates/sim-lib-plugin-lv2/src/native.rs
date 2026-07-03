//! Provider seam for capability-gated LV2 fallback loading.

use sim_kernel::{Error, Result};
use sim_lib_audio_dsp::Gain;
use sim_lib_plugin_core::PluginInstance;

use crate::{Lv2ExportedProcessor, export_gain_as_lv2};

/// Backend that can instantiate an LV2-shaped plugin from a plugin URI.
pub trait Lv2HostProvider {
    /// The plugin instance produced by this provider.
    type Plugin: PluginInstance;

    /// Instantiates the requested LV2 plugin.
    ///
    /// # Errors
    ///
    /// Returns an error when `uri` names an unavailable plugin.
    fn instantiate(&self, uri: &str) -> Result<Self::Plugin>;
}

/// Deterministic LV2 host provider for loader and graph tests.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureLv2HostProvider {
    uri: String,
    gain: f32,
}

impl FixtureLv2HostProvider {
    /// Builds a fixture provider exposing the built-in gain plugin URI.
    pub fn gain() -> Self {
        Self {
            uri: "https://sim.dev/lv2/gain".to_owned(),
            gain: 0.5,
        }
    }

    /// Returns this provider with a different fixture gain.
    pub fn with_gain(mut self, gain: f32) -> Self {
        self.gain = gain;
        self
    }

    /// Returns this provider with a different accepted plugin URI.
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = uri.into();
        self
    }

    /// Returns the LV2 plugin URI accepted by this provider.
    pub fn uri(&self) -> &str {
        &self.uri
    }
}

impl Lv2HostProvider for FixtureLv2HostProvider {
    type Plugin = Lv2ExportedProcessor<Gain>;

    fn instantiate(&self, uri: &str) -> Result<Self::Plugin> {
        if uri != self.uri {
            return Err(Error::Eval(format!(
                "LV2 fixture URI '{uri}' is not available"
            )));
        }
        export_gain_as_lv2(self.gain)
    }
}
