use std::collections::BTreeSet;

use sim_kernel::{CapabilityName, Error, Result};

/// Privileged audio plugin operations exposed by host adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AudioPluginCapability {
    /// Loading WebAssembly audio plugins.
    WasmPlugin,
    /// Loading native audio plugins.
    NativePlugin,
}

impl AudioPluginCapability {
    /// Returns the stable kernel capability name for this plugin operation.
    pub fn as_capability_name(self) -> CapabilityName {
        match self {
            Self::WasmPlugin => CapabilityName::new("plugin.audio.wasm"),
            Self::NativePlugin => CapabilityName::new("plugin.audio.native"),
        }
    }
}

/// Granted audio plugin capabilities for loader entry points.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilitySet {
    granted: BTreeSet<AudioPluginCapability>,
}

impl CapabilitySet {
    /// Builds an empty plugin capability set.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Builds a plugin capability set containing one capability.
    pub fn with(capability: AudioPluginCapability) -> Self {
        Self::empty().grant(capability)
    }

    /// Returns this set with `capability` granted.
    pub fn grant(mut self, capability: AudioPluginCapability) -> Self {
        self.granted.insert(capability);
        self
    }

    /// Grants `capability` in place.
    pub fn insert(&mut self, capability: AudioPluginCapability) {
        self.granted.insert(capability);
    }

    /// Reports whether `capability` is granted.
    pub fn contains(&self, capability: AudioPluginCapability) -> bool {
        self.granted.contains(&capability)
    }

    /// Requires `capability`, returning a structured capability error when absent.
    pub fn require(&self, capability: AudioPluginCapability) -> Result<()> {
        if self.contains(capability) {
            Ok(())
        } else {
            Err(Error::CapabilityDenied {
                capability: capability.as_capability_name(),
            })
        }
    }
}
