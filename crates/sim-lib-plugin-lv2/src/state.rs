use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_plugin_core::PluginState;
use sim_value::kind::expr_kind;

const LIB_NS: &str = "plugin-lv2";

/// A portable snapshot of one LV2 plugin's state, keyed by its URI.
///
/// Pairs a plugin URI with a [`PluginState`] and round-trips through the shared
/// [`Expr`] graph via [`Lv2StatePatch::to_expr`] and
/// [`Lv2StatePatch::from_expr`].
#[derive(Clone, Debug, PartialEq)]
pub struct Lv2StatePatch {
    plugin_uri: String,
    state: PluginState,
}

impl Lv2StatePatch {
    /// Builds a state patch for `plugin_uri` carrying `state`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `plugin_uri` is blank.
    pub fn new(plugin_uri: impl Into<String>, state: PluginState) -> Result<Self> {
        let plugin_uri = plugin_uri.into();
        if plugin_uri.trim().is_empty() {
            return Err(Error::Eval(
                "LV2 state patch plugin URI cannot be empty".to_owned(),
            ));
        }
        Ok(Self { plugin_uri, state })
    }

    /// Returns the plugin URI this patch applies to.
    pub fn plugin_uri(&self) -> &str {
        &self.plugin_uri
    }

    /// Returns the captured plugin state.
    pub fn state(&self) -> &PluginState {
        &self.state
    }

    /// Encodes this patch as a tagged [`Expr::Map`] in the `plugin-lv2` namespace.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                field("tag"),
                Expr::Symbol(Symbol::qualified(LIB_NS, "state-patch")),
            ),
            (field("uri"), Expr::String(self.plugin_uri.clone())),
            (field("state"), self.state.to_expr()),
        ])
    }

    /// Decodes a patch from an [`Expr`] emitted by [`Lv2StatePatch::to_expr`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Eval`] when `expr` is not a map, its tag is missing or
    /// wrong, or the `uri`/`state` fields are absent or ill-typed.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let map = expr_map(expr, "LV2 state patch")?;
        match lookup(map, "tag") {
            Some(Expr::Symbol(symbol)) if is_symbol(symbol, LIB_NS, "state-patch") => {}
            Some(_) => return Err(Error::Eval("LV2 state patch tag is invalid".to_owned())),
            None => return Err(missing("tag")),
        }
        Self::new(
            expr_string(lookup_required(map, "uri")?, "LV2 plugin URI")?.to_owned(),
            PluginState::from_expr(lookup_required(map, "state")?)?,
        )
    }
}

fn field(name: &'static str) -> Expr {
    sim_value::build::qsym(LIB_NS, name)
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

fn expr_string<'a>(expr: &'a Expr, context: &str) -> Result<&'a str> {
    match expr {
        Expr::String(text) => Ok(text),
        other => Err(Error::Eval(format!(
            "expected {context} string, found {}",
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
    Error::Eval(format!("LV2 state patch field is missing: {field}"))
}
