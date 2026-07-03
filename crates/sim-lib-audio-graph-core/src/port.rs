use std::{fmt, str::FromStr};

use sim_kernel::{Error, Result};
use sim_lib_stream_core::RateContract;

/// Kind of signal a port carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortMedia {
    /// Sample-rate audio.
    Audio,
    /// Control-rate parameter signals.
    Control,
    /// Discrete events (for example MIDI).
    Event,
}

/// Direction of a port relative to its node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortDir {
    /// Input port.
    In,
    /// Output port.
    Out,
}

/// Declaration of one port on a processor: name, media, direction, channel
/// count, and rate contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortDecl {
    /// Port name, unique among a node's ports of the same direction.
    pub name: String,
    /// Media kind carried by the port.
    pub media: PortMedia,
    /// Port direction.
    pub dir: PortDir,
    /// Channel count.
    pub channels: u16,
    /// Rate contract the port advertises.
    pub rate_contract: RateContract,
}

impl PortDecl {
    /// Creates a port declaration with the media kind's default rate contract.
    pub fn new(name: impl Into<String>, media: PortMedia, dir: PortDir, channels: u16) -> Self {
        Self {
            name: name.into(),
            media,
            dir,
            channels,
            rate_contract: media.default_rate_contract(),
        }
    }

    /// Returns the declaration with its rate contract overridden.
    pub fn with_rate_contract(mut self, rate_contract: RateContract) -> Self {
        self.rate_contract = rate_contract;
        self
    }
}

impl PortMedia {
    /// Returns the default rate contract for this media kind: sample-exact for
    /// audio, control-rate for control and event media.
    pub fn default_rate_contract(self) -> RateContract {
        match self {
            Self::Audio => RateContract::sample_exact(None),
            Self::Control | Self::Event => RateContract::control(),
        }
    }
}

/// A structured URI identifying a single channel of a port.
///
/// Graph ports use the `sim-node://graph/<graph-id>/<node-id>/<port-name>:<index>`
/// form; [`PortUri::node`] builds that shape and [`PortUri::node_id`] /
/// [`PortUri::node_port_name`] read it back.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortUri {
    /// URI scheme (for example `sim-node`).
    pub scheme: String,
    /// URI authority (for example `graph`).
    pub authority: String,
    /// Path segments after the authority.
    pub path: Vec<String>,
    /// Channel index within the port.
    pub index: u32,
}

impl PortUri {
    /// Creates and validates a port URI from its parts.
    pub fn new(
        scheme: impl Into<String>,
        authority: impl Into<String>,
        path: Vec<String>,
        index: u32,
    ) -> Result<Self> {
        let uri = Self {
            scheme: scheme.into(),
            authority: authority.into(),
            path,
            index,
        };
        uri.validate()?;
        Ok(uri)
    }

    /// Builds a graph node port URI in the `sim-node://graph/...` form.
    pub fn node(
        graph_id: impl Into<String>,
        node_id: impl Into<String>,
        port_name: impl Into<String>,
        index: u32,
    ) -> Result<Self> {
        Self::new(
            "sim-node",
            "graph",
            vec![graph_id.into(), node_id.into(), port_name.into()],
            index,
        )
    }

    /// Returns the node id if this is a graph node port URI.
    pub fn node_id(&self) -> Option<&str> {
        (self.scheme == "sim-node" && self.authority == "graph" && self.path.len() >= 3)
            .then_some(self.path[1].as_str())
    }

    /// Returns the port name if this is a graph node port URI.
    pub fn node_port_name(&self) -> Option<&str> {
        (self.scheme == "sim-node" && self.authority == "graph" && self.path.len() >= 3)
            .then_some(self.path[2].as_str())
    }

    fn validate(&self) -> Result<()> {
        if self.scheme.is_empty() {
            return Err(Error::Eval("port URI scheme cannot be empty".to_owned()));
        }
        if self.scheme.contains(':') || self.scheme.contains('/') {
            return Err(Error::Eval(format!(
                "port URI scheme contains an invalid separator: {}",
                self.scheme
            )));
        }
        if self.authority.is_empty() {
            return Err(Error::Eval("port URI authority cannot be empty".to_owned()));
        }
        if self.authority.contains('/') {
            return Err(Error::Eval(format!(
                "port URI authority contains an invalid separator: {}",
                self.authority
            )));
        }
        if let Some(segment) = self.path.iter().find(|segment| segment.is_empty()) {
            return Err(Error::Eval(format!(
                "port URI path segment cannot be empty: {segment}"
            )));
        }
        if let Some(segment) = self.path.iter().find(|segment| segment.contains('/')) {
            return Err(Error::Eval(format!(
                "port URI path segment contains an invalid separator: {segment}"
            )));
        }
        Ok(())
    }
}

impl fmt::Display for PortUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}://{}", self.scheme, self.authority)?;
        for segment in &self.path {
            write!(f, "/{segment}")?;
        }
        write!(f, ":{}", self.index)
    }
}

impl FromStr for PortUri {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let (scheme, rest) = text
            .split_once("://")
            .ok_or_else(|| Error::Eval(format!("port URI is missing scheme: {text}")))?;
        let (address, index_text) = rest
            .rsplit_once(':')
            .ok_or_else(|| Error::Eval(format!("port URI is missing port index: {text}")))?;
        let index = index_text
            .parse::<u32>()
            .map_err(|_| Error::Eval(format!("port URI has invalid port index: {text}")))?;
        let mut segments = address.split('/');
        let authority = segments
            .next()
            .ok_or_else(|| Error::Eval(format!("port URI is missing authority: {text}")))?;
        let path = segments.map(str::to_owned).collect::<Vec<_>>();
        Self::new(scheme, authority, path, index)
    }
}
