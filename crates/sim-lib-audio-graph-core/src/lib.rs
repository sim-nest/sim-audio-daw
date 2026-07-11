#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Pure Rust audio processor graph primitives.
//!
//! The core graph is hardware-free: callers provide processors, prepare the
//! graph, and can render deterministic offline blocks in tests or previews.
//!
//! ```rust
//! use sim_lib_audio_graph_core::{
//!     Graph, PrepareConfig, ProcessBlock, Processor,
//! };
//!
//! #[derive(Default)]
//! struct CopyNode;
//!
//! impl Processor for CopyNode {
//!     fn prepare(&mut self, _cfg: PrepareConfig) {}
//!
//!     fn reset(&mut self) {}
//!
//!     fn process(&mut self, block: &mut ProcessBlock<'_>) {
//!         let frames = block.frames as usize;
//!         for (input, output) in block.in_audio.iter().zip(block.out_audio.iter_mut()) {
//!             output[..frames].copy_from_slice(&input[..frames]);
//!         }
//!     }
//! }
//!
//! let mut graph = Graph::new();
//! graph.add_node("copy", Box::<CopyNode>::default(), 1, 1).unwrap();
//! graph.prepare(48_000, 4).unwrap();
//!
//! let output = graph.process_offline(&[vec![0.25, -0.5]], 2).unwrap();
//! assert_eq!(output, vec![vec![0.25, -0.5]]);
//! ```

mod arena;
mod block;
mod bridge;
mod citizen;
mod graph;
mod patch;
mod port;
mod processor;

pub use arena::BlockArena;
pub use block::ProcessBlock;
pub use bridge::DomainBridgeProcessor;
pub use citizen::{
    AudioGraphNodeConfig, AudioGraphPatchDescriptor, audio_graph_node_config_class_symbol,
    audio_graph_patch_class_symbol,
};
pub use graph::{Cable, Graph};
pub use patch::{Patch, PatchNode};
pub use port::{PortDecl, PortDir, PortMedia, PortUri};
pub use processor::{
    BlockEvent, EventSink, NullEventSink, PrepareConfig, Processor, ProcessorDescriptor, Transport,
};
pub use sim_lib_stream_core::{
    BridgeLatency, ClockDomain, DomainBridgeDescriptor, DomainBridgeKind, LatencyClass,
    RateContract,
};

/// Cookbook recipes for this lib, embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod bridge_tests;
#[cfg(test)]
mod tests;
