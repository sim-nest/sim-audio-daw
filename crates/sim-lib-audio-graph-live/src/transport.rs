use std::collections::BTreeMap;

use sim_kernel::{Result, Symbol};
use sim_lib_audio_graph_core::Transport;
use sim_lib_stream_clock::Clock;
use sim_lib_stream_core::{StreamDiagnostic, StreamEnvelope, StreamMedia, StreamPacket};

/// Returns the symbol identifying the live audio graph clock.
pub fn live_clock_symbol() -> Symbol {
    Symbol::qualified("clock", "audio-graph-live")
}

/// Stream-clock-backed transport source for live audio blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveTransportClock {
    clock: Clock,
    sample_rate_hz: u32,
}

/// Reordering window that buffers LAN preview envelopes back into sequence
/// order, emitting jitter/reorder/drop/late-packet diagnostics along the way.
#[derive(Clone, Debug)]
pub struct LanBufferedPreviewWindow {
    next_sequence: u64,
    reorder_depth: u64,
    pending: BTreeMap<u64, StreamEnvelope>,
    diagnostics: Vec<StreamPacket>,
}

impl LiveTransportClock {
    /// Creates a sample-frame clock for the given sample rate.
    pub fn sample_frame(sample_rate_hz: u32) -> Result<Self> {
        Ok(Self {
            clock: Clock::frame(live_clock_symbol(), u64::from(sample_rate_hz))?,
            sample_rate_hz,
        })
    }

    /// Returns the underlying stream clock.
    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    /// Returns the clock's sample rate in hertz.
    pub fn sample_rate_hz(&self) -> u32 {
        self.sample_rate_hz
    }

    /// Builds a transport snapshot at a sample position and play state.
    pub fn transport_at(&self, sample_pos: u64, playing: bool) -> Transport {
        Transport {
            playing,
            sample_pos,
            tempo_bpm: 120.0,
            ppq_pos: 0.0,
        }
    }
}

impl LanBufferedPreviewWindow {
    /// Creates a window expecting `next_sequence`, tolerating gaps up to
    /// `reorder_depth` before dropping the missing range.
    pub fn new(next_sequence: u64, reorder_depth: u64) -> Self {
        Self {
            next_sequence,
            reorder_depth,
            pending: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Accepts an envelope and returns any envelopes now in sequence order.
    ///
    /// Late, jittered, reordered, or dropped packets are recorded as
    /// diagnostics retrievable via [`drain_diagnostics`](Self::drain_diagnostics).
    pub fn push(&mut self, envelope: StreamEnvelope) -> Result<Vec<StreamEnvelope>> {
        validate_lan_buffered_preview_envelope(&envelope)?;
        let sequence = envelope.sequence();
        if sequence < self.next_sequence || self.pending.contains_key(&sequence) {
            self.record(
                lan_buffered_preview_late_packet_diagnostic_kind(),
                format!(
                    "LAN preview packet {sequence} arrived after sequence {} was expected",
                    self.next_sequence
                ),
            );
            return Ok(Vec::new());
        }
        if sequence > self.next_sequence {
            let gap = sequence - self.next_sequence;
            self.record(
                lan_buffered_preview_jitter_diagnostic_kind(),
                format!("LAN preview packet {sequence} arrived with a sequence gap of {gap}"),
            );
            self.record(
                lan_buffered_preview_reorder_diagnostic_kind(),
                format!(
                    "LAN preview packet {sequence} arrived before expected sequence {}",
                    self.next_sequence
                ),
            );
            if gap > self.reorder_depth {
                self.record(
                    lan_buffered_preview_drop_diagnostic_kind(),
                    format!(
                        "LAN preview dropped missing sequence range {}..{sequence}",
                        self.next_sequence
                    ),
                );
                self.next_sequence = sequence;
                return Ok(self.accept_ready(envelope));
            }
            self.pending.insert(sequence, envelope);
            return Ok(Vec::new());
        }
        Ok(self.accept_ready(envelope))
    }

    /// Drains and returns the accumulated preview diagnostics.
    pub fn drain_diagnostics(&mut self) -> Vec<StreamPacket> {
        std::mem::take(&mut self.diagnostics)
    }

    fn accept_ready(&mut self, envelope: StreamEnvelope) -> Vec<StreamEnvelope> {
        let mut ready = vec![envelope];
        self.next_sequence = self.next_sequence.saturating_add(1);
        while let Some(envelope) = self.pending.remove(&self.next_sequence) {
            ready.push(envelope);
            self.next_sequence = self.next_sequence.saturating_add(1);
        }
        ready
    }

    fn record(&mut self, kind: Symbol, message: String) {
        self.diagnostics
            .push(StreamPacket::Diagnostic(StreamDiagnostic::new(
                kind, message,
            )));
    }
}

/// Validates that an envelope carries PCM media on the LAN buffered audio
/// preview transport profile.
pub fn validate_lan_buffered_preview_envelope(envelope: &StreamEnvelope) -> Result<()> {
    if envelope.media() != StreamMedia::Pcm {
        return Err(sim_kernel::Error::Eval(
            "LAN buffered audio preview requires PCM media".to_owned(),
        ));
    }
    if envelope.profile().name()
        != sim_lib_stream_core::TransportProfile::lan_buffered_audio_preview().name()
    {
        return Err(sim_kernel::Error::Eval(
            "LAN buffered audio preview requires stream/profile/lan-buffered-audio-preview"
                .to_owned(),
        ));
    }
    Ok(())
}

/// Returns the diagnostic kind symbol for preview sequence jitter.
pub fn lan_buffered_preview_jitter_diagnostic_kind() -> Symbol {
    Symbol::qualified("stream/preview", "Jitter")
}

/// Returns the diagnostic kind symbol for dropped preview packet ranges.
pub fn lan_buffered_preview_drop_diagnostic_kind() -> Symbol {
    Symbol::qualified("stream/preview", "Drop")
}

/// Returns the diagnostic kind symbol for out-of-order preview packets.
pub fn lan_buffered_preview_reorder_diagnostic_kind() -> Symbol {
    Symbol::qualified("stream/preview", "Reorder")
}

/// Returns the diagnostic kind symbol for late preview packets.
pub fn lan_buffered_preview_late_packet_diagnostic_kind() -> Symbol {
    Symbol::qualified("stream/preview", "LatePacket")
}
