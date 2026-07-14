//! Deterministic cookbook builders for VST3 plugin recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::vst3_gain_vst3_descriptor;

/// Build the modeled VST3 gain descriptor used by the cookbook recipe.
pub fn vst3_gain_demo() -> Expr {
    let descriptor = vst3_gain_vst3_descriptor().expect("valid VST3 gain descriptor");
    let plugin = descriptor
        .to_plugin_descriptor()
        .expect("VST3 gain lowers to plugin-core descriptor");
    Expr::Map(vec![
        (field("kind"), sym("plugin-vst3", "descriptor")),
        (field("class-id"), Expr::String(descriptor.class_id)),
        (field("name"), Expr::String(descriptor.name)),
        (field("buses"), number(descriptor.buses.len())),
        (field("parameters"), number(descriptor.parameters.len())),
        (
            field("format"),
            sym("plugin-format", plugin.id.format.as_str()),
        ),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("plugin-vst3", name))
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
    fn vst3_gain_demo_lowers_to_core_plugin_descriptor() {
        let Expr::Map(entries) = vst3_gain_demo() else {
            panic!("VST3 demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| {
            matches!(value, Expr::Symbol(symbol) if symbol.as_qualified_str() == "plugin-format/vst3")
        }));
    }
}
