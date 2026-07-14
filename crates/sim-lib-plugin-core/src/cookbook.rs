//! Deterministic cookbook builders for plugin-core recipes.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_audio_graph_core::{PortDir, PortMedia};

use crate::{ParameterDescriptor, PluginDescriptor, PluginFormat};

/// Build the modeled gain plugin descriptor used by the cookbook recipe.
pub fn gain_plugin_demo() -> Expr {
    let descriptor =
        PluginDescriptor::audio_effect(PluginFormat::Sim, "org.sim.gain", "SIM Gain", 2)
            .expect("valid gain plugin descriptor")
            .with_parameter(
                ParameterDescriptor::new(0, "gain", "Gain", 0.0, 2.0, 1.0)
                    .expect("valid gain parameter"),
            );
    plugin_descriptor_expr(&descriptor)
}

fn plugin_descriptor_expr(descriptor: &PluginDescriptor) -> Expr {
    Expr::Map(vec![
        (field("kind"), sym("plugin-core", "descriptor")),
        (
            field("format"),
            sym("plugin-format", descriptor.id.format.as_str()),
        ),
        (field("id"), Expr::String(descriptor.id.stable_id.clone())),
        (field("name"), Expr::String(descriptor.name.clone())),
        (
            field("ports"),
            Expr::Vector(descriptor.ports.iter().map(port_expr).collect()),
        ),
        (
            field("parameters"),
            Expr::Vector(
                descriptor
                    .parameters
                    .iter()
                    .map(|parameter| {
                        Expr::Map(vec![
                            (field("id"), number(parameter.id)),
                            (
                                field("stable-id"),
                                Expr::String(parameter.stable_id.clone()),
                            ),
                            (field("default"), number_f64(parameter.default)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}

fn port_expr(port: &sim_lib_audio_graph_core::PortDecl) -> Expr {
    Expr::Map(vec![
        (field("name"), Expr::String(port.name.clone())),
        (field("media"), sym("audio-port", port_media(port.media))),
        (field("dir"), sym("audio-port", port_dir(port.dir))),
        (field("channels"), number(port.channels)),
    ])
}

fn port_media(media: PortMedia) -> &'static str {
    match media {
        PortMedia::Audio => "audio",
        PortMedia::Control => "control",
        PortMedia::Event => "event",
    }
}

fn port_dir(dir: PortDir) -> &'static str {
    match dir {
        PortDir::In => "in",
        PortDir::Out => "out",
    }
}

fn field(name: &str) -> Expr {
    Expr::Symbol(Symbol::qualified("plugin-core", name))
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

fn number_f64(value: f64) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "f64"),
        canonical: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_plugin_demo_carries_stereo_ports_and_gain_parameter() {
        let Expr::Map(entries) = gain_plugin_demo() else {
            panic!("gain plugin demo is a map")
        };
        let rendered = format!("{entries:?}");
        assert!(rendered.contains("audio-in"));
        assert!(rendered.contains("audio-out"));
        assert!(rendered.contains("gain"));
    }
}
