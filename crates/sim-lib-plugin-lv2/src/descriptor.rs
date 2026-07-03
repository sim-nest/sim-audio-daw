use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};
use sim_lib_plugin_core::{
    ParameterDescriptor, ParameterKind, PluginDescriptor, PluginFormat, PluginId,
};

/// The LV2 port class of an [`Lv2Port`], mapped onto a graph [`PortMedia`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lv2PortKind {
    /// An LV2 audio port, mapped to [`PortMedia::Audio`].
    Audio,
    /// An LV2 control port, mapped to [`PortMedia::Control`].
    Control,
    /// An LV2 atom-sequence (event) port, mapped to [`PortMedia::Event`].
    AtomSequence,
}

impl Lv2PortKind {
    fn media(self) -> PortMedia {
        match self {
            Self::Audio => PortMedia::Audio,
            Self::Control => PortMedia::Control,
            Self::AtomSequence => PortMedia::Event,
        }
    }
}

/// One declared port of an [`Lv2PluginDescriptor`].
///
/// Carries the LV2 port metadata and lowers to a graph [`PortDecl`] via
/// [`Lv2Port::to_port_decl`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lv2Port {
    /// The LV2 port index within its plugin.
    pub index: u32,
    /// The LV2 port symbol, used as the graph port name.
    pub symbol: String,
    /// The human-readable port name.
    pub name: String,
    /// The LV2 port class.
    pub kind: Lv2PortKind,
    /// The signal direction (input or output).
    pub dir: PortDir,
    /// The number of graph lanes (channels) this port exposes.
    pub channels: u16,
}

impl Lv2Port {
    /// Builds a port from its fields, validating symbol, name, and channels.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `symbol` or `name` is blank, or when
    /// `channels` is zero.
    pub fn new(
        index: u32,
        symbol: impl Into<String>,
        name: impl Into<String>,
        kind: Lv2PortKind,
        dir: PortDir,
        channels: u16,
    ) -> Result<Self> {
        let symbol = symbol.into();
        let name = name.into();
        if symbol.trim().is_empty() {
            return Err(Error::Eval("LV2 port symbol cannot be empty".to_owned()));
        }
        if name.trim().is_empty() {
            return Err(Error::Eval("LV2 port name cannot be empty".to_owned()));
        }
        if channels == 0 {
            return Err(Error::Eval(format!(
                "LV2 port {symbol} must expose at least one graph lane"
            )));
        }
        Ok(Self {
            index,
            symbol,
            name,
            kind,
            dir,
            channels,
        })
    }

    /// Builds an audio input port named "Audio Input" with `channels` lanes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `symbol` is blank or `channels` is zero.
    pub fn audio_input(index: u32, symbol: impl Into<String>, channels: u16) -> Result<Self> {
        Self::new(
            index,
            symbol,
            "Audio Input",
            Lv2PortKind::Audio,
            PortDir::In,
            channels,
        )
    }

    /// Builds an audio output port named "Audio Output" with `channels` lanes.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `symbol` is blank or `channels` is zero.
    pub fn audio_output(index: u32, symbol: impl Into<String>, channels: u16) -> Result<Self> {
        Self::new(
            index,
            symbol,
            "Audio Output",
            Lv2PortKind::Audio,
            PortDir::Out,
            channels,
        )
    }

    /// Builds a single-lane control input port with the given `name`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `symbol` or `name` is blank.
    pub fn control_input(
        index: u32,
        symbol: impl Into<String>,
        name: impl Into<String>,
    ) -> Result<Self> {
        Self::new(index, symbol, name, Lv2PortKind::Control, PortDir::In, 1)
    }

    /// Builds a single-lane atom-sequence input port named "Events In".
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `symbol` is blank.
    pub fn atom_input(index: u32, symbol: impl Into<String>) -> Result<Self> {
        Self::new(
            index,
            symbol,
            "Events In",
            Lv2PortKind::AtomSequence,
            PortDir::In,
            1,
        )
    }

    /// Lowers this port to a graph [`PortDecl`].
    pub fn to_port_decl(&self) -> PortDecl {
        PortDecl::new(
            self.symbol.clone(),
            self.kind.media(),
            self.dir,
            self.channels,
        )
    }
}

/// An LV2-shaped plugin description that lowers to a core [`PluginDescriptor`].
///
/// Collects the LV2 plugin identity, its [`Lv2Port`] declarations, and its
/// [`ParameterDescriptor`] set, then converts to the format-neutral descriptor
/// via [`Lv2PluginDescriptor::to_plugin_descriptor`].
#[derive(Clone, Debug, PartialEq)]
pub struct Lv2PluginDescriptor {
    /// The plugin's LV2 URI, used as its stable id.
    pub uri: String,
    /// The human-readable plugin name.
    pub name: String,
    /// The vendor string (defaults to `"sim"`).
    pub vendor: String,
    /// The plugin version (defaults to this crate's package version).
    pub version: String,
    /// The declared ports, in index order.
    pub ports: Vec<Lv2Port>,
    /// The declared automatable parameters.
    pub parameters: Vec<ParameterDescriptor>,
}

impl Lv2PluginDescriptor {
    /// Builds a descriptor from a `uri` and `name`, with empty ports/parameters.
    ///
    /// The vendor defaults to `"sim"` and the version to this crate's package
    /// version.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `uri` or `name` is blank.
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Result<Self> {
        let uri = uri.into();
        let name = name.into();
        if uri.trim().is_empty() {
            return Err(Error::Eval("LV2 plugin URI cannot be empty".to_owned()));
        }
        if name.trim().is_empty() {
            return Err(Error::Eval("LV2 plugin name cannot be empty".to_owned()));
        }
        Ok(Self {
            uri,
            name,
            vendor: "sim".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            ports: Vec::new(),
            parameters: Vec::new(),
        })
    }

    /// Appends `port` and returns the updated descriptor (builder style).
    pub fn with_port(mut self, port: Lv2Port) -> Self {
        self.ports.push(port);
        self
    }

    /// Appends `parameter` and returns the updated descriptor (builder style).
    pub fn with_parameter(mut self, parameter: ParameterDescriptor) -> Self {
        self.parameters.push(parameter);
        self
    }

    /// Lowers every declared port to a graph [`PortDecl`], in index order.
    pub fn port_decls(&self) -> Vec<PortDecl> {
        self.ports.iter().map(Lv2Port::to_port_decl).collect()
    }

    /// Converts this LV2 description to a format-neutral [`PluginDescriptor`].
    ///
    /// # Errors
    ///
    /// Returns an error when the core [`PluginId`] or [`PluginDescriptor`]
    /// rejects the URI, name, vendor, or version.
    pub fn to_plugin_descriptor(&self) -> Result<PluginDescriptor> {
        let mut descriptor = PluginDescriptor::new(
            PluginId::new(PluginFormat::Lv2, self.uri.clone())?,
            self.name.clone(),
            self.vendor.clone(),
            self.version.clone(),
        )?;
        descriptor.ports = self.port_decls();
        descriptor.parameters = self.parameters.clone();
        Ok(descriptor)
    }
}

/// Builds the built-in stereo gain plugin's LV2 description.
///
/// Declares a stereo audio input, a stereo audio output, and a single `gain`
/// control port backed by a float parameter ranging from 0.0 to 2.0.
///
/// # Errors
///
/// Returns an error when any port or parameter fails validation.
pub fn lv2_gain_lv2_descriptor() -> Result<Lv2PluginDescriptor> {
    Ok(
        Lv2PluginDescriptor::new("https://sim.dev/lv2/gain", "SIM LV2 Gain")?
            .with_port(Lv2Port::audio_input(0, "audio-in", 2)?)
            .with_port(Lv2Port::audio_output(1, "audio-out", 2)?)
            .with_port(Lv2Port::control_input(2, "gain", "Gain")?)
            .with_parameter(
                ParameterDescriptor::new(2, "gain", "Gain", 0.0, 2.0, 1.0)?
                    .with_kind(ParameterKind::Float),
            ),
    )
}

/// Builds the built-in gain plugin's format-neutral [`PluginDescriptor`].
///
/// Convenience over [`lv2_gain_lv2_descriptor`] followed by
/// [`Lv2PluginDescriptor::to_plugin_descriptor`].
///
/// # Errors
///
/// Returns an error when the LV2 description or its conversion fails.
pub fn lv2_gain_descriptor() -> Result<PluginDescriptor> {
    lv2_gain_lv2_descriptor()?.to_plugin_descriptor()
}
