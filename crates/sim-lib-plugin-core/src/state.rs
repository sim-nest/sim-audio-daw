use std::collections::BTreeMap;

use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};
use sim_value::kind::expr_kind;

const LIB_NS: &str = "plugin-core";

/// A plugin's persistable state: parameter values plus opaque keyed data.
///
/// Parameters are keyed by their numeric id and opaque data by string key, both
/// in sorted [`BTreeMap`]s so serialization is deterministic. The state
/// round-trips through an [`Expr`] map via [`PluginState::to_expr`] and
/// [`PluginState::from_expr`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginState {
    params: BTreeMap<u32, f64>,
    data: BTreeMap<String, Expr>,
}

impl PluginState {
    /// Creates an empty state with no parameters and no data.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the parameter id-to-value map.
    pub fn params(&self) -> &BTreeMap<u32, f64> {
        &self.params
    }

    /// Returns the opaque key-to-[`Expr`] data map.
    pub fn data(&self) -> &BTreeMap<String, Expr> {
        &self.data
    }

    /// Sets the value of the parameter with the given id, inserting it if
    /// absent.
    pub fn set_param(&mut self, id: u32, value: f64) {
        self.params.insert(id, value);
    }

    /// Returns the stored value for the parameter id, if present.
    pub fn param(&self, id: u32) -> Option<f64> {
        self.params.get(&id).copied()
    }

    /// Inserts or replaces an opaque data entry under `key`.
    pub fn insert_data(&mut self, key: impl Into<String>, value: Expr) {
        self.data.insert(key.into(), value);
    }

    /// Encodes the state as a tagged [`Expr`] map.
    ///
    /// The result is a `plugin-core/state`-tagged map carrying `params` and
    /// `data` vectors; [`PluginState::from_expr`] reverses it.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified(LIB_NS, "state")),
            ),
            (
                field("params"),
                Expr::Vector(
                    self.params
                        .iter()
                        .map(|(id, value)| {
                            Expr::Map(vec![
                                (field("id"), number_u32(*id)),
                                (field("value"), number_f64(*value)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                field("data"),
                Expr::Vector(
                    self.data
                        .iter()
                        .map(|(key, value)| {
                            Expr::Map(vec![
                                (field("key"), Expr::String(key.clone())),
                                (field("value"), value.clone()),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    /// Decodes a state produced by [`PluginState::to_expr`].
    ///
    /// # Errors
    ///
    /// Returns an error when `expr` is not a `plugin-core/state`-tagged map or
    /// any required field has the wrong shape or numeric type.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "plugin state")?;
        match lookup(map, "tag") {
            Some(Expr::Symbol(symbol)) if is_symbol(symbol, LIB_NS, "state") => {}
            Some(_) => return Err(Error::Eval("plugin state tag is invalid".to_owned())),
            None => return Err(missing("tag")),
        }
        let mut state = Self::new();
        for entry in expr_vector(lookup_required(map, "params")?, "params")? {
            let entry = expr_map(entry, "param entry")?;
            state.set_param(
                expr_u32(lookup_required(entry, "id")?, "param id")?,
                expr_f64(lookup_required(entry, "value")?, "param value")?,
            );
        }
        for entry in expr_vector(lookup_required(map, "data")?, "data")? {
            let entry = expr_map(entry, "data entry")?;
            state.insert_data(
                expr_string(lookup_required(entry, "key")?, "data key")?.to_owned(),
                lookup_required(entry, "value")?.clone(),
            );
        }
        Ok(state)
    }
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
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

fn expr_u32(expr: &Expr, context: &str) -> Result<u32> {
    let text = number_text(expr, context)?;
    text.parse::<u32>()
        .map_err(|_| Error::Eval(format!("expected {context} u32 number, found {text}")))
}

fn expr_f64(expr: &Expr, context: &str) -> Result<f64> {
    let text = number_text(expr, context)?;
    text.parse::<f64>()
        .map_err(|_| Error::Eval(format!("expected {context} f64 number, found {text}")))
}

fn number_text<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::Number(number) => Ok(number.canonical.as_str()),
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} number, found {}",
            expr_kind(other)
        ))),
    }
}

fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    lookup(map, name).ok_or_else(|| missing(name))
}

fn lookup<'a>(map: &'a [(Expr, Expr)], name: &str) -> Option<&'a Expr> {
    map.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, name) => Some(value),
        _ => None,
    })
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}

fn missing(field: &str) -> Error {
    Error::Eval(format!("plugin state field is missing: {field}"))
}
