#![forbid(unsafe_code)]
//! Provider-neutral ports for native audio devices and plugins.

use std::collections::{BTreeMap, VecDeque};

/// Stable provider-neutral identity.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NativeId(pub String);

/// Allocation and blocking contract fixed before a realtime callback starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeBudget {
    pub max_frames: u32,
    pub max_channels: u16,
    pub queue_capacity: u16,
    pub callback_may_allocate: bool,
    pub callback_may_block: bool,
}

impl RealtimeBudget {
    pub fn strict(max_frames: u32, max_channels: u16, queue_capacity: u16) -> Self {
        Self {
            max_frames,
            max_channels,
            queue_capacity,
            callback_may_allocate: false,
            callback_may_block: false,
        }
    }

    pub fn validate(self) -> Result<Self, NativeRefusal> {
        if self.max_frames == 0
            || self.max_channels == 0
            || self.queue_capacity == 0
            || self.callback_may_allocate
            || self.callback_may_block
        {
            return Err(NativeRefusal::BudgetExceeded);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeRefusal {
    Unsupported,
    Unavailable,
    InvalidRequest,
    BudgetExceeded,
    DeviceLost,
    CallbackOverflow,
    ClockDiscontinuity,
    AbiMismatch { expected: u32, found: u32 },
    Cancelled,
    AlreadyClosed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceCard {
    pub id: NativeId,
    pub label: String,
    pub input_channels: u16,
    pub output_channels: u16,
    pub sample_rates: Vec<u32>,
    pub hotplug: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginCard {
    pub id: NativeId,
    pub label: String,
    pub abi: u32,
    pub audio_inputs: u16,
    pub audio_outputs: u16,
    pub isolated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceEvent {
    Added(AudioDeviceCard),
    Removed(NativeId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallbackEvent {
    Ready { frames: u32, clock: u64 },
    Overflow,
    DeviceLost,
    ClockDiscontinuity { previous: u64, current: u64 },
}

/// Unique callback authority. It is deliberately neither `Clone` nor `Copy`.
#[derive(Debug, PartialEq, Eq)]
pub struct CallbackOwner {
    id: NativeId,
}

impl CallbackOwner {
    pub fn new(id: NativeId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &NativeId {
        &self.id
    }
}

pub trait AudioDeviceStream: Send {
    fn callback_owner(&self) -> &CallbackOwner;
    fn poll_callback(&mut self) -> Result<Option<CallbackEvent>, NativeRefusal>;
    fn cancel(&mut self) -> Result<(), NativeRefusal>;
    fn close(&mut self) -> Result<(), NativeRefusal>;
}

pub trait AudioDevicePort: Send + Sync {
    fn cards(&self) -> Result<Vec<AudioDeviceCard>, NativeRefusal>;
    fn poll_hotplug(&self) -> Result<Vec<DeviceEvent>, NativeRefusal>;
    fn open(
        &self,
        id: &NativeId,
        sample_rate: u32,
        budget: RealtimeBudget,
    ) -> Result<Box<dyn AudioDeviceStream>, NativeRefusal>;
}

pub trait NativePluginInstance: Send {
    fn card(&self) -> &PluginCard;
    fn process(&mut self, frames: u32) -> Result<(), NativeRefusal>;
    fn cancel(&mut self) -> Result<(), NativeRefusal>;
    fn unload(&mut self) -> Result<(), NativeRefusal>;
}

pub trait NativePluginPort: Send + Sync {
    fn cards(&self) -> Result<Vec<PluginCard>, NativeRefusal>;
    fn load(
        &self,
        id: &NativeId,
        expected_abi: u32,
        budget: RealtimeBudget,
    ) -> Result<Box<dyn NativePluginInstance>, NativeRefusal>;
}

/// Deterministic native-plugin model with the same ABI and lifecycle contract
/// as a platform capsule, but no dynamic-library access.
#[derive(Default)]
pub struct ModelNativePluginPort {
    cards: BTreeMap<NativeId, PluginCard>,
}

impl ModelNativePluginPort {
    pub fn add(&mut self, card: PluginCard) {
        self.cards.insert(card.id.clone(), card);
    }
}

impl NativePluginPort for ModelNativePluginPort {
    fn cards(&self) -> Result<Vec<PluginCard>, NativeRefusal> {
        Ok(self.cards.values().cloned().collect())
    }

    fn load(
        &self,
        id: &NativeId,
        expected_abi: u32,
        budget: RealtimeBudget,
    ) -> Result<Box<dyn NativePluginInstance>, NativeRefusal> {
        budget.validate()?;
        let card = self.cards.get(id).ok_or(NativeRefusal::Unsupported)?;
        if card.abi != expected_abi {
            return Err(NativeRefusal::AbiMismatch {
                expected: expected_abi,
                found: card.abi,
            });
        }
        Ok(Box::new(ModelPlugin {
            card: card.clone(),
            budget,
            unloaded: false,
        }))
    }
}

struct ModelPlugin {
    card: PluginCard,
    budget: RealtimeBudget,
    unloaded: bool,
}

impl NativePluginInstance for ModelPlugin {
    fn card(&self) -> &PluginCard {
        &self.card
    }

    fn process(&mut self, frames: u32) -> Result<(), NativeRefusal> {
        if self.unloaded {
            Err(NativeRefusal::AlreadyClosed)
        } else if frames > self.budget.max_frames {
            Err(NativeRefusal::BudgetExceeded)
        } else {
            Ok(())
        }
    }

    fn cancel(&mut self) -> Result<(), NativeRefusal> {
        if self.unloaded {
            Err(NativeRefusal::AlreadyClosed)
        } else {
            self.unloaded = true;
            Err(NativeRefusal::Cancelled)
        }
    }

    fn unload(&mut self) -> Result<(), NativeRefusal> {
        if self.unloaded {
            Err(NativeRefusal::AlreadyClosed)
        } else {
            self.unloaded = true;
            Ok(())
        }
    }
}

/// Deterministic device model used by all portable conformance tests.
#[derive(Default)]
pub struct ModelAudioDevicePort {
    cards: BTreeMap<NativeId, AudioDeviceCard>,
    events: VecDeque<DeviceEvent>,
    callbacks: VecDeque<CallbackEvent>,
}

impl ModelAudioDevicePort {
    pub fn add(&mut self, card: AudioDeviceCard) {
        self.events.push_back(DeviceEvent::Added(card.clone()));
        self.cards.insert(card.id.clone(), card);
    }
    pub fn remove(&mut self, id: &NativeId) {
        self.cards.remove(id);
        self.events.push_back(DeviceEvent::Removed(id.clone()));
    }
    pub fn callback(&mut self, event: CallbackEvent) {
        self.callbacks.push_back(event);
    }
}

impl AudioDevicePort for ModelAudioDevicePort {
    fn cards(&self) -> Result<Vec<AudioDeviceCard>, NativeRefusal> {
        Ok(self.cards.values().cloned().collect())
    }
    fn poll_hotplug(&self) -> Result<Vec<DeviceEvent>, NativeRefusal> {
        Ok(self.events.iter().cloned().collect())
    }
    fn open(
        &self,
        id: &NativeId,
        sample_rate: u32,
        budget: RealtimeBudget,
    ) -> Result<Box<dyn AudioDeviceStream>, NativeRefusal> {
        budget.validate()?;
        let card = self.cards.get(id).ok_or(NativeRefusal::Unsupported)?;
        if !card.sample_rates.contains(&sample_rate) {
            return Err(NativeRefusal::Unsupported);
        }
        Ok(Box::new(ModelStream {
            owner: CallbackOwner { id: id.clone() },
            events: self.callbacks.clone(),
            closed: false,
        }))
    }
}

struct ModelStream {
    owner: CallbackOwner,
    events: VecDeque<CallbackEvent>,
    closed: bool,
}
impl AudioDeviceStream for ModelStream {
    fn callback_owner(&self) -> &CallbackOwner {
        &self.owner
    }
    fn poll_callback(&mut self) -> Result<Option<CallbackEvent>, NativeRefusal> {
        if self.closed {
            Err(NativeRefusal::AlreadyClosed)
        } else {
            Ok(self.events.pop_front())
        }
    }
    fn cancel(&mut self) -> Result<(), NativeRefusal> {
        if self.closed {
            Err(NativeRefusal::AlreadyClosed)
        } else {
            self.closed = true;
            Err(NativeRefusal::Cancelled)
        }
    }
    fn close(&mut self) -> Result<(), NativeRefusal> {
        if self.closed {
            Err(NativeRefusal::AlreadyClosed)
        } else {
            self.closed = true;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
