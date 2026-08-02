use std::sync::Arc;

#[cfg(debug_assertions)]
use std::panic::{AssertUnwindSafe, catch_unwind};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_audio_graph_live::{LiveGraphConfig, LiveGraphRunner};

use crate::{
    AllPassFilter, BandlimitedOscillator, BandlimitedWaveform, BiquadFilter, Chorus, CombFilter,
    Compressor, DcBlocker, DelayProcessor, DspConfigDescriptor, Flanger, FractionalDelay, Gain,
    Gate, Limiter, ModulatedDelayProcessor, OnePoleFilter, OscillatorPolicy,
    OversampledSoftClipper, Pan, PolyphaseResampler, ResampleError, ResamplerPolicy, SmoothedGain,
    SoftClipper, StateVariableFilter, StateVariableMode, Vibrato, Waveshape, Waveshaper,
    audio_dsp_symbols, install_audio_dsp_lib, r30_delay_golden_fixture, r30_gain_golden_fixture,
    run_offline,
};

// conformance: audio DSP reuse covers filters, waveshaping, dynamics, and effects.

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
    assert_processor::<BandlimitedOscillator>();
}

#[test]
fn polyblep_oscillator_suppresses_a_folded_saw_harmonic() {
    const SAMPLE_RATE: u32 = 48_000;
    const LEN: usize = 4_096;
    let fundamental_bin = 768usize;
    let frequency = fundamental_bin as f32 * SAMPLE_RATE as f32 / LEN as f32;
    let mut oscillator =
        BandlimitedOscillator::new(OscillatorPolicy::new(frequency, BandlimitedWaveform::Saw));
    let output = process_mono(&mut oscillator, &vec![0.0; LEN], SAMPLE_RATE);
    let corrected = &output[0];
    let naive = (0..LEN)
        .map(|index| {
            let phase = (index as f32 * frequency / SAMPLE_RATE as f32).fract();
            2.0 * phase - 1.0
        })
        .collect::<Vec<_>>();
    let folded_third_bin = LEN - fundamental_bin * 3;
    let corrected_alias = dft_bin_magnitude(corrected, folded_third_bin);
    let naive_alias = dft_bin_magnitude(&naive, folded_third_bin);

    assert!(corrected.iter().all(|sample| sample.is_finite()));
    assert!(dft_bin_magnitude(corrected, fundamental_bin) > 0.3);
    assert!(
        corrected_alias < naive_alias * 0.35,
        "polyBLEP alias {corrected_alias} was not below naive alias {naive_alias}"
    );
}

#[test]
fn oscillator_and_resampler_callbacks_retain_preallocated_state() {
    let mut oscillator =
        BandlimitedOscillator::new(OscillatorPolicy::new(440.0, BandlimitedWaveform::Triangle));
    oscillator.prepare(PrepareConfig::new(48_000, 128, 0, 2));
    let oscillator_before = oscillator.realtime_state_snapshot();
    let _ = process_without_reprepare(&mut oscillator, 2, 128);
    assert_eq!(oscillator.realtime_state_snapshot(), oscillator_before);

    let mut resampler = PolyphaseResampler::new(48_000, 44_100, 2, ResamplerPolicy::default())
        .expect("valid resampler");
    let before = resampler.realtime_state_snapshot();
    let input = vec![0.0; 256];
    let required = resampler.required_output_frames(128).unwrap();
    let mut output = vec![0.0; required * 2];
    let report = resampler.process_interleaved(&input, &mut output).unwrap();
    assert_eq!(report.output_frames, required);
    assert_eq!(resampler.realtime_state_snapshot(), before);
}

#[test]
fn polyphase_resampler_has_unity_impulse_response_and_rejects_short_output() {
    let policy = ResamplerPolicy {
        taps: 48,
        ..ResamplerPolicy::default()
    };
    let mut resampler = PolyphaseResampler::new(48_000, 48_000, 1, policy).unwrap();
    let mut impulse = vec![0.0; 256];
    impulse[96] = 1.0;
    let required = resampler.required_output_frames(impulse.len()).unwrap();
    let before = resampler.realtime_state_snapshot();
    let error = resampler
        .process_interleaved(&impulse, &mut vec![0.0; required - 1])
        .unwrap_err();
    assert_eq!(
        error,
        ResampleError::OutputTooSmall {
            required,
            available: required - 1,
        }
    );
    assert_eq!(resampler.realtime_state_snapshot(), before);

    let mut output = vec![0.0; required];
    resampler
        .process_interleaved(&impulse, &mut output)
        .unwrap();
    let response_sum = output.iter().map(|sample| f64::from(*sample)).sum::<f64>();
    let peak = output
        .iter()
        .enumerate()
        .max_by(|left, right| left.1.abs().total_cmp(&right.1.abs()))
        .map(|(index, _)| index)
        .unwrap();
    assert!(
        (response_sum - 1.0).abs() < 1e-5,
        "impulse sum {response_sum}"
    );
    assert_eq!(peak, 96);
}

#[test]
fn downsampling_filter_preserves_passband_and_rejects_alias_tone() {
    let low = resample_tone(2_000.0);
    let above_output_nyquist = resample_tone(12_000.0);
    let low_rms = rms(&low[64..]);
    let alias_rms = rms(&above_output_nyquist[64..]);

    assert!(low_rms > 0.65, "passband RMS {low_rms}");
    assert!(
        alias_rms < 0.03,
        "12 kHz input aliased into 16 kHz output at RMS {alias_rms}"
    );
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
fn zero_delay_fully_wet_delay_outputs_current_input() {
    let input = [0.25, -0.5, 0.75, -1.0];
    let mut delay = DelayProcessor::new(0.0, 0.001);

    assert_eq!(delay.tail_frames(), 0);
    assert_eq!(
        process_mono(&mut delay, &input, 48_000),
        vec![input.to_vec()]
    );
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
    process_without_reprepare(processor, block_channels, frames)
}

fn process_without_reprepare<P: Processor + ?Sized>(
    processor: &mut P,
    block_channels: usize,
    frames: usize,
) -> Vec<Vec<f32>> {
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

#[test]
fn stateless_processors_accept_wider_blocks_without_prepared_state() {
    assert_stateless_wider_block_processes("SmoothedGain", SmoothedGain::new(0.5, 1.0));
    assert_stateless_wider_block_processes("Gain", Gain::new(0.5));
    assert_stateless_wider_block_processes("Pan", Pan::new(0.25));
    assert_stateless_wider_block_processes("Waveshaper", Waveshaper::new(Waveshape::Tanh, 1.2));
    assert_stateless_wider_block_processes("SoftClipper", SoftClipper::new(2.0));
}

#[cfg(debug_assertions)]
#[test]
fn stateful_processors_reject_wider_blocks_in_debug() {
    assert_stateful_wider_block_guard("DcBlocker", DcBlocker::default());
    assert_stateful_wider_block_guard("OnePoleFilter", OnePoleFilter::low_pass(1_000.0));
    assert_stateful_wider_block_guard("BiquadFilter", BiquadFilter::low_pass(1_000.0, 0.707));
    assert_stateful_wider_block_guard(
        "StateVariableFilter",
        StateVariableFilter::new(StateVariableMode::LowPass, 1_000.0, 0.707),
    );
    assert_stateful_wider_block_guard("DelayProcessor", DelayProcessor::milliseconds(2.0, 8.0));
    assert_stateful_wider_block_guard("FractionalDelay", FractionalDelay::milliseconds(1.5, 8.0));
    assert_stateful_wider_block_guard("CombFilter", CombFilter::milliseconds(2.0, 0.25));
    assert_stateful_wider_block_guard("AllPassFilter", AllPassFilter::milliseconds(2.0, 0.5));
    assert_stateful_wider_block_guard(
        "ModulatedDelayProcessor",
        ModulatedDelayProcessor::new(2.0, 1.0, 0.5),
    );
    assert_stateful_wider_block_guard("Chorus", Chorus::new(0.5, 1.0));
    assert_stateful_wider_block_guard("Flanger", Flanger::new(0.5, 0.5, 0.25));
    assert_stateful_wider_block_guard("Vibrato", Vibrato::new(1.0, 0.5));
    assert_stateful_wider_block_guard("Compressor", Compressor::new(-12.0, 4.0));
    assert_stateful_wider_block_guard("Limiter", Limiter::new(-6.0));
    assert_stateful_wider_block_guard("Gate", Gate::new(-18.0, -60.0));
    assert_stateful_wider_block_guard(
        "OversampledSoftClipper",
        OversampledSoftClipper::soft_clipper(2.0, 4),
    );
}

#[cfg(not(debug_assertions))]
#[test]
fn stateful_processors_clamp_wider_blocks_without_state_growth_in_release() {
    assert_stateful_no_growth("DcBlocker", DcBlocker::default(), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("OnePoleFilter", OnePoleFilter::low_pass(1_000.0), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth(
        "BiquadFilter",
        BiquadFilter::low_pass(1_000.0, 0.707),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth(
        "StateVariableFilter",
        StateVariableFilter::new(StateVariableMode::LowPass, 1_000.0, 0.707),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth(
        "DelayProcessor",
        DelayProcessor::milliseconds(2.0, 8.0),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth(
        "FractionalDelay",
        FractionalDelay::milliseconds(1.5, 8.0),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth("CombFilter", CombFilter::milliseconds(2.0, 0.25), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth(
        "AllPassFilter",
        AllPassFilter::milliseconds(2.0, 0.5),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth(
        "ModulatedDelayProcessor",
        ModulatedDelayProcessor::new(2.0, 1.0, 0.5),
        |p| p.realtime_state_snapshot(),
    );
    assert_stateful_no_growth("Chorus", Chorus::new(0.5, 1.0), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("Flanger", Flanger::new(0.5, 0.5, 0.25), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("Vibrato", Vibrato::new(1.0, 0.5), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("Compressor", Compressor::new(-12.0, 4.0), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("Limiter", Limiter::new(-6.0), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth("Gate", Gate::new(-18.0, -60.0), |p| {
        p.realtime_state_snapshot()
    });
    assert_stateful_no_growth(
        "OversampledSoftClipper",
        OversampledSoftClipper::soft_clipper(2.0, 4),
        |p| p.realtime_state_snapshot(),
    );
}

fn assert_stateless_wider_block_processes<P: Processor>(name: &str, mut processor: P) {
    let output = process_with_prepared_width(&mut processor, 1, 2, 16);
    assert_eq!(output.len(), 2, "{name} should process both output lanes");
    assert_all_finite(&output);
}

#[cfg(debug_assertions)]
fn assert_stateful_wider_block_guard<P: Processor>(name: &str, mut processor: P) {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = process_with_prepared_width(&mut processor, 1, 2, 16);
    }));
    assert!(
        result.is_err(),
        "{name} accepted a wider block than prepare configured"
    );
}

#[cfg(not(debug_assertions))]
fn assert_stateful_no_growth<P, F>(name: &str, mut processor: P, snapshot: F)
where
    P: Processor,
    F: Fn(&P) -> Vec<usize>,
{
    processor.prepare(PrepareConfig::new(48_000, 16, 1, 1));
    let before = snapshot(&processor);
    let output = process_without_reprepare(&mut processor, 2, 16);
    assert_eq!(
        snapshot(&processor),
        before,
        "{name} grew realtime state while clamping a wider block"
    );
    assert_eq!(output.len(), 2, "{name} should preserve block shape");
    assert_all_finite(&output);
}

fn dft_bin_magnitude(samples: &[f32], bin: usize) -> f64 {
    let len = samples.len() as f64;
    let (real, imaginary) =
        samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(real, imaginary), (index, sample)| {
                let angle = std::f64::consts::TAU * bin as f64 * index as f64 / len;
                (
                    real + f64::from(*sample) * angle.cos(),
                    imaginary - f64::from(*sample) * angle.sin(),
                )
            });
    real.hypot(imaginary) * 2.0 / len
}

fn resample_tone(frequency_hz: f64) -> Vec<f32> {
    let input = (0..4_096)
        .map(|index| (std::f64::consts::TAU * frequency_hz * index as f64 / 48_000.0).sin() as f32)
        .collect::<Vec<_>>();
    let policy = ResamplerPolicy {
        taps: 64,
        ..ResamplerPolicy::default()
    };
    let mut resampler = PolyphaseResampler::new(48_000, 16_000, 1, policy).unwrap();
    let required = resampler.required_output_frames(input.len()).unwrap();
    let mut output = vec![0.0; required];
    resampler.process_interleaved(&input, &mut output).unwrap();
    output
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|sample| f64::from(*sample).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt()
}
