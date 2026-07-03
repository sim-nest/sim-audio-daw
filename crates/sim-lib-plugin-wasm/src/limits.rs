//! Resource-limit policy for WebAssembly audio plugins.

/// Resource limits applied to every WebAssembly plugin instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmResourceLimits {
    /// Wasmtime fuel units available for each audio process call.
    pub fuel_per_process: u64,
    /// Maximum number of 64 KiB linear-memory pages per instance.
    pub max_memory_pages: u32,
}

impl Default for WasmResourceLimits {
    fn default() -> Self {
        Self {
            fuel_per_process: 10_000_000,
            max_memory_pages: 64,
        }
    }
}

impl WasmResourceLimits {
    /// Returns a tighter profile for tests and low-trust plugin loads.
    pub fn strict() -> Self {
        Self {
            fuel_per_process: 1_000_000,
            max_memory_pages: 16,
        }
    }

    /// Returns a roomier profile for trusted local plugin development.
    pub fn permissive() -> Self {
        Self {
            fuel_per_process: 100_000_000,
            max_memory_pages: 256,
        }
    }

    /// Returns the maximum memory size in bytes.
    pub fn max_memory_bytes(self) -> usize {
        self.max_memory_pages as usize * 64 * 1024
    }
}
