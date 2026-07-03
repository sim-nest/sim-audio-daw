use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

use crate::{Patch, PatchNode};

const LIB_NS: &str = "audio-graph";

/// Citizen descriptor wrapping a single [`PatchNode`] configuration.
///
/// The node is stored in its [`Expr`] encoding so it round-trips through the
/// citizen protocol; [`AudioGraphNodeConfig::node`] decodes it back.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "audio-graph/NodeConfig", version = 1)]
pub struct AudioGraphNodeConfig {
    #[citizen(with = "node_expr")]
    node: Expr,
}

/// Citizen descriptor wrapping a whole [`Patch`].
///
/// The patch is stored in its [`Expr`] encoding; [`AudioGraphPatchDescriptor::patch`]
/// decodes it back.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "audio-graph/Patch", version = 1)]
pub struct AudioGraphPatchDescriptor {
    #[citizen(with = "patch_expr")]
    patch: Expr,
}

impl AudioGraphNodeConfig {
    /// Wraps a patch node, validating its id is non-empty.
    pub fn new(node: PatchNode) -> Result<Self> {
        validate_node(&node)?;
        Ok(Self {
            node: node_to_expr(&node),
        })
    }

    /// Builds a config from a node expression, validating that it decodes.
    pub fn from_expr(expr: Expr) -> Result<Self> {
        node_expr::decode(&expr)?;
        Ok(Self { node: expr })
    }

    /// Decodes and returns the wrapped [`PatchNode`].
    pub fn node(&self) -> Result<PatchNode> {
        node_from_expr(&self.node)
    }

    /// Returns the underlying node expression without decoding it.
    pub fn as_expr(&self) -> &Expr {
        &self.node
    }
}

impl Default for AudioGraphNodeConfig {
    fn default() -> Self {
        Self::new(PatchNode {
            id: "citizen-node".to_owned(),
            in_channels: 2,
            out_channels: 2,
        })
        .expect("default audio graph node config should be valid")
    }
}

impl AudioGraphPatchDescriptor {
    /// Wraps a patch, validating that it round-trips through its expression.
    pub fn new(patch: Patch) -> Result<Self> {
        Patch::from_expr(&patch.to_expr())?;
        Ok(Self {
            patch: patch.to_expr(),
        })
    }

    /// Builds a descriptor from a patch expression, validating that it decodes.
    pub fn from_expr(expr: Expr) -> Result<Self> {
        patch_expr::decode(&expr)?;
        Ok(Self { patch: expr })
    }

    /// Decodes and returns the wrapped [`Patch`].
    pub fn patch(&self) -> Result<Patch> {
        Patch::from_expr(&self.patch)
    }

    /// Returns the underlying patch expression without decoding it.
    pub fn as_expr(&self) -> &Expr {
        &self.patch
    }
}

impl Default for AudioGraphPatchDescriptor {
    fn default() -> Self {
        Self::new(Patch {
            nodes: vec![
                AudioGraphNodeConfig::default()
                    .node()
                    .expect("default node should decode"),
            ],
            cables: Vec::new(),
        })
        .expect("default audio graph patch should be valid")
    }
}

/// Returns the class symbol under which node configs register as citizens.
pub fn audio_graph_node_config_class_symbol() -> Symbol {
    Symbol::qualified("audio-graph", "NodeConfig")
}

/// Returns the class symbol under which patches register as citizens.
pub fn audio_graph_patch_class_symbol() -> Symbol {
    Symbol::qualified("audio-graph", "Patch")
}

pub(crate) mod node_expr {
    use sim_kernel::{Expr, Result};

    use super::node_from_expr;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        node_from_expr(expr)?;
        Ok(expr.clone())
    }
}

pub(crate) mod patch_expr {
    use sim_kernel::{Expr, Result};

    use crate::Patch;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        Patch::from_expr(expr)?;
        Ok(expr.clone())
    }
}

fn node_to_expr(node: &PatchNode) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("node-config")),
        (field("id"), Expr::String(node.id.clone())),
        (field("in-channels"), number_u16(node.in_channels)),
        (field("out-channels"), number_u16(node.out_channels)),
    ])
}

fn node_from_expr(expr: &Expr) -> Result<PatchNode> {
    let map = expr_map(expr, "audio graph node config")?;
    expect_tag(map, "node-config")?;
    let node = PatchNode {
        id: expr_string(lookup_required(map, "id")?, "node id")?.to_owned(),
        in_channels: expr_u16(lookup_required(map, "in-channels")?, "in-channels")?,
        out_channels: expr_u16(lookup_required(map, "out-channels")?, "out-channels")?,
    };
    validate_node(&node)?;
    Ok(node)
}

fn validate_node(node: &PatchNode) -> Result<()> {
    if node.id.trim().is_empty() {
        return Err(Error::Eval(
            "audio graph node config id cannot be empty".to_owned(),
        ));
    }
    Ok(())
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
}

fn tag(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified(LIB_NS, name))
}

fn number_u16(value: u16) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
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
            "audio graph descriptor tag must be {expected}"
        ))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        _ => Err(Error::Eval(format!("{context} must be a string"))),
    }
}

fn expr_u16(expr: &Expr, context: &str) -> Result<u16> {
    let text = match expr {
        Expr::Number(number) => number.canonical.as_str(),
        Expr::String(text) => text,
        _ => return Err(Error::Eval(format!("{context} must be a number"))),
    };
    text.parse::<u16>()
        .map_err(|_| Error::Eval(format!("{context} must be a u16")))
}

fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    map.iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, name) => Some(value),
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("audio graph descriptor field is missing: {name}")))
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}
