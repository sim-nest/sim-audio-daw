//! Deterministic cookbook builders for audio-DSP recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::DspConfigDescriptor;

/// Build the modeled offline DSP chain descriptor used by the cookbook recipe.
pub fn offline_chain_demo() -> Expr {
    Expr::Map(vec![
        (field("kind"), sym("audio-dsp", "offline-chain")),
        (
            field("processors"),
            Expr::Vector(vec![
                DspConfigDescriptor::gain(0.75)
                    .expect("valid gain config")
                    .as_expr()
                    .clone(),
                config("delay", "time-ms", 125.0),
                config("filter", "cutoff-hz", 1_200.0),
                config("compressor", "threshold-db", -18.0),
            ]),
        ),
        (field("fixture"), sym("audio-dsp", "offline-fixture")),
    ])
}

/// Build the deterministic audio-processing trace used by the 30-agent recipe.
pub fn audio_processing_trace_demo() -> Expr {
    list(vec![
        sym_plain("audio-processing-trace"),
        list(vec![sym_plain("id"), sym_plain("a30-020-audio-processing")]),
        list(vec![
            sym_plain("fixture"),
            list(vec![sym_plain("source"), sym_plain("synthetic-waveform")]),
            list(vec![sym_plain("media"), sym_plain("copied-no")]),
            list(vec![sym_plain("sample-rate"), number(48000)]),
            list(vec![sym_plain("duration-ms"), number(250)]),
        ]),
        list(vec![
            sym_plain("synthesis"),
            list(vec![sym_plain("component"), sym_plain("sine-hz-440")]),
            list(vec![
                sym_plain("component"),
                sym_plain("noise-floor-minus-48db"),
            ]),
            list(vec![sym_plain("window"), sym_plain("hann")]),
        ]),
        list(vec![
            sym_plain("features"),
            list(vec![sym_plain("rms-percent"), number(42)]),
            list(vec![
                sym_plain("spectrum"),
                list(vec![sym_plain("peak-hz"), number(440)]),
                list(vec![sym_plain("centroid-hz"), number(560)]),
                list(vec![
                    sym_plain("band-energy"),
                    sym_plain("low-18"),
                    sym_plain("mid-71"),
                    sym_plain("high-11"),
                ]),
            ]),
        ]),
        list(vec![
            sym_plain("fake-asr"),
            list(vec![sym_plain("runner"), sym_plain("fake-asr")]),
            list(vec![sym_plain("transcript"), sym_plain("steady-test-tone")]),
            list(vec![
                sym_plain("segment"),
                number(0),
                number(250),
                sym_plain("steady-test-tone"),
            ]),
        ]),
        list(vec![
            sym_plain("prosody"),
            list(vec![sym_plain("loudness"), sym_plain("moderate")]),
            list(vec![sym_plain("pitch"), sym_plain("stable")]),
            list(vec![sym_plain("pace"), sym_plain("even")]),
        ]),
        list(vec![
            sym_plain("answer"),
            sym_plain("steady-tone-with-moderate-loudness"),
        ]),
        list(vec![
            sym_plain("effect-ledger"),
            effect("synth-fixture", "local"),
            effect("compute-rms", "pass"),
            effect("compute-spectrum", "pass"),
            effect("fake-asr-transcript", "deterministic"),
        ]),
    ])
}

fn config(kind: &str, key: &str, value: f64) -> Expr {
    DspConfigDescriptor::new(kind, vec![(key.to_owned(), value)])
        .expect("valid cookbook DSP config")
        .as_expr()
        .clone()
}

fn effect(action: &str, result: &str) -> Expr {
    list(vec![
        sym_plain("effect"),
        sym_plain(action),
        sym_plain(result),
    ])
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("audio-dsp", name))
}

fn sym(namespace: &str, name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified(namespace, name))
}

fn sym_plain(name: &str) -> Expr {
    Expr::Symbol(Symbol::new(name))
}

fn number(value: impl ToString) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "i64"),
        canonical: value.to_string(),
    })
}

fn list(items: Vec<Expr>) -> Expr {
    Expr::List(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_chain_demo_contains_four_processors() {
        let Expr::Map(entries) = offline_chain_demo() else {
            panic!("offline chain demo is a map")
        };
        assert!(matches!(
            entries.iter().find(|(key, _)| matches!(key, Expr::Symbol(symbol) if symbol.name.as_ref() == "processors")),
            Some((_, Expr::Vector(items))) if items.len() == 4
        ));
    }
}
