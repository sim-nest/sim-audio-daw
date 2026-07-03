use std::thread;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_audio_graph_core::{Processor, Transport};
use sim_lib_stream_host::{ProcessRingPush, ProcessRingSnapshot, ProcessSharedRing};

use crate::{
    LiveGraphConfig, LiveGraphRunner, LiveProcessReport, LiveSteadyStateSnapshot,
    realtime_local_audio_profile,
};

/// Local placement sites supported by the live audio graph backbone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivePlacementSite {
    /// Runs inline on the caller (cooperative coroutine).
    Coroutine,
    /// Runs on a dedicated worker thread.
    Thread,
    /// Runs directly in the host audio callback (realtime, audio-clock thread).
    HostCallback,
    /// Runs in a separate process behind shared rings.
    Process,
}

/// Capacity snapshot for a placed live node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LivePlacementSnapshot {
    site: LivePlacementSite,
    runner: LiveSteadyStateSnapshot,
    process_request_ring: Option<ProcessRingSnapshot>,
    process_response_ring: Option<ProcessRingSnapshot>,
}

/// Live graph runner bound to one local placement site.
#[derive(Debug)]
pub struct LivePlacedNode<P> {
    site: LivePlacementSite,
    runner: LiveGraphRunner<P>,
    process_request_ring: Option<ProcessSharedRing<ProcessSiteMessage>>,
    process_response_ring: Option<ProcessSharedRing<ProcessSiteMessage>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ProcessSiteMessage {
    Render { frames: usize, transport: Transport },
    Rendered { frames: u32 },
}

impl LivePlacementSite {
    /// Returns every placement site in a stable order.
    pub const fn all() -> [Self; 4] {
        [
            Self::Coroutine,
            Self::Thread,
            Self::HostCallback,
            Self::Process,
        ]
    }

    /// Returns the stable wire name for this site.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Coroutine => "coroutine",
            Self::Thread => "thread",
            Self::HostCallback => "host-callback",
            Self::Process => "process",
        }
    }

    /// Returns the namespaced symbol for this site.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified("stream/site", self.name())
    }

    /// Returns whether this site runs on the audio-clock thread.
    pub const fn runs_on_audio_clock_thread(self) -> bool {
        matches!(self, Self::HostCallback)
    }

    /// Returns whether this site communicates over a shared process ring.
    pub const fn uses_process_ring(self) -> bool {
        matches!(self, Self::Process)
    }

    /// Returns whether realtime-pinned processors may run at this site.
    pub const fn allows_realtime_pin(self) -> bool {
        self.runs_on_audio_clock_thread()
    }

    fn validate_processor(self, realtime_pin: bool) -> Result<()> {
        if realtime_pin && !self.allows_realtime_pin() {
            return Err(Error::Eval(format!(
                "realtime-pinned live audio node cannot run at {} site",
                self.name()
            )));
        }
        Ok(())
    }
}

impl<P: Processor> LivePlacedNode<P> {
    /// Places a processor at a site, validating realtime pinning and creating
    /// any process rings the site requires.
    pub fn new(processor: P, config: LiveGraphConfig, site: LivePlacementSite) -> Result<Self> {
        site.validate_processor(processor.realtime_pin())?;
        let runner = if site.runs_on_audio_clock_thread() {
            LiveGraphRunner::new_realtime(processor, config, &realtime_local_audio_profile())?
        } else {
            LiveGraphRunner::new(processor, config)?
        };
        let process_request_ring = site
            .uses_process_ring()
            .then(|| ProcessSharedRing::with_capacity(1))
            .transpose()?;
        let process_response_ring = site
            .uses_process_ring()
            .then(|| ProcessSharedRing::with_capacity(1))
            .transpose()?;
        Ok(Self {
            site,
            runner,
            process_request_ring,
            process_response_ring,
        })
    }

    /// Returns the placement site.
    pub fn site(&self) -> LivePlacementSite {
        self.site
    }

    /// Returns a shared reference to the underlying runner.
    pub fn runner(&self) -> &LiveGraphRunner<P> {
        &self.runner
    }

    /// Returns a mutable reference to the underlying runner.
    pub fn runner_mut(&mut self) -> &mut LiveGraphRunner<P> {
        &mut self.runner
    }

    /// Processes one interleaved block, dispatching to the site's execution
    /// strategy (inline, worker thread, or process ring).
    pub fn process_interleaved_f32(
        &mut self,
        input: Option<&[f32]>,
        output: &mut [f32],
        frames: usize,
        transport: Transport,
    ) -> Result<LiveProcessReport> {
        match self.site {
            LivePlacementSite::Coroutine | LivePlacementSite::HostCallback => self
                .runner
                .process_interleaved_f32(input, output, frames, transport),
            LivePlacementSite::Thread => {
                run_on_worker_thread(&mut self.runner, input, output, frames, transport)
            }
            LivePlacementSite::Process => {
                self.process_via_process_ring(input, output, frames, transport)
            }
        }
    }

    /// Captures the placed node's steady-state capacity snapshot.
    pub fn steady_state_snapshot(&self) -> LivePlacementSnapshot {
        LivePlacementSnapshot {
            site: self.site,
            runner: self.runner.steady_state_snapshot(),
            process_request_ring: self
                .process_request_ring
                .as_ref()
                .map(|ring| ring.snapshot()),
            process_response_ring: self
                .process_response_ring
                .as_ref()
                .map(|ring| ring.snapshot()),
        }
    }

    fn process_via_process_ring(
        &mut self,
        input: Option<&[f32]>,
        output: &mut [f32],
        frames: usize,
        transport: Transport,
    ) -> Result<LiveProcessReport> {
        let work = ProcessSiteMessage::Render { frames, transport };
        {
            let request_ring = self.process_request_ring.as_mut().ok_or_else(|| {
                Error::Eval("process site is missing its request ring".to_owned())
            })?;
            accept_process_push(request_ring.try_push(work), "request")?;
        }
        let work = self
            .process_request_ring
            .as_mut()
            .and_then(ProcessSharedRing::try_pop)
            .ok_or_else(|| Error::Eval("process site request ring lost work".to_owned()))?;
        let ProcessSiteMessage::Render { frames, transport } = work else {
            return Err(Error::Eval(
                "process site request ring received a response".to_owned(),
            ));
        };

        let report = run_on_worker_thread(&mut self.runner, input, output, frames, transport)?;
        {
            let response_ring = self.process_response_ring.as_mut().ok_or_else(|| {
                Error::Eval("process site is missing its response ring".to_owned())
            })?;
            accept_process_push(
                response_ring.try_push(ProcessSiteMessage::Rendered {
                    frames: report.frames(),
                }),
                "response",
            )?;
        }
        let response = self
            .process_response_ring
            .as_mut()
            .and_then(ProcessSharedRing::try_pop)
            .ok_or_else(|| Error::Eval("process site response ring lost output".to_owned()))?;
        match response {
            ProcessSiteMessage::Rendered { frames } if frames == report.frames() => Ok(report),
            ProcessSiteMessage::Rendered { .. } => Err(Error::Eval(
                "process site response ring returned the wrong frame count".to_owned(),
            )),
            ProcessSiteMessage::Render { .. } => Err(Error::Eval(
                "process site response ring received a request".to_owned(),
            )),
        }
    }
}

impl LivePlacementSnapshot {
    /// Returns the placement site captured by the snapshot.
    pub fn site(&self) -> LivePlacementSite {
        self.site
    }

    /// Returns the runner's steady-state capacity snapshot.
    pub fn runner(&self) -> &LiveSteadyStateSnapshot {
        &self.runner
    }

    /// Returns the request ring snapshot, if this site uses a process ring.
    pub fn process_request_ring(&self) -> Option<ProcessRingSnapshot> {
        self.process_request_ring
    }

    /// Returns the response ring snapshot, if this site uses a process ring.
    pub fn process_response_ring(&self) -> Option<ProcessRingSnapshot> {
        self.process_response_ring
    }
}

fn run_on_worker_thread<P: Processor>(
    runner: &mut LiveGraphRunner<P>,
    input: Option<&[f32]>,
    output: &mut [f32],
    frames: usize,
    transport: Transport,
) -> Result<LiveProcessReport> {
    match thread::scope(|scope| {
        scope
            .spawn(move || runner.process_interleaved_f32(input, output, frames, transport))
            .join()
    }) {
        Ok(result) => result,
        Err(_) => Err(Error::Eval(
            "live audio worker thread panicked while processing".to_owned(),
        )),
    }
}

fn accept_process_push<T>(push: ProcessRingPush<T>, role: &str) -> Result<()> {
    match push {
        ProcessRingPush::Accepted => Ok(()),
        ProcessRingPush::DroppedNewest(_) => {
            Err(Error::Eval(format!("process site {role} ring is full")))
        }
        ProcessRingPush::Closed(_) => {
            Err(Error::Eval(format!("process site {role} ring is closed")))
        }
    }
}
