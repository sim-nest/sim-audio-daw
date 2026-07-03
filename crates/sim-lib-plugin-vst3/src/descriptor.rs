use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};
use sim_lib_plugin_core::{
    ParameterDescriptor, ParameterKind, PluginDescriptor, PluginFormat, PluginId,
};

/// The medium carried by a VST3 bus.
///
/// Maps onto the graph port media when a [`Vst3Bus`] is lowered to a `PortDecl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vst3BusKind {
    /// An audio bus carrying sample lanes.
    Audio,
    /// An event bus carrying note and control events.
    Event,
}

impl Vst3BusKind {
    fn media(self) -> PortMedia {
        match self {
            Self::Audio => PortMedia::Audio,
            Self::Event => PortMedia::Event,
        }
    }
}

/// A VST3 input or output bus.
///
/// Describes one audio or event bus by name, direction, and lane count;
/// [`to_port_decl`](Vst3Bus::to_port_decl) lowers it into a graph `PortDecl`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vst3Bus {
    /// The display name of the bus.
    pub name: String,
    /// Whether the bus carries audio or events.
    pub kind: Vst3BusKind,
    /// The data direction (input or output) of the bus.
    pub dir: PortDir,
    /// The number of graph lanes (channels) the bus exposes.
    pub channels: u16,
}

impl Vst3Bus {
    /// Builds a bus, validating that `name` is non-empty and `channels` is
    /// non-zero.
    ///
    /// Returns an error when the name is blank or when no lane is requested.
    pub fn new(
        name: impl Into<String>,
        kind: Vst3BusKind,
        dir: PortDir,
        channels: u16,
    ) -> Result<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(Error::Eval("VST3 bus name cannot be empty".to_owned()));
        }
        if channels == 0 {
            return Err(Error::Eval(format!(
                "VST3 bus {name} must expose at least one graph lane"
            )));
        }
        Ok(Self {
            name,
            kind,
            dir,
            channels,
        })
    }

    /// Builds an audio input bus named `name` with `channels` lanes.
    pub fn audio_input(name: impl Into<String>, channels: u16) -> Result<Self> {
        Self::new(name, Vst3BusKind::Audio, PortDir::In, channels)
    }

    /// Builds an audio output bus named `name` with `channels` lanes.
    pub fn audio_output(name: impl Into<String>, channels: u16) -> Result<Self> {
        Self::new(name, Vst3BusKind::Audio, PortDir::Out, channels)
    }

    /// Builds a single-lane event input bus named `name`.
    pub fn event_input(name: impl Into<String>) -> Result<Self> {
        Self::new(name, Vst3BusKind::Event, PortDir::In, 1)
    }

    /// Lowers this bus into a graph `PortDecl` with matching media, direction,
    /// and lane count.
    pub fn to_port_decl(&self) -> PortDecl {
        PortDecl::new(
            self.name.clone(),
            self.kind.media(),
            self.dir,
            self.channels,
        )
    }
}

/// A VST3 parameter declaration.
///
/// Captures the host-facing identity and value range of one automatable
/// parameter; [`to_parameter_descriptor`](Vst3ParamInfo::to_parameter_descriptor)
/// lowers it into the shared plugin-core `ParameterDescriptor`.
#[derive(Clone, Debug, PartialEq)]
pub struct Vst3ParamInfo {
    /// The numeric VST3 parameter id.
    pub id: u32,
    /// The stable string id used for persistence and lookup.
    pub stable_id: String,
    /// The human-readable parameter title.
    pub title: String,
    /// The display units, empty when the parameter is unitless.
    pub units: String,
    /// The minimum parameter value.
    pub min: f64,
    /// The maximum parameter value.
    pub max: f64,
    /// The default value, clamped into the `min..=max` range.
    pub default: f64,
    /// Whether the host may automate the parameter.
    pub automatable: bool,
}

impl Vst3ParamInfo {
    /// Builds a parameter, validating ids and range.
    ///
    /// Requires non-empty `stable_id` and `title` and `min <= max`; `default`
    /// is clamped into the range, `units` starts empty, and `automatable`
    /// defaults to `true`. Returns an error when validation fails.
    pub fn new(
        id: u32,
        stable_id: impl Into<String>,
        title: impl Into<String>,
        min: f64,
        max: f64,
        default: f64,
    ) -> Result<Self> {
        let stable_id = stable_id.into();
        let title = title.into();
        if stable_id.trim().is_empty() {
            return Err(Error::Eval(
                "VST3 parameter stable id cannot be empty".to_owned(),
            ));
        }
        if title.trim().is_empty() {
            return Err(Error::Eval(
                "VST3 parameter title cannot be empty".to_owned(),
            ));
        }
        if min > max {
            return Err(Error::Eval(format!(
                "VST3 parameter {stable_id} min {min} exceeds max {max}"
            )));
        }
        Ok(Self {
            id,
            stable_id,
            title,
            units: String::new(),
            min,
            max,
            default: default.clamp(min, max),
            automatable: true,
        })
    }

    /// Lowers this parameter into a plugin-core `ParameterDescriptor` with a
    /// `Float` kind.
    pub fn to_parameter_descriptor(&self) -> Result<ParameterDescriptor> {
        Ok(ParameterDescriptor::new(
            self.id,
            self.stable_id.clone(),
            self.title.clone(),
            self.min,
            self.max,
            self.default,
        )?
        .with_kind(ParameterKind::Float))
    }
}

/// A VST3-shaped plugin descriptor.
///
/// Collects the class identity, bus layout, and parameter set of a VST3 export;
/// [`to_plugin_descriptor`](Vst3PluginDescriptor::to_plugin_descriptor) lowers
/// it into the shared plugin-core `PluginDescriptor` consumed by the host.
#[derive(Clone, Debug, PartialEq)]
pub struct Vst3PluginDescriptor {
    /// The VST3 class id identifying the plugin.
    pub class_id: String,
    /// The plugin display name.
    pub name: String,
    /// The vendor name (defaults to `sim`).
    pub vendor: String,
    /// The plugin version (defaults to this crate's package version).
    pub version: String,
    /// The declared input and output buses.
    pub buses: Vec<Vst3Bus>,
    /// The declared parameters.
    pub parameters: Vec<Vst3ParamInfo>,
}

impl Vst3PluginDescriptor {
    /// Builds an empty descriptor with the given `class_id` and `name`.
    ///
    /// Validates that both are non-empty, sets `vendor` to `sim` and `version`
    /// to this crate's package version, and starts with no buses or parameters.
    pub fn new(class_id: impl Into<String>, name: impl Into<String>) -> Result<Self> {
        let class_id = class_id.into();
        let name = name.into();
        if class_id.trim().is_empty() {
            return Err(Error::Eval("VST3 class id cannot be empty".to_owned()));
        }
        if name.trim().is_empty() {
            return Err(Error::Eval("VST3 plugin name cannot be empty".to_owned()));
        }
        Ok(Self {
            class_id,
            name,
            vendor: "sim".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            buses: Vec::new(),
            parameters: Vec::new(),
        })
    }

    /// Appends `bus` to the descriptor and returns it, for builder chaining.
    pub fn with_bus(mut self, bus: Vst3Bus) -> Self {
        self.buses.push(bus);
        self
    }

    /// Appends `parameter` to the descriptor and returns it, for builder
    /// chaining.
    pub fn with_parameter(mut self, parameter: Vst3ParamInfo) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Lowers every declared bus into a graph `PortDecl`.
    pub fn port_decls(&self) -> Vec<PortDecl> {
        self.buses.iter().map(Vst3Bus::to_port_decl).collect()
    }

    /// Lowers this descriptor into a plugin-core `PluginDescriptor`.
    ///
    /// Builds a `Vst3`-format `PluginId` from the class id and populates the
    /// resulting descriptor's ports and parameters from this descriptor's buses
    /// and parameter set. Returns an error if any conversion fails.
    pub fn to_plugin_descriptor(&self) -> Result<PluginDescriptor> {
        let mut descriptor = PluginDescriptor::new(
            PluginId::new(PluginFormat::Vst3, self.class_id.clone())?,
            self.name.clone(),
            self.vendor.clone(),
            self.version.clone(),
        )?;
        descriptor.ports = self.port_decls();
        descriptor.parameters = self
            .parameters
            .iter()
            .map(Vst3ParamInfo::to_parameter_descriptor)
            .collect::<Result<Vec<_>>>()?;
        Ok(descriptor)
    }
}

/// Builds the built-in VST3 gain fixture descriptor.
///
/// Declares stereo audio input and output buses, an event input bus, and a
/// single `Gain` parameter ranging `0.0..=2.0` with a default of `1.0`.
pub fn vst3_gain_vst3_descriptor() -> Result<Vst3PluginDescriptor> {
    Ok(
        Vst3PluginDescriptor::new("53494d2d4741494e2d56535433000001", "SIM VST3 Gain")?
            .with_bus(Vst3Bus::audio_input("audio-in", 2)?)
            .with_bus(Vst3Bus::audio_output("audio-out", 2)?)
            .with_bus(Vst3Bus::event_input("events-in")?)
            .with_parameter(Vst3ParamInfo::new(0, "gain", "Gain", 0.0, 2.0, 1.0)?),
    )
}

/// Builds the gain fixture as a plugin-core `PluginDescriptor`.
///
/// Convenience over [`vst3_gain_vst3_descriptor`] that immediately lowers the
/// fixture with [`Vst3PluginDescriptor::to_plugin_descriptor`].
pub fn vst3_gain_descriptor() -> Result<PluginDescriptor> {
    vst3_gain_vst3_descriptor()?.to_plugin_descriptor()
}
