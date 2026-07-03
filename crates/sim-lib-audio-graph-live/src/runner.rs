use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{
    BlockArena, BlockEvent, EventSink, PrepareConfig, ProcessBlock, Processor, Transport,
};
use sim_lib_stream_audio::PcmSpec;
use sim_lib_stream_core::{
    PcmPacket, StreamEnvelope, StreamInspectorSnapshot, StreamInspectorStatus, StreamPacket,
    TransportProfile,
};

use crate::{
    AudioToControlQueue, ControlToAudioQueue, LiveAudioEvent, LiveControlEvent, LiveStreamLane,
    validate_realtime_local_audio_profile,
};

const MAX_LIVE_CHANNELS: usize = 2;
const MAX_LIVE_EVENTS: usize = 64;

/// Live runner configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveGraphConfig {
    spec: PcmSpec,
    input_channels: usize,
    max_block_frames: u32,
    control_queue_capacity: usize,
    audio_queue_capacity: usize,
}

/// Process result for one live callback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveProcessReport {
    frames: u32,
    control_events: usize,
    dropped_control_events: u64,
}

/// Capacity snapshot used to verify steady-state processing does not grow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveSteadyStateSnapshot {
    input_lane_capacity: Vec<usize>,
    output_lane_capacity: Vec<usize>,
    scratch_capacity: usize,
    control_queue_capacity: usize,
    audio_queue_capacity: usize,
}

/// Preallocated live graph runner for host audio callbacks.
#[derive(Debug)]
pub struct LiveGraphRunner<P> {
    processor: P,
    config: LiveGraphConfig,
    input_planar: Vec<Vec<f32>>,
    output_planar: Vec<Vec<f32>>,
    scratch: BlockArena,
    event_slots: [BlockEvent<'static>; MAX_LIVE_EVENTS],
    control_to_audio: ControlToAudioQueue,
    audio_to_control: AudioToControlQueue,
}

impl LiveGraphConfig {
    /// Builds a runner configuration, validating block size, channel counts
    /// (up to stereo), and bounded queue capacities.
    pub fn new(
        spec: PcmSpec,
        input_channels: usize,
        max_block_frames: u32,
        control_queue_capacity: usize,
        audio_queue_capacity: usize,
    ) -> Result<Self> {
        if max_block_frames == 0 {
            return Err(Error::Eval(
                "live graph max block frames must be greater than zero".to_owned(),
            ));
        }
        if input_channels > MAX_LIVE_CHANNELS || spec.channels() > MAX_LIVE_CHANNELS {
            return Err(Error::Eval(format!(
                "live graph runner supports up to {MAX_LIVE_CHANNELS} channels"
            )));
        }
        if control_queue_capacity == 0 || audio_queue_capacity == 0 {
            return Err(Error::Eval(
                "live graph queues must be bounded and non-zero".to_owned(),
            ));
        }
        if control_queue_capacity > MAX_LIVE_EVENTS {
            return Err(Error::Eval(format!(
                "live graph control queue supports up to {MAX_LIVE_EVENTS} events per block"
            )));
        }
        if audio_queue_capacity > MAX_LIVE_EVENTS {
            return Err(Error::Eval(format!(
                "live graph audio diagnostic queue supports up to {MAX_LIVE_EVENTS} events per block"
            )));
        }
        Ok(Self {
            spec,
            input_channels,
            max_block_frames,
            control_queue_capacity,
            audio_queue_capacity,
        })
    }

    /// Builds a stereo-in/stereo-out configuration with full event queues.
    pub fn stereo(sample_rate_hz: u32, max_block_frames: u32) -> Result<Self> {
        Self::new(
            PcmSpec::f32(2, sample_rate_hz)?,
            2,
            max_block_frames,
            MAX_LIVE_EVENTS,
            MAX_LIVE_EVENTS,
        )
    }

    /// Returns the output PCM spec.
    pub fn spec(self) -> PcmSpec {
        self.spec
    }

    /// Returns the input channel count.
    pub fn input_channels(self) -> usize {
        self.input_channels
    }

    /// Returns the output channel count.
    pub fn output_channels(self) -> usize {
        self.spec.channels()
    }

    /// Returns the maximum block size in frames.
    pub fn max_block_frames(self) -> u32 {
        self.max_block_frames
    }
}

impl<P: Processor> LiveGraphRunner<P> {
    /// Creates a runner after validating the realtime local audio profile.
    pub fn new_realtime(
        processor: P,
        config: LiveGraphConfig,
        profile: &TransportProfile,
    ) -> Result<Self> {
        validate_realtime_local_audio_profile(profile)?;
        Self::new(processor, config)
    }

    /// Creates a runner, preparing the processor and preallocating all buffers
    /// and queues for allocation-free steady-state processing.
    pub fn new(mut processor: P, config: LiveGraphConfig) -> Result<Self> {
        processor.prepare(PrepareConfig::new(
            config.spec.sample_rate_hz(),
            config.max_block_frames,
            checked_channels(config.input_channels, "input")?,
            checked_channels(config.spec.channels(), "output")?,
        ));
        let max_frames = config.max_block_frames as usize;
        Ok(Self {
            processor,
            config,
            input_planar: vec![vec![0.0; max_frames]; config.input_channels],
            output_planar: vec![vec![0.0; max_frames]; config.spec.channels()],
            scratch: BlockArena::with_f32_capacity(
                max_frames * config.input_channels.max(config.spec.channels()).max(1),
            ),
            event_slots: [empty_event(); MAX_LIVE_EVENTS],
            control_to_audio: ControlToAudioQueue::with_capacity(config.control_queue_capacity)?,
            audio_to_control: AudioToControlQueue::with_capacity(config.audio_queue_capacity)?,
        })
    }

    /// Enqueues a control event for delivery on the next process call.
    pub fn enqueue_control_event(&mut self, event: LiveControlEvent) -> crate::LiveQueuePush {
        self.control_to_audio.push(event)
    }

    /// Enqueues a short MIDI control event built from `bytes`.
    pub fn enqueue_midi_short(
        &mut self,
        offset: u32,
        bytes: &[u8],
    ) -> Result<crate::LiveQueuePush> {
        Ok(self.enqueue_control_event(LiveControlEvent::midi_short(offset, bytes)?))
    }

    /// Enqueues a parameter-set control event.
    pub fn enqueue_param_set(
        &mut self,
        offset: u32,
        param: u32,
        value: f64,
    ) -> Result<crate::LiveQueuePush> {
        Ok(self.enqueue_control_event(LiveControlEvent::param_set(offset, param, value)?))
    }

    /// Processes one interleaved audio block: drains queued control events,
    /// runs the processor, and writes the interleaved output.
    pub fn process_interleaved_f32(
        &mut self,
        input: Option<&[f32]>,
        output: &mut [f32],
        frames: usize,
        transport: Transport,
    ) -> Result<LiveProcessReport> {
        self.validate_block(input, output, frames)?;
        let dropped_control_events = self.control_to_audio.take_dropped();
        if dropped_control_events > 0 {
            self.record_audio_event(LiveAudioEvent::DroppedControlEvents {
                count: dropped_control_events,
            });
        }
        let event_count = self.drain_control_events(frames)?;
        self.copy_input(input, frames);
        self.clear_output(frames);
        self.run_processor(frames, event_count, transport)?;
        self.copy_output(output, frames);
        Ok(LiveProcessReport {
            frames: frames as u32,
            control_events: event_count,
            dropped_control_events,
        })
    }

    /// Drains audio-thread events back to the control thread, appending a
    /// dropped-events marker if the queue overflowed.
    pub fn drain_audio_events(&mut self) -> Vec<LiveAudioEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.audio_to_control.pop() {
            events.push(event);
        }
        let dropped = self.audio_to_control.take_dropped();
        if dropped > 0 {
            events.push(LiveAudioEvent::DroppedAudioEvents { count: dropped });
        }
        events
    }

    /// Drains audio-thread events and renders each as a diagnostic packet.
    pub fn drain_audio_diagnostics(&mut self) -> Vec<sim_lib_stream_core::StreamPacket> {
        self.drain_audio_events()
            .into_iter()
            .map(LiveAudioEvent::to_diagnostic_packet)
            .collect()
    }

    /// Returns a stream inspector snapshot for the audio-to-control queue.
    pub fn diagnostic_inspector(&self) -> Result<StreamInspectorSnapshot> {
        let metadata = LiveStreamLane::Diagnostic.metadata(self.audio_to_control.capacity())?;
        let stats = self.audio_to_control.stats();
        Ok(StreamInspectorSnapshot::new(
            &metadata,
            Symbol::qualified("stream/route", "live-audio-callback"),
            TransportProfile::realtime_local_audio().name().clone(),
            StreamInspectorStatus::from_stats(&stats, false),
            self.audio_to_control.len(),
            &stats,
            stats.pushed.checked_sub(1),
            Vec::new(),
        ))
    }

    /// Captures buffer and queue capacities for steady-state growth checks.
    pub fn steady_state_snapshot(&self) -> LiveSteadyStateSnapshot {
        LiveSteadyStateSnapshot {
            input_lane_capacity: self.input_planar.iter().map(Vec::capacity).collect(),
            output_lane_capacity: self.output_planar.iter().map(Vec::capacity).collect(),
            scratch_capacity: self.scratch.f32_capacity(),
            control_queue_capacity: self.control_to_audio.allocated_capacity(),
            audio_queue_capacity: self.audio_to_control.allocated_capacity(),
        }
    }

    /// Wraps an interleaved output block as a LAN buffered preview envelope.
    pub fn buffered_preview_chunk(
        &self,
        output: &[f32],
        frames: usize,
        sequence: u64,
    ) -> Result<StreamEnvelope> {
        let samples = self.validate_preview_block(output, frames)?;
        let packet = StreamPacket::Pcm(PcmPacket::f32(
            self.config.spec.channels(),
            frames,
            output[..samples].to_vec(),
        )?);
        LiveStreamLane::AudioOutput.lan_buffered_preview_envelope(sequence, Vec::new(), packet)
    }

    fn validate_block(
        &mut self,
        input: Option<&[f32]>,
        output: &[f32],
        frames: usize,
    ) -> Result<()> {
        if frames > self.config.max_block_frames as usize {
            self.record_audio_event(LiveAudioEvent::Xrun {
                frames: frames as u32,
                max_frames: self.config.max_block_frames,
            });
            return Err(Error::Eval(format!(
                "live graph block has {frames} frames, max block is {}",
                self.config.max_block_frames
            )));
        }
        let input_samples = frames.saturating_mul(self.config.input_channels);
        if let Some(samples) = input
            && samples.len() < input_samples
        {
            return Err(Error::Eval(format!(
                "live graph input has {} samples, expected at least {input_samples}",
                samples.len()
            )));
        }
        let output_samples = frames.saturating_mul(self.config.spec.channels());
        if output.len() < output_samples {
            return Err(Error::Eval(format!(
                "live graph output has {} samples, expected at least {output_samples}",
                output.len()
            )));
        }
        Ok(())
    }

    fn validate_preview_block(&self, output: &[f32], frames: usize) -> Result<usize> {
        if frames > self.config.max_block_frames as usize {
            return Err(Error::Eval(format!(
                "live graph preview has {frames} frames, max block is {}",
                self.config.max_block_frames
            )));
        }
        let output_samples = frames
            .checked_mul(self.config.spec.channels())
            .ok_or_else(|| Error::Eval("live graph preview sample count overflowed".to_owned()))?;
        if output.len() < output_samples {
            return Err(Error::Eval(format!(
                "live graph preview has {} samples, expected at least {output_samples}",
                output.len()
            )));
        }
        Ok(output_samples)
    }

    fn drain_control_events(&mut self, frames: usize) -> Result<usize> {
        let mut count = 0;
        while let Some(event) = self.control_to_audio.pop() {
            if event.offset() > frames as u32 {
                return Err(Error::Eval(format!(
                    "live control event offset {} exceeds block frames {frames}",
                    event.offset()
                )));
            }
            self.event_slots[count] = event.to_block_event();
            count += 1;
        }
        Ok(count)
    }

    fn copy_input(&mut self, input: Option<&[f32]>, frames: usize) {
        for lane in &mut self.input_planar {
            lane[..frames].fill(0.0);
        }
        if let Some(samples) = input {
            for frame in 0..frames {
                for channel in 0..self.config.input_channels {
                    self.input_planar[channel][frame] =
                        samples[frame * self.config.input_channels + channel];
                }
            }
        }
    }

    fn clear_output(&mut self, frames: usize) {
        for lane in &mut self.output_planar {
            lane[..frames].fill(0.0);
        }
    }

    fn copy_output(&self, output: &mut [f32], frames: usize) {
        let channels = self.config.spec.channels();
        for frame in 0..frames {
            for channel in 0..channels {
                output[frame * channels + channel] = self.output_planar[channel][frame];
            }
        }
    }

    fn run_processor(
        &mut self,
        frames: usize,
        event_count: usize,
        transport: Transport,
    ) -> Result<()> {
        let in_events = &self.event_slots[..event_count];
        let processor = &mut self.processor;
        let scratch = &mut self.scratch;
        let input_planar = &self.input_planar;
        let output_planar = &mut self.output_planar;
        let audio_to_control = &mut self.audio_to_control;

        macro_rules! run_block {
            ($in_audio:expr, $out_audio:expr) => {{
                let mut event_sink = LiveEventSink {
                    queue: audio_to_control,
                };
                scratch.reset();
                let mut block = ProcessBlock {
                    frames: frames as u32,
                    in_audio: $in_audio,
                    out_audio: $out_audio,
                    in_events,
                    out_events: &mut event_sink,
                    transport,
                    scratch,
                };
                block.validate_audio_lanes()?;
                processor.process(&mut block);
                block.validate_audio_lanes()
            }};
        }

        match (self.config.input_channels, self.config.spec.channels()) {
            (0, 1) => {
                let in_audio: [&[f32]; 0] = [];
                let mut out_audio = [&mut output_planar[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            (0, 2) => {
                let in_audio: [&[f32]; 0] = [];
                let (left, right) = output_planar.split_at_mut(1);
                let mut out_audio = [&mut left[0][..frames], &mut right[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            (1, 1) => {
                let in_audio = [&input_planar[0][..frames]];
                let mut out_audio = [&mut output_planar[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            (1, 2) => {
                let in_audio = [&input_planar[0][..frames]];
                let (left, right) = output_planar.split_at_mut(1);
                let mut out_audio = [&mut left[0][..frames], &mut right[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            (2, 1) => {
                let in_audio = [&input_planar[0][..frames], &input_planar[1][..frames]];
                let mut out_audio = [&mut output_planar[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            (2, 2) => {
                let in_audio = [&input_planar[0][..frames], &input_planar[1][..frames]];
                let (left, right) = output_planar.split_at_mut(1);
                let mut out_audio = [&mut left[0][..frames], &mut right[0][..frames]];
                run_block!(&in_audio, &mut out_audio)
            }
            _ => Err(Error::Eval(
                "live graph runner supports mono and stereo I/O".to_owned(),
            )),
        }
    }

    fn record_audio_event(&mut self, event: LiveAudioEvent) {
        let _ = self.audio_to_control.push(event);
    }
}

struct LiveEventSink<'a> {
    queue: &'a mut AudioToControlQueue,
}

impl EventSink for LiveEventSink<'_> {
    fn push(&mut self, event: BlockEvent<'_>) -> Result<()> {
        if let Some(event) = LiveAudioEvent::from_processor_event(event) {
            let _ = self.queue.push(event);
        }
        Ok(())
    }
}

fn checked_channels(channels: usize, role: &str) -> Result<u16> {
    u16::try_from(channels)
        .map_err(|_| Error::Eval(format!("live graph {role} channel count exceeds u16")))
}

const fn empty_event() -> BlockEvent<'static> {
    BlockEvent::ParamSet {
        offset: 0,
        param: 0,
        value: 0.0,
    }
}

impl LiveProcessReport {
    /// Returns the number of frames processed.
    pub fn frames(self) -> u32 {
        self.frames
    }

    /// Returns the number of control events applied this block.
    pub fn control_events(self) -> usize {
        self.control_events
    }

    /// Returns the number of control events dropped before this block.
    pub fn dropped_control_events(self) -> u64 {
        self.dropped_control_events
    }
}
