use std::collections::VecDeque;

use sim_kernel::{Error, Result};
use sim_lib_stream_core::StreamStats;

use crate::{LiveAudioEvent, LiveControlEvent, LiveQueuePush};

/// Bounded queue carrying control events into the audio callback.
pub type ControlToAudioQueue = BoundedLiveQueue<LiveControlEvent>;
/// Bounded queue carrying audio-thread events back to the control thread.
pub type AudioToControlQueue = BoundedLiveQueue<LiveAudioEvent>;

/// Bounded queue used at live audio/control boundaries.
#[derive(Clone, Debug)]
pub struct BoundedLiveQueue<T> {
    entries: VecDeque<T>,
    bound: usize,
    pending_dropped_newest: u64,
    stats: StreamStats,
}

impl<T> BoundedLiveQueue<T> {
    /// Creates a queue bounded to `bound` items, rejecting a zero bound.
    pub fn with_capacity(bound: usize) -> Result<Self> {
        if bound == 0 {
            return Err(Error::Eval(
                "live queue capacity must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            entries: VecDeque::with_capacity(bound),
            bound,
            pending_dropped_newest: 0,
            stats: StreamStats::default(),
        })
    }

    /// Pushes an item, dropping the newest under backpressure when full.
    pub fn push(&mut self, item: T) -> LiveQueuePush {
        self.stats.pushed = self.stats.pushed.saturating_add(1);
        if self.entries.len() >= self.bound {
            self.pending_dropped_newest = self.pending_dropped_newest.saturating_add(1);
            self.stats.dropped_newest = self.stats.dropped_newest.saturating_add(1);
            LiveQueuePush::DroppedNewest
        } else {
            self.entries.push_back(item);
            self.stats.accepted = self.stats.accepted.saturating_add(1);
            LiveQueuePush::Accepted
        }
    }

    /// Removes and returns the oldest item, if any.
    pub fn pop(&mut self) -> Option<T> {
        let item = self.entries.pop_front();
        if item.is_some() {
            self.stats.yielded = self.stats.yielded.saturating_add(1);
        }
        item
    }

    /// Returns the number of queued items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the configured capacity bound.
    pub fn capacity(&self) -> usize {
        self.bound
    }

    /// Returns the backing buffer's allocated capacity.
    pub fn allocated_capacity(&self) -> usize {
        self.entries.capacity()
    }

    /// Returns the count of items dropped since the last [`take_dropped`](Self::take_dropped).
    pub fn dropped(&self) -> u64 {
        self.pending_dropped_newest
    }

    /// Returns and clears the pending dropped-item count.
    pub fn take_dropped(&mut self) -> u64 {
        let dropped = self.pending_dropped_newest;
        self.pending_dropped_newest = 0;
        dropped
    }

    /// Returns a snapshot of the queue's stream statistics.
    pub fn stats(&self) -> StreamStats {
        self.stats.clone()
    }
}
