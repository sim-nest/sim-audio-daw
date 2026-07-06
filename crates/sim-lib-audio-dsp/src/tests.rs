use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_audio_graph_live::{LiveGraphConfig, LiveGraphRunner};

use crate::{
    AllPassFilter, BiquadFilter, Chorus, CombFilter, Compressor, DcBlocker, DelayProcessor,
    DspConfigDescriptor, Flanger, FractionalDelay, Gain, Gate, Limiter, OnePoleFilter,
    OversampledSoftClipper, Pan, SmoothedGain, SoftClipper, StateVariableFilter, StateVariableMode,
    Vibrato, Waveshape, Waveshaper, audio_dsp_symbols, install_audio_dsp_lib,
    r30_delay_golden_fixture, r30_gain_golden_fixture, run_offline,
};

fn assert_processor<T: Processor>() {}

#[test]
fn all_public_effects_implement_processor() {
    assert_processor::<SmoothedGain>();
    assert_processor::<Gain>();
    assert_processor::<Pan>();
    assert_processor::<DcBlocker>();
    assert_processor::<OnePoleFilter>();
    assert_processor::<BiquadFilter>();
    assert_processor::<StateVariableFilter>();
    assert_processor::<DelayProcessor>();
    assert_processor::<FractionalDelay>();
    assert_processor::<CombFilter>();
    assert_processor::<AllPassFilter>();
    assert_processor::<Chorus>();
    assert_processor::<Flanger>();
    assert_processor::<Vibrato>();
    assert_processor::<Waveshaper>();
    assert_processor::<SoftClipper>();
    assert_processor::<Compressor>();
    assert_processor::<Limiter>();
    assert_processor::<Gate>();
    assert_processor::<OversampledSoftClipper>();
}

#[test]
fn golden_gain_fixture_is_exact() {
    let fixture = r30_gain_golden_fixture();
    let mut gain = Gain::new(0.25);
    assert_eq!(run_offline(&mut gain, &fixture, 1), fixture.expected);
}

#[test]
fn golden_delay_fixture_is_exact() {
    let fixture = r30_delay_golden_fixture();
    let mut delay = DelayProcessor::milliseconds(2.0, 2.0);
    assert_eq!(run_offline(&mut delay, &fixture, 1), fixture.expected);
}

#[test]
fn smoothing_gain_pan_and_dc_blocker_are_deterministic() {
    let mut smoothed = SmoothedGain::new(0.0, 1.0);
    let output = process_mono_with_events(
        &mut smoothed,
        &[1.0, 1.0, 1.0, 1.0],
        &[sim_lib_audio_graph_core::BlockEvent::ParamSet {
            offset: 0,
            param: 0,
            value: 1.0,
        }],
        1_000,
    );
    assert_eq!(round6(&output[0]), vec![1.0, 1.0, 1.0, 1.0]);

    let mut pan = Pan::new(-1.0);
    let panned = process_stereo(&mut pan, &[1.0, 1.0], &[1.0, 1.0], 48_000);
    assert_eq!(round6(&panned[0]), vec![1.0, 1.0]);
    assert_eq!(round6(&panned[1]), vec![0.0, 0.0]);

    let mut blocker = DcBlocker::new(0.5);
    let blocked = process_mono(&mut blocker, &[1.0, 1.0, 1.0, 1.0], 48_000);
    assert_eq!(round6(&blocked[0]), vec![1.0, 0.5, 0.25, 0.125]);
}

#[test]
fn filter_family_outputs_are_finite_and_stable() {
    let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let mut one_pole = OnePoleFilter::low_pass(1_000.0);
    let mut biquad = BiquadFilter::low_pass(1_000.0, 0.707);
    let mut svf = StateVariableFilter::new(StateVariableMode::BandPass, 1_000.0, 0.707);

    let one = process_mono(&mut one_pole, &input, 48_000);
    let bi = process_mono(&mut biquad, &input, 48_000);
    let sv = process_mono(&mut svf, &input, 48_000);

    assert_all_finite(&one);
    assert_all_finite(&bi);
    assert_all_finite(&sv);
    assert!(one[0][0] > one[0][1]);
    assert!(bi[0][0] > 0.0);
    assert!(sv[0].iter().any(|sample| sample.abs() > 0.0));
}

#[test]
fn delay_modulation_dynamics_and_oversampling_are_deterministic() {
    let input = [0.0, 0.25, -0.5, 0.75, -1.0, 0.5, 0.0, -0.25];
    let processors: &mut [&mut dyn Processor] = &mut [
        &mut FractionalDelay::milliseconds(1.5, 4.0),
        &mut CombFilter::milliseconds(2.0, 0.25),
        &mut AllPassFilter::milliseconds(2.0, 0.5),
        &mut Chorus::new(0.5, 1.0),
        &mut Flanger::new(0.5, 0.5, 0.25),
        &mut Vibrato::new(1.0, 0.5),
        &mut Waveshaper::new(Waveshape::Cubic, 1.25),
        &mut SoftClipper::new(2.0),
        &mut Compressor::new(-12.0, 4.0),
        &mut Limiter::new(-6.0),
        &mut Gate::new(-18.0, -60.0),
        &mut OversampledSoftClipper::soft_clipper(2.0, 4),
    ];

    let mut fingerprints = Vec::new();
    for processor in processors {
        let first = process_mono(*processor, &input, 48_000);
        processor.reset();
        let second = process_mono(*processor, &input, 48_000);
        assert_eq!(round6(&first[0]), round6(&second[0]));
        assert_all_finite(&first);
        fingerprints.push(round6(&first[0]));
    }

    assert_eq!(fingerprints.len(), 12);
}

#[test]
fn same_processor_runs_offline_and_in_live_graph() {
    let mut offline_gain = Gain::new(0.5);
    let offline = process_stereo(
        &mut offline_gain,
        &[1.0, 0.5, -0.5, -1.0],
        &[-1.0, -0.5, 0.5, 1.0],
        48_000,
    );
    let mut runner =
        LiveGraphRunner::new(Gain::new(0.5), LiveGraphConfig::stereo(48_000, 4).unwrap()).unwrap();
    let mut live_output = [0.0; 8];
    runner
        .process_interleaved_f32(
            Some(&[1.0, -1.0, 0.5, -0.5, -0.5, 0.5, -1.0, 1.0]),
            &mut live_output,
            4,
            Transport::default(),
        )
        .unwrap();

    assert_eq!(
        live_output.to_vec(),
        vec![
            offline[0][0],
            offline[1][0],
            offline[0][1],
            offline[1][1],
            offline[0][2],
            offline[1][2],
            offline[0][3],
            offline[1][3],
        ]
    );
}

#[test]
fn install_audio_dsp_lib_registers_runtime_exports() {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    sim_test_support::assert_lib_exports(
        &mut cx,
        install_audio_dsp_lib,
        &Symbol::new("audio-dsp"),
        &audio_dsp_symbols(),
    );
}

#[test]
fn citizen_dsp_config_descriptor_round_trips_and_fails_closed() {
    let descriptor = DspConfigDescriptor::gain(0.5).unwrap();
    assert_eq!(descriptor.kind().unwrap(), "gain");
    assert_eq!(descriptor.params().unwrap(), vec![("gain".to_owned(), 0.5)]);

    let err = DspConfigDescriptor::new("gain", vec![("gain".to_owned(), f64::NAN)]).unwrap_err();
    assert!(format!("{err}").contains("must be finite"));
}

fn process_mono<P: Processor + ?Sized>(
    processor: &mut P,
    input: &[f32],
    sample_rate_hz: u32,
) -> Vec<Vec<f32>> {
    process_mono_with_events(processor, input, &[], sample_rate_hz)
}

fn process_mono_with_events<P: Processor + ?Sized>(
    processor: &mut P,
    input: &[f32],
    events: &[sim_lib_audio_graph_core::BlockEvent<'_>],
    sample_rate_hz: u32,
) -> Vec<Vec<f32>> {
    process_block(processor, &[input], 1, events, sample_rate_hz)
}

fn process_stereo<P: Processor + ?Sized>(
    processor: &mut P,
    left: &[f32],
    right: &[f32],
    sample_rate_hz: u32,
) -> Vec<Vec<f32>> {
    process_block(processor, &[left, right], 2, &[], sample_rate_hz)
}

fn process_block<P: Processor + ?Sized>(
    processor: &mut P,
    inputs: &[&[f32]],
    out_channels: usize,
    events: &[sim_lib_audio_graph_core::BlockEvent<'_>],
    sample_rate_hz: u32,
) -> Vec<Vec<f32>> {
    let frames = inputs.first().map_or(0, |lane| lane.len());
    processor.prepare(PrepareConfig::new(
        sample_rate_hz,
        frames as u32,
        inputs.len() as u16,
        out_channels as u16,
    ));
    let mut output = vec![vec![0.0; frames]; out_channels];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * out_channels.max(1));
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: inputs,
        out_audio: &mut output_refs,
        in_events: events,
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    processor.process(&mut block);
    output
}

fn round6(values: &[f32]) -> Vec<f32> {
    values
        .iter()
        .map(|value| (value * 1_000_000.0).round() / 1_000_000.0)
        .collect()
}

fn assert_all_finite(output: &[Vec<f32>]) {
    for lane in output {
        for sample in lane {
            assert!(sample.is_finite());
        }
    }
}

/// Prepares `processor` for `prepared_channels` then processes one block whose
/// output has `block_channels` lanes, without re-preparing in between. Used to
/// exercise the audio-path channel clamp when the block width differs from what
/// `prepare` saw.
fn process_with_prepared_width<P: Processor + ?Sized>(
    processor: &mut P,
    prepared_channels: usize,
    block_channels: usize,
    frames: usize,
) -> Vec<Vec<f32>> {
    processor.prepare(PrepareConfig::new(
        48_000,
        frames as u32,
        block_channels as u16,
        prepared_channels as u16,
    ));
    let input: Vec<f32> = (0..frames)
        .map(|frame| frame as f32 / frames as f32)
        .collect();
    let inputs: Vec<&[f32]> = (0..block_channels).map(|_| input.as_slice()).collect();
    let mut output = vec![vec![0.0; frames]; block_channels];
    let mut output_refs: Vec<&mut [f32]> = output.iter_mut().map(Vec::as_mut_slice).collect();
    let mut sink = NullEventSink;
    let mut scratch = BlockArena::with_f32_capacity(frames * block_channels.max(1));
    let mut block = ProcessBlock {
        frames: frames as u32,
        in_audio: &inputs,
        out_audio: &mut output_refs,
        in_events: &[],
        out_events: &mut sink,
        transport: Transport::default(),
        scratch: &mut scratch,
    };
    processor.process(&mut block);
    output
}

#[test]
fn narrower_block_than_prepare_clamps_and_stays_finite() {
    // Prepared for four channels, handed a two-channel block: the audio path
    // clamps to the block width and never touches the extra prepared state.
    let mut compressor = Compressor::new(-12.0, 4.0);
    let output = process_with_prepared_width(&mut compressor, 4, 2, 32);
    assert_eq!(output.len(), 2);
    assert_all_finite(&output);

    let mut delay = DelayProcessor::milliseconds(2.0, 8.0);
    let output = process_with_prepared_width(&mut delay, 4, 2, 32);
    assert_eq!(output.len(), 2);
    assert_all_finite(&output);
}

// In debug builds the audio-path guard trips instead of silently reallocating
// per-channel state when a block is wider than `prepare` configured. In release
// builds the same width is clamped without allocating, so this contract check
// is debug-only.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "more channels than prepare configured")]
fn wider_block_than_prepare_trips_guard_in_debug() {
    let mut compressor = Compressor::new(-12.0, 4.0);
    let _ = process_with_prepared_width(&mut compressor, 1, 2, 16);
}
