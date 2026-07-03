use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

use crate::{Cable, PortUri};

/// Portable description of one graph node: its id and channel counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchNode {
    /// Node id, unique within the patch.
    pub id: String,
    /// Number of input channels.
    pub in_channels: u16,
    /// Number of output channels.
    pub out_channels: u16,
}

/// Portable, serializable description of a graph: its nodes and cables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    /// Nodes in the patch.
    pub nodes: Vec<PatchNode>,
    /// Cables connecting node ports.
    pub cables: Vec<Cable>,
}

impl Patch {
    /// Encodes the patch as a tagged expression map.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified("audio-graph", "patch")),
            ),
            (
                field("nodes"),
                Expr::Vector(self.nodes.iter().map(node_to_expr).collect()),
            ),
            (
                field("cables"),
                Expr::Vector(self.cables.iter().map(cable_to_expr).collect()),
            ),
        ])
    }

    /// Decodes a patch from its expression form, validating the tag and fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "patch")?;
        match lookup(map, "tag") {
            Some(Expr::Symbol(symbol)) if is_symbol(symbol, "audio-graph", "patch") => {}
            Some(_) => return Err(Error::Eval("audio graph patch tag is invalid".to_owned())),
            None => return Err(Error::Eval("audio graph patch tag is missing".to_owned())),
        }
        let nodes = expr_vector(
            lookup(map, "nodes").ok_or_else(|| missing("nodes"))?,
            "patch nodes",
        )?
        .iter()
        .map(node_from_expr)
        .collect::<Result<Vec<_>>>()?;
        let cables = expr_vector(
            lookup(map, "cables").ok_or_else(|| missing("cables"))?,
            "patch cables",
        )?
        .iter()
        .map(cable_from_expr)
        .collect::<Result<Vec<_>>>()?;
        Ok(Self { nodes, cables })
    }
}

fn node_to_expr(node: &PatchNode) -> Expr {
    Expr::Map(vec![
        (field("id"), Expr::String(node.id.clone())),
        (field("in-channels"), number(node.in_channels)),
        (field("out-channels"), number(node.out_channels)),
    ])
}

fn node_from_expr(expr: &Expr) -> Result<PatchNode> {
    let map = expr_map(expr, "patch node")?;
    Ok(PatchNode {
        id: expr_string(
            lookup(map, "id").ok_or_else(|| missing("node id"))?,
            "node id",
        )?
        .to_owned(),
        in_channels: expr_u16(
            lookup(map, "in-channels").ok_or_else(|| missing("node in-channels"))?,
            "node in-channels",
        )?,
        out_channels: expr_u16(
            lookup(map, "out-channels").ok_or_else(|| missing("node out-channels"))?,
            "node out-channels",
        )?,
    })
}

fn cable_to_expr(cable: &Cable) -> Expr {
    Expr::Map(vec![
        (field("from"), Expr::String(cable.from.to_string())),
        (field("to"), Expr::String(cable.to.to_string())),
    ])
}

fn cable_from_expr(expr: &Expr) -> Result<Cable> {
    let map = expr_map(expr, "patch cable")?;
    let from = expr_string(
        lookup(map, "from").ok_or_else(|| missing("cable from"))?,
        "cable from",
    )?
    .parse::<PortUri>()?;
    let to = expr_string(
        lookup(map, "to").ok_or_else(|| missing("cable to"))?,
        "cable to",
    )?
    .parse::<PortUri>()?;
    Ok(Cable { from, to })
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym("audio-graph", name)
}

fn number(value: u16) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

fn expr_map<'a>(expr: &'a Expr, context: &str) -> Result<&'a [(Expr, Expr)]> {
    match expr {
        Expr::Map(entries) => Ok(entries),
        other => Err(Error::Eval(format!(
            "expected {context} map, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_vector<'a>(expr: &'a Expr, context: &str) -> Result<&'a [Expr]> {
    match expr {
        Expr::Vector(items) => Ok(items),
        other => Err(Error::Eval(format!(
            "expected {context} vector, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} string, found {}",
            expr_kind(other)
        ))),
    }
}

fn expr_u16(expr: &Expr, context: &str) -> Result<u16> {
    match expr {
        Expr::Number(number) => number.canonical.parse::<u16>().map_err(|_| {
            Error::Eval(format!(
                "expected {context} u16 number, found {}",
                number.canonical
            ))
        }),
        Expr::String(text) => text
            .parse::<u16>()
            .map_err(|_| Error::Eval(format!("expected {context} u16 string, found {text}"))),
        other => Err(Error::Eval(format!(
            "expected {context} u16, found {}",
            expr_kind(other)
        ))),
    }
}

fn lookup<'a>(map: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    map.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if is_symbol(symbol, "audio-graph", name) => Some(value),
        _ => None,
    })
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}

fn missing(field: &str) -> Error {
    Error::Eval(format!("audio graph patch field is missing: {field}"))
}

use sim_value::kind::expr_kind;
