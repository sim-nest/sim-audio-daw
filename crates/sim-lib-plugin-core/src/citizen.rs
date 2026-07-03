use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};
use sim_lib_audio_graph_core::{PortDecl, PortDir, PortMedia};

use crate::{ParameterDescriptor, ParameterKind, PluginDescriptor, PluginFormat, PluginId};

const LIB_NS: &str = "plugin-core";

/// A runtime citizen wrapping a [`PluginDescriptor`] in its encoded [`Expr`]
/// form.
///
/// The record stores the descriptor as a `plugin-core/PluginDescriptor`-tagged
/// expression so it can live in the object graph as a first-class citizen, and
/// converts back to a typed [`PluginDescriptor`] on demand.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "plugin-core/PluginDescriptor", version = 1)]
pub struct PluginDescriptorRecord {
    #[citizen(with = "plugin_descriptor_expr")]
    descriptor: Expr,
}

impl PluginDescriptorRecord {
    /// Builds a record by encoding a typed [`PluginDescriptor`].
    pub fn new(descriptor: PluginDescriptor) -> Self {
        Self {
            descriptor: descriptor_to_expr(&descriptor),
        }
    }

    /// Builds a record from an already-encoded descriptor expression.
    ///
    /// # Errors
    ///
    /// Returns an error when `expr` does not decode to a valid plugin
    /// descriptor.
    pub fn from_expr(expr: Expr) -> Result<Self> {
        plugin_descriptor_expr::decode(&expr)?;
        Ok(Self { descriptor: expr })
    }

    /// Decodes the held expression back into a typed [`PluginDescriptor`].
    ///
    /// # Errors
    ///
    /// Returns an error when the stored expression is not a valid descriptor.
    pub fn descriptor(&self) -> Result<PluginDescriptor> {
        descriptor_from_expr(&self.descriptor)
    }

    /// Returns the underlying encoded descriptor expression.
    pub fn as_expr(&self) -> &Expr {
        &self.descriptor
    }
}

impl Default for PluginDescriptorRecord {
    fn default() -> Self {
        let descriptor =
            PluginDescriptor::audio_effect(PluginFormat::Sim, "org.sim.citizen", "Citizen", 2)
                .expect("default plugin descriptor should be valid")
                .with_parameter(
                    ParameterDescriptor::new(0, "gain", "Gain", 0.0, 2.0, 1.0)
                        .expect("default plugin parameter should be valid"),
                );
        Self::new(descriptor)
    }
}

/// Returns the class symbol (`plugin-core/PluginDescriptor`) under which
/// [`PluginDescriptorRecord`] is registered.
pub fn plugin_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("plugin-core", "PluginDescriptor")
}

pub(crate) mod plugin_descriptor_expr {
    use sim_kernel::{Expr, Result};

    use super::descriptor_from_expr;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        descriptor_from_expr(expr)?;
        Ok(expr.clone())
    }
}

fn descriptor_to_expr(descriptor: &PluginDescriptor) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("descriptor")),
        (
            field("format"),
            Expr::String(descriptor.id.format.as_str().to_owned()),
        ),
        (
            field("stable-id"),
            Expr::String(descriptor.id.stable_id.clone()),
        ),
        (field("name"), Expr::String(descriptor.name.clone())),
        (field("vendor"), Expr::String(descriptor.vendor.clone())),
        (field("version"), Expr::String(descriptor.version.clone())),
        (
            field("ports"),
            Expr::Vector(descriptor.ports.iter().map(port_to_expr).collect()),
        ),
        (
            field("parameters"),
            Expr::Vector(
                descriptor
                    .parameters
                    .iter()
                    .map(parameter_to_expr)
                    .collect(),
            ),
        ),
        (
            field("latency-frames"),
            number_u32(descriptor.latency_frames),
        ),
    ])
}

fn descriptor_from_expr(expr: &Expr) -> Result<PluginDescriptor> {
    let map = expr_map(expr, "plugin descriptor")?;
    expect_tag(map, "descriptor")?;
    let id = PluginId::new(
        plugin_format(expr_string(lookup_required(map, "format")?, "format")?)?,
        expr_string(lookup_required(map, "stable-id")?, "stable-id")?.to_owned(),
    )?;
    let mut descriptor = PluginDescriptor::new(
        id,
        expr_string(lookup_required(map, "name")?, "name")?.to_owned(),
        expr_string(lookup_required(map, "vendor")?, "vendor")?.to_owned(),
        expr_string(lookup_required(map, "version")?, "version")?.to_owned(),
    )?;
    descriptor.ports = expr_vector(lookup_required(map, "ports")?, "ports")?
        .iter()
        .map(port_from_expr)
        .collect::<Result<Vec<_>>>()?;
    descriptor.parameters = expr_vector(lookup_required(map, "parameters")?, "parameters")?
        .iter()
        .map(parameter_from_expr)
        .collect::<Result<Vec<_>>>()?;
    descriptor.latency_frames = expr_u32(lookup_required(map, "latency-frames")?, "latency")?;
    Ok(descriptor)
}

fn port_to_expr(port: &PortDecl) -> Expr {
    Expr::Map(vec![
        (field("name"), Expr::String(port.name.clone())),
        (
            field("media"),
            Expr::String(port_media_name(port.media).to_owned()),
        ),
        (
            field("dir"),
            Expr::String(port_dir_name(port.dir).to_owned()),
        ),
        (field("channels"), number_u16(port.channels)),
    ])
}

fn port_from_expr(expr: &Expr) -> Result<PortDecl> {
    let map = expr_map(expr, "plugin port")?;
    let channels = expr_u16(lookup_required(map, "channels")?, "port channels")?;
    if channels == 0 {
        return Err(Error::Eval(
            "plugin port channel count must be greater than zero".to_owned(),
        ));
    }
    Ok(PortDecl::new(
        expr_string(lookup_required(map, "name")?, "port name")?.to_owned(),
        port_media(expr_string(lookup_required(map, "media")?, "port media")?)?,
        port_dir(expr_string(lookup_required(map, "dir")?, "port direction")?)?,
        channels,
    ))
}

fn parameter_to_expr(parameter: &ParameterDescriptor) -> Expr {
    Expr::Map(vec![
        (field("id"), number_u32(parameter.id)),
        (
            field("stable-id"),
            Expr::String(parameter.stable_id.clone()),
        ),
        (field("name"), Expr::String(parameter.name.clone())),
        (
            field("kind"),
            Expr::String(parameter_kind_name(parameter.kind).to_owned()),
        ),
        (field("min"), number_f64(parameter.min)),
        (field("max"), number_f64(parameter.max)),
        (field("default"), number_f64(parameter.default)),
        (field("automatable"), Expr::Bool(parameter.automatable)),
    ])
}

fn parameter_from_expr(expr: &Expr) -> Result<ParameterDescriptor> {
    let map = expr_map(expr, "plugin parameter")?;
    let mut parameter = ParameterDescriptor::new(
        expr_u32(lookup_required(map, "id")?, "parameter id")?,
        expr_string(lookup_required(map, "stable-id")?, "parameter stable id")?.to_owned(),
        expr_string(lookup_required(map, "name")?, "parameter name")?.to_owned(),
        expr_f64(lookup_required(map, "min")?, "parameter min")?,
        expr_f64(lookup_required(map, "max")?, "parameter max")?,
        expr_f64(lookup_required(map, "default")?, "parameter default")?,
    )?
    .with_kind(parameter_kind(expr_string(
        lookup_required(map, "kind")?,
        "parameter kind",
    )?)?);
    parameter.automatable = expr_bool(lookup_required(map, "automatable")?, "automatable")?;
    Ok(parameter)
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
}

fn tag(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified(LIB_NS, name))
}

fn number_u16(value: u16) -> Expr {
    number_u32(u32::from(value))
}

fn number_u32(value: u32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

fn number_f64(value: f64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}

fn plugin_format(text: &str) -> Result<PluginFormat> {
    match text {
        "clap" => Ok(PluginFormat::Clap),
        "lv2" => Ok(PluginFormat::Lv2),
        "vst3" => Ok(PluginFormat::Vst3),
        "wasm" => Ok(PluginFormat::Wasm),
        "sim" => Ok(PluginFormat::Sim),
        _ => Err(Error::Eval(format!("unknown plugin format: {text}"))),
    }
}

fn port_media_name(media: PortMedia) -> &'static str {
    match media {
        PortMedia::Audio => "audio",
        PortMedia::Control => "control",
        PortMedia::Event => "event",
    }
}

fn port_media(text: &str) -> Result<PortMedia> {
    match text {
        "audio" => Ok(PortMedia::Audio),
        "control" => Ok(PortMedia::Control),
        "event" => Ok(PortMedia::Event),
        _ => Err(Error::Eval(format!("unknown plugin port media: {text}"))),
    }
}

fn port_dir_name(dir: PortDir) -> &'static str {
    match dir {
        PortDir::In => "in",
        PortDir::Out => "out",
    }
}

fn port_dir(text: &str) -> Result<PortDir> {
    match text {
        "in" => Ok(PortDir::In),
        "out" => Ok(PortDir::Out),
        _ => Err(Error::Eval(format!(
            "unknown plugin port direction: {text}"
        ))),
    }
}

fn parameter_kind_name(kind: ParameterKind) -> &'static str {
    match kind {
        ParameterKind::Float => "float",
        ParameterKind::Integer => "integer",
        ParameterKind::Boolean => "boolean",
    }
}

fn parameter_kind(text: &str) -> Result<ParameterKind> {
    match text {
        "float" => Ok(ParameterKind::Float),
        "integer" => Ok(ParameterKind::Integer),
        "boolean" => Ok(ParameterKind::Boolean),
        _ => Err(Error::Eval(format!(
            "unknown plugin parameter kind: {text}"
        ))),
    }
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        _ => Err(Error::Eval(format!("{context} must be a map"))),
    }
}

fn expect_tag(map: &[(Expr, Expr)], expected: &str) -> Result<()> {
    match lookup_required(map, "tag")? {
        Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, expected) => Ok(()),
        _ => Err(Error::Eval(format!(
            "plugin descriptor tag must be {expected}"
        ))),
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        _ => Err(Error::Eval(format!("{context} must be a vector"))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        _ => Err(Error::Eval(format!("{context} must be a string"))),
    }
}

fn expr_bool(expr: &Expr, context: &str) -> Result<bool> {
    match expr {
        Expr::Bool(value) => Ok(*value),
        _ => Err(Error::Eval(format!("{context} must be a bool"))),
    }
}

fn expr_u16(expr: &Expr, context: &str) -> Result<u16> {
    expr_u32(expr, context)?
        .try_into()
        .map_err(|_| Error::Eval(format!("{context} is out of range for u16")))
}

fn expr_u32(expr: &Expr, context: &str) -> Result<u32> {
    let text = number_text(expr, context)?;
    text.parse::<u32>()
        .map_err(|_| Error::Eval(format!("{context} must be a u32")))
}

fn expr_f64(expr: &Expr, context: &str) -> Result<f64> {
    let text = number_text(expr, context)?;
    let value = text
        .parse::<f64>()
        .map_err(|_| Error::Eval(format!("{context} must be an f64")))?;
    if !value.is_finite() {
        return Err(Error::Eval(format!("{context} must be finite")));
    }
    Ok(value)
}

fn number_text<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Number(number) => Ok(number.canonical.as_str()),
        Expr::String(text) => Ok(text),
        _ => Err(Error::Eval(format!("{context} must be a number"))),
    }
}

fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    map.iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, name) => Some(value),
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("plugin descriptor field is missing: {name}")))
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}
