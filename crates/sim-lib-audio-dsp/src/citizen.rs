use sim_citizen_derive::Citizen;
use sim_kernel::{Error, Expr, NumberLiteral, Result, Symbol};

const LIB_NS: &str = "audio-dsp";

/// Citizen descriptor for a DSP processor configuration: a kind name plus a
/// list of named `f64` parameters.
///
/// The configuration is stored in its [`Expr`] encoding so it round-trips
/// through the citizen protocol.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "audio-dsp/Config", version = 1)]
pub struct DspConfigDescriptor {
    #[citizen(with = "config_expr")]
    config: Expr,
}

impl DspConfigDescriptor {
    /// Builds a config descriptor, validating the kind and parameters.
    pub fn new(kind: impl Into<String>, params: Vec<(String, f64)>) -> Result<Self> {
        let kind = kind.into();
        validate_kind(&kind)?;
        validate_params(&params)?;
        Ok(Self {
            config: config_to_expr(&kind, &params),
        })
    }

    /// Builds a `gain` config descriptor with a single `gain` parameter.
    pub fn gain(gain: f64) -> Result<Self> {
        Self::new("gain", vec![("gain".to_owned(), gain)])
    }

    /// Builds a descriptor from a config expression, validating that it decodes.
    pub fn from_expr(expr: Expr) -> Result<Self> {
        config_expr::decode(&expr)?;
        Ok(Self { config: expr })
    }

    /// Decodes and returns the config kind name.
    pub fn kind(&self) -> Result<String> {
        let (kind, _) = config_from_expr(&self.config)?;
        Ok(kind)
    }

    /// Decodes and returns the config parameters as name/value pairs.
    pub fn params(&self) -> Result<Vec<(String, f64)>> {
        let (_, params) = config_from_expr(&self.config)?;
        Ok(params)
    }

    /// Returns the underlying config expression without decoding it.
    pub fn as_expr(&self) -> &Expr {
        &self.config
    }
}

impl Default for DspConfigDescriptor {
    fn default() -> Self {
        Self::gain(1.0).expect("default DSP config descriptor should be valid")
    }
}

/// Returns the class symbol under which DSP configs register as citizens.
pub fn dsp_config_class_symbol() -> Symbol {
    Symbol::qualified("audio-dsp", "Config")
}

pub(crate) mod config_expr {
    use sim_kernel::{Expr, Result};

    use super::config_from_expr;

    pub fn encode(expr: &Expr) -> Expr {
        expr.clone()
    }

    pub fn decode(expr: &Expr) -> Result<Expr> {
        config_from_expr(expr)?;
        Ok(expr.clone())
    }
}

fn config_to_expr(kind: &str, params: &[(String, f64)]) -> Expr {
    Expr::Map(vec![
        (field("tag"), tag("config")),
        (field("kind"), Expr::Symbol(Symbol::qualified(LIB_NS, kind))),
        (
            field("params"),
            Expr::Vector(
                params
                    .iter()
                    .map(|(key, value)| {
                        Expr::Map(vec![
                            (field("key"), Expr::String(key.clone())),
                            (field("value"), number_f64(*value)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn config_from_expr(expr: &Expr) -> Result<(String, Vec<(String, f64)>)> {
    let map = expr_map(expr, "DSP config descriptor")?;
    expect_tag(map, "config")?;
    let kind = match lookup_required(map, "kind")? {
        Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(LIB_NS) => {
            symbol.name.to_string()
        }
        Expr::String(text) => text.clone(),
        _ => return Err(Error::Eval("DSP config kind must be a symbol".to_owned())),
    };
    validate_kind(&kind)?;
    let params = params_from_expr(lookup_required(map, "params")?)?;
    validate_params(&params)?;
    Ok((kind, params))
}

fn params_from_expr(expr: &Expr) -> Result<Vec<(String, f64)>> {
    let Expr::Vector(items) = expr else {
        return Err(Error::Eval("DSP config params must be a vector".to_owned()));
    };
    items
        .iter()
        .map(|item| {
            let map = expr_map(item, "DSP config parameter")?;
            Ok((
                expr_string(lookup_required(map, "key")?, "parameter key")?.to_owned(),
                expr_f64(lookup_required(map, "value")?, "parameter value")?,
            ))
        })
        .collect()
}

fn validate_kind(kind: &str) -> Result<()> {
    if kind.trim().is_empty() {
        return Err(Error::Eval("DSP config kind cannot be empty".to_owned()));
    }
    Ok(())
}

fn validate_params(params: &[(String, f64)]) -> Result<()> {
    for (key, value) in params {
        if key.trim().is_empty() {
            return Err(Error::Eval(
                "DSP config parameter key cannot be empty".to_owned(),
            ));
        }
        if !value.is_finite() {
            return Err(Error::Eval(format!(
                "DSP config parameter {key} must be finite"
            )));
        }
    }
    Ok(())
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
}

fn tag(name: &'static str) -> Expr {
    Expr::Symbol(Symbol::qualified(LIB_NS, name))
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
        _ => Err(Error::Eval(format!("{context} must be a map"))),
    }
}

fn expect_tag(map: &[(Expr, Expr)], expected: &str) -> Result<()> {
    match lookup_required(map, "tag")? {
        Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, expected) => Ok(()),
        _ => Err(Error::Eval(format!("DSP config tag must be {expected}"))),
    }
}

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        _ => Err(Error::Eval(format!("{context} must be a string"))),
    }
}

fn expr_f64(expr: &Expr, context: &str) -> Result<f64> {
    let text = match expr {
        Expr::Number(number) => number.canonical.as_str(),
        Expr::String(text) => text,
        _ => return Err(Error::Eval(format!("{context} must be a number"))),
    };
    let value = text
        .parse::<f64>()
        .map_err(|_| Error::Eval(format!("{context} must be an f64")))?;
    if !value.is_finite() {
        return Err(Error::Eval(format!("{context} must be finite")));
    }
    Ok(value)
}

fn lookup_required<'a>(map: &'a [(Expr, Expr)], name: &str) -> Result<&'a Expr> {
    map.iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if is_symbol(symbol, LIB_NS, name) => Some(value),
            _ => None,
        })
        .ok_or_else(|| Error::Eval(format!("DSP config field is missing: {name}")))
}

fn is_symbol(symbol: &Symbol, namespace: &str, name: &str) -> bool {
    symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name
}
