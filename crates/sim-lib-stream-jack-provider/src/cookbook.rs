//! Deterministic cookbook builders for JACK provider recipes.

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_host::AUDIO_PROVIDER_ENTRY_V1;

use crate::{default_modeled_jack_site, jack_provider_symbol};

/// Build the modeled loadable JACK provider descriptor.
pub fn jack_loadable_modeled_provider_demo() -> Expr {
    let site = default_modeled_jack_site();
    let card = site.card();
    Expr::Map(vec![
        (
            field("kind"),
            sym("stream-jack-provider", "loadable-modeled-provider"),
        ),
        (field("provider"), Expr::Symbol(jack_provider_symbol())),
        (field("site"), Expr::Symbol(card.key.0.clone())),
        (field("channels-out"), number(card.channels_out)),
        (field("channels-in"), number(card.channels_in)),
        (
            field("hardware-required"),
            Expr::Bool(card.hardware_required),
        ),
    ])
}

/// Build the modeled JACK provider load descriptor.
pub fn jack_provider_load_demo() -> Expr {
    Expr::Map(vec![
        (field("kind"), sym("stream-jack-provider", "load")),
        (
            field("capability"),
            sym("capability", "audio.provider.native"),
        ),
        (
            field("loader"),
            sym("stream-jack-provider", "loader-registry"),
        ),
        (
            field("entry"),
            Expr::String(AUDIO_PROVIDER_ENTRY_V1.to_owned()),
        ),
        (field("site"), sym("audio/provider", "jack-modeled")),
        (field("fallback"), sym("stream-jack-provider", "modeled")),
    ])
}

/// Build the guarded JACK hardware-smoke descriptor.
pub fn jack_hardware_smoke_demo() -> Expr {
    Expr::Map(vec![
        (field("kind"), sym("stream-jack-provider", "hardware-smoke")),
        (
            field("guard"),
            Expr::String("SIM_JACK_HARDWARE_SMOKE".to_owned()),
        ),
        (field("command"), Expr::String("cargo-test".to_owned())),
        (field("expected"), sym("stream-jack-provider", "test-ok")),
        (
            field("default"),
            sym("stream-jack-provider", "modeled-only"),
        ),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("stream-jack-provider", name))
}

fn sym(namespace: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(namespace, name))
}

fn number(value: impl ToString) -> Expr {
    Expr::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modeled_provider_demo_needs_no_hardware() {
        let Expr::Map(entries) = jack_loadable_modeled_provider_demo() else {
            panic!("JACK modeled provider demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| *value == Expr::Bool(false)));
    }
}
