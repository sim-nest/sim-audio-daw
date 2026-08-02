#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Reusable pure Rust DSP processors, bandlimited sources, and fixed-state
//! sample-rate conversion for the SIM audio graph.

mod citizen;
mod common;
pub mod cookbook;
mod cookbook_runtime;
mod delay;
mod dynamics;
mod filter;
mod fixture;
mod gain;
mod modulation;
mod oscillator;
mod oversampling;
mod resample;
mod runtime;
mod smoothing;

pub use citizen::{DspConfigDescriptor, dsp_config_class_symbol};
pub use cookbook::{audio_processing_trace_demo, offline_chain_demo};
pub use delay::{AllPassFilter, CombFilter, DelayLine, DelayProcessor, FractionalDelay};
pub use dynamics::{
    Compressor, DynamicsEnvelope, Gate, Limiter, SoftClipper, Waveshape, Waveshaper,
};
pub use filter::{
    BiquadFilter, BiquadKind, OnePoleFilter, OnePoleMode, StateVariableFilter, StateVariableMode,
};
pub use fixture::{GoldenFixture, r30_delay_golden_fixture, r30_gain_golden_fixture, run_offline};
pub use gain::{DcBlocker, Gain, Pan};
pub use modulation::{Chorus, Flanger, ModulatedDelayProcessor, Vibrato};
pub use oscillator::{
    BandlimitPolicy, BandlimitedOscillator, BandlimitedWaveform, OscillatorPolicy,
};
pub use oversampling::{NonlinearSampleProcessor, OversampledSoftClipper, OversamplingWrapper};
pub use resample::{PolyphaseResampler, ResampleError, ResampleReport, ResamplerPolicy};
pub use runtime::{AudioDspLib, audio_dsp_symbols, install_audio_dsp_lib};
pub use smoothing::{SmoothValue, SmoothedGain};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
