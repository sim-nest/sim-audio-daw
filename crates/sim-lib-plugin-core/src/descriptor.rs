use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};

/// The host backend format a plugin is loaded through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PluginFormat {
    /// The CLAP plugin format.
    Clap,
    /// The LV2 plugin format.
    Lv2,
    /// The VST3 plugin format.
    Vst3,
    /// A WebAssembly-hosted plugin.
    Wasm,
    /// The native SIM plugin format.
    Sim,
}

impl PluginFormat {
    /// Returns the lowercase wire name for this format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clap => "clap",
            Self::Lv2 => "lv2",
            Self::Vst3 => "vst3",
            Self::Wasm => "wasm",
            Self::Sim => "sim",
        }
    }
}

/// A plugin's stable identity: its format paired with a backend-stable id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PluginId {
    /// The host backend format this id refers to.
    pub format: PluginFormat,
    /// The format-stable identifier string for the plugin.
    pub stable_id: String,
}

/// A request to load a plugin through a specific backend format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginLoadSpec {
    format: PluginFormat,
    location: String,
}

impl PluginLoadSpec {
    /// Builds a plugin load request, rejecting an empty or whitespace-only
    /// location.
    ///
    /// # Errors
    ///
    /// Returns an error when `location` is empty after trimming.
    pub fn new(format: PluginFormat, location: impl Into<String>) -> Result<Self> {
        let location = location.into();
        if location.trim().is_empty() {
            return Err(Error::Eval(
                "plugin load location cannot be empty".to_owned(),
            ));
        }
        Ok(Self { format, location })
    }

    /// Returns the backend format requested by this load.
    pub fn format(&self) -> PluginFormat {
        self.format
    }

    /// Returns the backend-specific load location.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// Requires a specific backend format for this load request.
    ///
    /// # Errors
    ///
    /// Returns a type mismatch when the requested format does not match
    /// `expected`.
    pub fn require_format(&self, expected: PluginFormat) -> Result<()> {
        if self.format == expected {
            Ok(())
        } else {
            Err(Error::TypeMismatch {
                expected: expected.as_str(),
                found: self.format.as_str(),
            })
        }
    }
}

impl PluginId {
    /// Builds a plugin id, rejecting an empty or whitespace-only stable id.
    ///
    /// # Errors
    ///
    /// Returns an error when `stable_id` is empty after trimming.
    pub fn new(format: PluginFormat, stable_id: impl Into<String>) -> Result<Self> {
        let stable_id = stable_id.into();
        if stable_id.trim().is_empty() {
            return Err(Error::Eval("plugin stable id cannot be empty".to_owned()));
        }
        Ok(Self { format, stable_id })
    }
}

/// The value domain a parameter exposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterKind {
    /// A continuous floating-point parameter.
    Float,
    /// A discrete integer-valued parameter.
    Integer,
    /// A two-state on/off parameter.
    Boolean,
}

/// A single automatable plugin parameter and its value range.
#[derive(Clone, Debug, PartialEq)]
pub struct ParameterDescriptor {
    /// The parameter's numeric id, unique within a plugin.
    pub id: u32,
    /// The backend-stable identifier string.
    pub stable_id: String,
    /// The human-readable display name.
    pub name: String,
    /// The value domain ([`ParameterKind`]).
    pub kind: ParameterKind,
    /// The inclusive minimum plain value.
    pub min: f64,
    /// The inclusive maximum plain value.
    pub max: f64,
    /// The default plain value, clamped into `min..=max`.
    pub default: f64,
    /// Whether a host may automate this parameter.
    pub automatable: bool,
}

impl ParameterDescriptor {
    /// Builds a [`ParameterKind::Float`], automatable parameter.
    ///
    /// `default` is clamped into `min..=max`.
    ///
    /// # Errors
    ///
    /// Returns an error when `stable_id` or `name` is empty after trimming, or
    /// when `min` exceeds `max`.
    pub fn new(
        id: u32,
        stable_id: impl Into<String>,
        name: impl Into<String>,
        min: f64,
        max: f64,
        default: f64,
    ) -> Result<Self> {
        let stable_id = stable_id.into();
        let name = name.into();
        if stable_id.trim().is_empty() {
            return Err(Error::Eval(
                "parameter stable id cannot be empty".to_owned(),
            ));
        }
        if name.trim().is_empty() {
            return Err(Error::Eval("parameter name cannot be empty".to_owned()));
        }
        if min > max {
            return Err(Error::Eval(format!(
                "parameter {stable_id} min {min} exceeds max {max}"
            )));
        }
        Ok(Self {
            id,
            stable_id,
            name,
            kind: ParameterKind::Float,
            min,
            max,
            default: default.clamp(min, max),
            automatable: true,
        })
    }

    /// Returns the descriptor with its [`ParameterKind`] replaced.
    pub fn with_kind(mut self, kind: ParameterKind) -> Self {
        self.kind = kind;
        self
    }

    /// Maps a plain value to the normalized `0.0..=1.0` range.
    ///
    /// The input is clamped into `min..=max` first; a zero-width range maps to
    /// `0.0`.
    pub fn plain_to_normalized(&self, value: f64) -> f64 {
        if (self.max - self.min).abs() <= f64::EPSILON {
            return 0.0;
        }
        ((value.clamp(self.min, self.max) - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    /// Maps a normalized `0.0..=1.0` value back to a plain value in
    /// `min..=max`.
    ///
    /// The input is clamped into `0.0..=1.0` first.
    pub fn normalized_to_plain(&self, normalized: f64) -> f64 {
        self.min + normalized.clamp(0.0, 1.0) * (self.max - self.min)
    }
}

/// A plugin's full static description: identity, metadata, ports, parameters,
/// and reported latency.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginDescriptor {
    /// The plugin's stable identity.
    pub id: PluginId,
    /// The human-readable plugin name.
    pub name: String,
    /// The vendor string.
    pub vendor: String,
    /// The version string.
    pub version: String,
    /// The plugin's declared audio and event ports.
    pub ports: Vec<PortDecl>,
    /// The plugin's automatable parameters.
    pub parameters: Vec<ParameterDescriptor>,
    /// The reported processing latency in frames.
    pub latency_frames: u32,
}

impl PluginDescriptor {
    /// Builds a descriptor with no ports, no parameters, and zero latency.
    ///
    /// # Errors
    ///
    /// Returns an error when `name` is empty after trimming.
    pub fn new(
        id: PluginId,
        name: impl Into<String>,
        vendor: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(Error::Eval("plugin name cannot be empty".to_owned()));
        }
        Ok(Self {
            id,
            name,
            vendor: vendor.into(),
            version: version.into(),
            ports: Vec::new(),
            parameters: Vec::new(),
            latency_frames: 0,
        })
    }

    /// Builds a stereo-capable audio-effect descriptor with the standard port
    /// layout.
    ///
    /// Adds `channels`-wide `audio-in`/`audio-out` ports plus single-channel
    /// `events-in`/`events-out` ports, and sets the vendor to `"sim"` and the
    /// version to this crate's package version.
    ///
    /// # Errors
    ///
    /// Returns an error when `stable_id` or `name` is empty after trimming.
    pub fn audio_effect(
        format: PluginFormat,
        stable_id: impl Into<String>,
        name: impl Into<String>,
        channels: u16,
    ) -> Result<Self> {
        let mut descriptor = Self::new(
            PluginId::new(format, stable_id)?,
            name,
            "sim",
            env!("CARGO_PKG_VERSION"),
        )?;
        descriptor.ports.push(PortDecl::new(
            "audio-in",
            PortMedia::Audio,
            PortDir::In,
            channels,
        ));
        descriptor.ports.push(PortDecl::new(
            "audio-out",
            PortMedia::Audio,
            PortDir::Out,
            channels,
        ));
        descriptor
            .ports
            .push(PortDecl::new("events-in", PortMedia::Event, PortDir::In, 1));
        descriptor.ports.push(PortDecl::new(
            "events-out",
            PortMedia::Event,
            PortDir::Out,
            1,
        ));
        Ok(descriptor)
    }

    /// Returns the descriptor with `parameter` appended to its parameter list.
    pub fn with_parameter(mut self, parameter: ParameterDescriptor) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Returns the parameter whose [`ParameterDescriptor::id`] matches `id`, if
    /// any.
    pub fn parameter(&self, id: u32) -> Option<&ParameterDescriptor> {
        self.parameters.iter().find(|parameter| parameter.id == id)
    }
}
