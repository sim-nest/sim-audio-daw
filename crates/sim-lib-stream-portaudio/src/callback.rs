use sim_kernel::{Error, Result};
use sim_lib_audio_graph_core::{
    BlockArena, NullEventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_stream_audio::{
    PcmSampleFormat, PcmSpec, f32_interleaved_to_planar, f32_planar_to_interleaved,
    i16_samples_to_f32,
};

/// Interleaved host callback input buffer accepted by the PortAudio bridge.
#[derive(Clone, Debug, PartialEq)]
pub enum PortAudioHostBuffer {
    /// Interleaved 32-bit float samples.
    F32(Vec<f32>),
    /// Interleaved signed 16-bit integer samples.
    I16(Vec<i16>),
}

/// Converts host callback buffers and drives an audio-graph processor.
#[derive(Debug)]
pub struct PortAudioCallbackBridge<P> {
    processor: P,
    output_spec: PcmSpec,
    input_channels: usize,
    max_block_frames: u32,
    scratch: BlockArena,
    sample_pos: u64,
}

impl PortAudioHostBuffer {
    /// Returns the PCM sample format carried by this buffer.
    pub fn sample_format(&self) -> PcmSampleFormat {
        match self {
            Self::F32(_) => PcmSampleFormat::F32,
            Self::I16(_) => PcmSampleFormat::I16,
        }
    }

    /// Deinterleaves into per-channel float lanes for the given channel count.
    ///
    /// Integer samples are converted to float before deinterleaving. Returns an
    /// error when the sample count is not a multiple of `channels`.
    pub fn to_f32_planar(&self, channels: usize) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::F32(samples) => f32_interleaved_to_planar(samples, channels),
            Self::I16(samples) => {
                let converted = i16_samples_to_f32(samples);
                f32_interleaved_to_planar(&converted, channels)
            }
        }
    }
}

impl<P: Processor> PortAudioCallbackBridge<P> {
    /// Builds a bridge that drives `processor` from host callback buffers.
    ///
    /// The processor is prepared once for `output_spec`'s sample rate,
    /// `max_block_frames`, and the input/output channel counts. Returns an
    /// error when `max_block_frames` is zero or a channel count exceeds
    /// [`u16`].
    pub fn new(
        mut processor: P,
        output_spec: PcmSpec,
        input_channels: usize,
        max_block_frames: u32,
    ) -> Result<Self> {
        if max_block_frames == 0 {
            return Err(Error::Eval(
                "PortAudio callback max block frames must be greater than zero".to_owned(),
            ));
        }
        let in_channels = checked_channels(input_channels, "input")?;
        let out_channels = checked_channels(output_spec.channels(), "output")?;
        processor.prepare(PrepareConfig::new(
            output_spec.sample_rate_hz(),
            max_block_frames,
            in_channels,
            out_channels,
        ));
        Ok(Self {
            processor,
            output_spec,
            input_channels,
            max_block_frames,
            scratch: BlockArena::with_f32_capacity(
                max_block_frames as usize * output_spec.channels().max(input_channels).max(1),
            ),
            sample_pos: 0,
        })
    }

    /// Processes one host callback block and returns interleaved output.
    ///
    /// `input` supplies the host capture buffer, or silence when `None`. The
    /// block advances the transport sample position by `frames`. Returns an
    /// error when `frames` exceeds the configured maximum block size.
    pub fn process_interleaved(
        &mut self,
        input: Option<&PortAudioHostBuffer>,
        frames: usize,
    ) -> Result<Vec<f32>> {
        if frames > self.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "PortAudio callback received {frames} frames, max is {}",
                self.max_block_frames
            )));
        }
        let input_planar = match input {
            Some(buffer) => buffer.to_f32_planar(self.input_channels)?,
            None => vec![vec![0.0; frames]; self.input_channels],
        };
        let mut output_planar = vec![vec![0.0; frames]; self.output_spec.channels()];
        self.process_planar(&input_planar, &mut output_planar, frames)?;
        f32_planar_to_interleaved(&output_planar)
    }

    /// Returns the running transport sample position.
    pub fn sample_pos(&self) -> u64 {
        self.sample_pos
    }

    /// Resets the processor, the sample position, and the scratch arena.
    pub fn reset(&mut self) {
        self.processor.reset();
        self.sample_pos = 0;
        self.scratch.reset();
    }

    fn process_planar(
        &mut self,
        input_planar: &[Vec<f32>],
        output_planar: &mut [Vec<f32>],
        frames: usize,
    ) -> Result<()> {
        let input_refs = input_planar.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut output_refs = output_planar
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        self.scratch.reset();
        let mut event_sink = NullEventSink;
        let mut block = ProcessBlock {
            frames: frames as u32,
            in_audio: &input_refs,
            out_audio: &mut output_refs,
            in_events: &[],
            out_events: &mut event_sink,
            transport: Transport {
                playing: true,
                sample_pos: self.sample_pos,
                tempo_bpm: 120.0,
                ppq_pos: 0.0,
            },
            scratch: &mut self.scratch,
        };
        block.validate_audio_lanes()?;
        self.processor.process(&mut block);
        block.validate_audio_lanes()?;
        self.sample_pos = self.sample_pos.saturating_add(frames as u64);
        Ok(())
    }
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("PortAudio {role} channel count exceeds u16")))
}
