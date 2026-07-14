//! Deterministic cookbook builders for live audio-graph recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::LiveGraphConfig;

/// Build the modeled live graph config descriptor used by the cookbook recipe.
pub fn live_config_demo() -> Expr {
    let config = LiveGraphConfig::stereo(48_000, 64).expect("valid live graph config");
    Expr::Map(vec![
        (field("kind"), sym("audio-graph-live", "config")),
        (field("sample-rate"), number(config.spec().sample_rate_hz())),
        (field("input-channels"), number(config.input_channels())),
        (field("output-channels"), number(config.output_channels())),
        (field("max-block-frames"), number(config.max_block_frames())),
        (field("clock"), sym("audio-graph-live", "fake-clock")),
        (field("hardware-required"), Expr::Bool(false)),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("audio-graph-live", name))
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
    fn live_config_demo_is_bounded_stereo() {
        let Expr::Map(entries) = live_config_demo() else {
            panic!("live config demo is a map")
        };
        assert!(entries.iter().any(|(_, value)| *value == Expr::Bool(false)));
        assert!(format!("{entries:?}").contains("max-block-frames"));
    }
}
