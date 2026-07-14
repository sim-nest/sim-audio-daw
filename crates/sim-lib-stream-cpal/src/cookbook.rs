//! Deterministic cookbook builders for cpal stream recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::{CpalModeledSite, cpal_modeled_provider_symbol};

/// Build the modeled cpal site descriptor used by the cookbook recipe.
pub fn cpal_modeled_site_demo() -> Expr {
    let request = CpalModeledSite::playback_request(8).expect("valid modeled cpal request");
    Expr::Map(vec![
        (field("kind"), sym("stream-cpal", "modeled-site")),
        (
            field("provider"),
            Expr::Symbol(cpal_modeled_provider_symbol()),
        ),
        (field("backend"), Expr::Symbol(request.backend().clone())),
        (field("device"), Expr::Symbol(request.device().clone())),
        (field("media"), Expr::Symbol(request.media().symbol())),
        (
            field("direction"),
            Expr::Symbol(request.direction().symbol()),
        ),
        (field("capacity"), number(request.buffer().capacity())),
        (field("hardware-required"), Expr::Bool(false)),
    ])
}

/// Build the guarded cpal hardware-smoke descriptor.
pub fn cpal_hardware_smoke_demo() -> Expr {
    Expr::Map(vec![
        (field("kind"), sym("stream-cpal", "hardware-smoke")),
        (
            field("guard"),
            Expr::String("SIM_CPAL_HARDWARE_SMOKE".to_owned()),
        ),
        (field("command"), Expr::String("cargo-test".to_owned())),
        (field("expected"), sym("stream-cpal", "test-ok")),
        (field("default"), sym("stream-cpal", "modeled-only")),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("stream-cpal", name))
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
    fn cpal_modeled_site_demo_uses_fake_backend_without_hardware() {
        let Expr::Map(entries) = cpal_modeled_site_demo() else {
            panic!("modeled cpal demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| *value == Expr::Bool(false)));
        assert!(entries.iter().any(|(_, value)| {
            matches!(value, Expr::Symbol(symbol) if symbol.as_qualified_str() == "stream/media/pcm")
        }));
    }
}
