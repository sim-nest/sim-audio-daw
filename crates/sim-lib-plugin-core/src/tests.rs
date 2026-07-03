use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};

use crate::{
    HostedPluginProcessor, ParameterDescriptor, PluginDescriptor, PluginDescriptorRecord,
    PluginFormat, PluginId, PluginInstance, PluginLoadSpec, PluginState, ProcessorPlugin,
    install_plugin_core_lib, plugin_core_symbols,
};

#[test]
fn descriptors_validate_and_normalize_parameters() {
    let parameter = ParameterDescriptor::new(7, "gain", "Gain", -60.0, 6.0, 0.0).unwrap();
    assert_eq!(parameter.plain_to_normalized(-60.0), 0.0);
    assert_eq!(parameter.plain_to_normalized(6.0), 1.0);
    assert_eq!(parameter.normalized_to_plain(0.5), -27.0);

    let descriptor =
        PluginDescriptor::audio_effect(PluginFormat::Clap, "org.sim.gain", "SIM Gain", 2)
            .unwrap()
            .with_parameter(parameter);
    assert_eq!(descriptor.id.format.as_str(), "clap");
    assert_eq!(descriptor.parameter(7).unwrap().stable_id, "gain");
    assert_eq!(descriptor.ports.len(), 4);
}

#[test]
fn citizen_plugin_descriptor_round_trips_and_fails_closed() {
    let parameter = ParameterDescriptor::new(7, "gain", "Gain", -60.0, 6.0, 0.0).unwrap();
    let descriptor =
        PluginDescriptor::audio_effect(PluginFormat::Clap, "org.sim.gain", "SIM Gain", 2)
            .unwrap()
            .with_parameter(parameter);

    let record = PluginDescriptorRecord::new(descriptor.clone());
    assert_eq!(record.descriptor().unwrap(), descriptor);

    let err = PluginDescriptorRecord::from_expr(sim_kernel::Expr::Map(Vec::new())).unwrap_err();
    assert!(format!("{err}").contains("field is missing"));
}

#[test]
fn plugin_state_round_trips_as_expr() {
    let mut state = PluginState::new();
    state.set_param(7, 0.25);
    state.insert_data("preset", sim_kernel::Expr::String("small".to_owned()));

    let decoded = PluginState::from_expr(&state.to_expr()).unwrap();
    assert_eq!(decoded, state);
}

#[test]
fn plugin_load_specs_validate_location_and_format() {
    assert!(PluginLoadSpec::new(PluginFormat::Clap, " ").is_err());

    let spec = PluginLoadSpec::new(PluginFormat::Clap, "fixture://gain").unwrap();
    assert_eq!(spec.format(), PluginFormat::Clap);
    assert_eq!(spec.location(), "fixture://gain");
    assert!(spec.require_format(PluginFormat::Clap).is_ok());
    assert!(spec.require_format(PluginFormat::Wasm).is_err());
}

#[test]
fn hosted_plugin_processor_delegates_to_instance() {
    let descriptor =
        PluginDescriptor::audio_effect(PluginFormat::Clap, "org.sim.double", "Double", 1).unwrap();
    let mut hosted = HostedPluginProcessor::new(TestPlugin::new(descriptor, 2.0));
    let output = process_mono(&mut hosted, &[0.25, -0.5, 1.0]);
    assert_eq!(round6(&output[0]), vec![0.5, -1.0, 2.0]);
}

#[test]
fn processor_plugin_exports_processor_as_plugin_instance() {
    let descriptor =
        PluginDescriptor::audio_effect(PluginFormat::Sim, "org.sim.triple", "Triple", 1).unwrap();
    let mut exported = ProcessorPlugin::new(descriptor, MultiplyProcessor::new(3.0));
    let output = process_plugin(&mut exported, &[0.25, -0.5, 1.0]);
    assert_eq!(round6(&output[0]), vec![0.75, -1.5, 3.0]);
}

#[test]
fn install_plugin_core_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    install_plugin_core_lib(&mut cx).expect("install");
    install_plugin_core_lib(&mut cx).expect("idempotent install");

    let manifest = &cx
        .registry()
        .lib(&Symbol::new("plugin-core"))
        .expect("registered")
        .manifest;
    for symbol in plugin_core_symbols() {
        assert!(
            manifest
                .exports
                .iter()
                .any(|export| *export.symbol() == symbol),
            "missing {symbol}"
        );
    }
}

#[derive(Clone, Debug)]
struct TestPlugin {
    descriptor: PluginDescriptor,
    multiplier: f32,
}

impl TestPlugin {
    fn new(descriptor: PluginDescriptor, multiplier: f32) -> Self {
        Self {
            descriptor,
            multiplier,
        }
    }
}

impl PluginInstance for TestPlugin {
    fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        MultiplyProcessor::new(self.multiplier).process(block);
    }
}

#[derive(Clone, Debug)]
struct MultiplyProcessor {
    multiplier: f32,
}

impl MultiplyProcessor {
    fn new(multiplier: f32) -> Self {
        Self { multiplier }
    }
}

impl Processor for MultiplyProcessor {
    fn prepare(&mut self, _cfg: PrepareConfig) {}

    fn reset(&mut self) {}

    fn process(&mut self, block: &mut ProcessBlock<'_>) {
        for frame in 0..block.frames as usize {
            block.out_audio[0][frame] = block.in_audio[0][frame] * self.multiplier;
        }
    }
}

fn process_plugin(plugin: &mut dyn PluginInstance, input: &[f32]) -> Vec<Vec<f32>> {
    plugin.prepare(PrepareConfig::new(48_000, input.len() as u32, 1, 1));
    process_with(|block| plugin.process(block), input)
}

fn process_mono<P: Processor>(processor: &mut P, input: &[f32]) -> Vec<Vec<f32>> {
    processor.prepare(PrepareConfig::new(48_000, input.len() as u32, 1, 1));
    process_with(|block| processor.process(block), input)
}

fn process_with<F: FnOnce(&mut ProcessBlock<'_>)>(process: F, input: &[f32]) -> Vec<Vec<f32>> {
    let mut output = vec![vec![0.0; input.len()]];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(input.len());
    let mut block = ProcessBlock {
        frames: input.len() as u32,
        in_audio: &[input],
        out_audio: &mut output_refs,
        in_events: &[],
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    process(&mut block);
    output
}

fn round6(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
        .collect()
}

#[test]
fn plugin_ids_reject_empty_stable_ids() {
    assert!(PluginId::new(PluginFormat::Clap, "").is_err());
}

#[test]
fn wasm_plugin_format_has_wire_name() {
    assert_eq!(PluginFormat::Wasm.as_str(), "wasm");
}
