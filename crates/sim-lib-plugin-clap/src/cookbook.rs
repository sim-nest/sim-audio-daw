//! Deterministic cookbook builders for CLAP plugin recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::clap_gain_descriptor;

/// Build the modeled CLAP gain descriptor used by the cookbook recipe.
pub fn clap_gain_demo() -> Expr {
    let descriptor = clap_gain_descriptor().expect("valid CLAP gain descriptor");
    Expr::Map(vec![
        (field("kind"), sym("plugin-clap", "descriptor")),
        (
            field("format"),
            sym("plugin-format", descriptor.id.format.as_str()),
        ),
        (field("id"), Expr::String(descriptor.id.stable_id)),
        (field("name"), Expr::String(descriptor.name)),
        (field("ports"), number(descriptor.ports.len())),
        (field("parameters"), number(descriptor.parameters.len())),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("plugin-clap", name))
}

fn sym(namespace: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(namespace, name))
}

fn number(value: impl ToString) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_gain_demo_uses_clap_format() {
        let Expr::Map(entries) = clap_gain_demo() else {
            panic!("CLAP demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| {
            matches!(value, Expr::Symbol(symbol) if symbol.as_qualified_str() == "plugin-format/clap")
        }));
    }
}
