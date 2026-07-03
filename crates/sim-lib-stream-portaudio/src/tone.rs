use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_audio::{PcmBuffer, PcmSampleFormat, PcmSpec, f32_samples_to_i16};
use sim_lib_stream_host::HostStreamConfigRequest;

/// Host-backed test-tone plan for the selected default output.
#[derive(Clone, Debug, PartialEq)]
pub struct PortAudioTestTonePlan {
    request: HostStreamConfigRequest,
    preview: PcmBuffer,
    frequency_hz: f32,
}

impl PortAudioTestTonePlan {
    /// Builds a plan from a stream request and a rendered preview tone.
    ///
    /// The preview is `frames` samples of a sine at `frequency_hz` rendered in
    /// `spec`'s format at a fixed 0.125 amplitude. Returns an error when
    /// [`test_tone_buffer`] rejects the parameters.
    pub fn new(
        request: HostStreamConfigRequest,
        spec: PcmSpec,
        frames: usize,
        frequency_hz: f32,
    ) -> Result<Self> {
        Ok(Self {
            request,
            preview: test_tone_buffer(spec, frames, frequency_hz, 0.125)?,
            frequency_hz,
        })
    }

    /// Returns the host stream request the plan targets.
    pub fn request(&self) -> &HostStreamConfigRequest {
        &self.request
    }

    /// Returns the rendered preview tone buffer.
    pub fn preview(&self) -> &PcmBuffer {
        &self.preview
    }

    /// Returns the tone frequency in hertz.
    pub fn frequency_hz(&self) -> f32 {
        self.frequency_hz
    }

    /// Returns the target device symbol from the stream request.
    pub fn device(&self) -> &Symbol {
        self.request.device()
    }
}

/// Builds a short deterministic sine tone in the requested PCM format.
pub fn test_tone_buffer(
    spec: PcmSpec,
    frames: usize,
    frequency_hz: f32,
    amplitude: f32,
) -> Result<PcmBuffer> {
    if !frequency_hz.is_finite() || frequency_hz <= 0.0 {
        return Err(Error::Eval(
            "test-tone frequency must be positive and finite".to_owned(),
        ));
    }
    if !amplitude.is_finite() || !(0.0..=1.0).contains(&amplitude) {
        return Err(Error::Eval(
            "test-tone amplitude must be finite and within 0..=1".to_owned(),
        ));
    }
    let mut samples = Vec::with_capacity(frames.saturating_mul(spec.channels()));
    for frame in 0..frames {
        let phase =
            std::f32::consts::TAU * frequency_hz * frame as f32 / spec.sample_rate_hz() as f32;
        let value = phase.sin() * amplitude;
        samples.extend(std::iter::repeat_n(value, spec.channels()));
    }
    match spec.sample_format() {
        PcmSampleFormat::F32 => PcmBuffer::f32(spec, frames, samples),
        PcmSampleFormat::I16 => PcmBuffer::i16(spec, frames, f32_samples_to_i16(&samples)?),
    }
}
