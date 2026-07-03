#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Reusable pure Rust DSP processors for the SIM audio graph.

mod citizen;
mod common;
mod delay;
mod dynamics;
mod filter;
mod fixture;
mod gain;
mod modulation;
mod oversampling;
mod runtime;
mod smoothing;

pub use citizen::{DspConfigDescriptor, dsp_config_class_symbol};
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
pub use oversampling::{NonlinearSampleProcessor, OversampledSoftClipper, OversamplingWrapper};
pub use runtime::{AudioDspLib, audio_dsp_symbols, install_audio_dsp_lib};
pub use smoothing::{SmoothValue, SmoothedGain};

#[cfg(test)]
mod tests;
