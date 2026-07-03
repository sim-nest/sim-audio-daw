use sim_kernel::{Error, Result};

/// Bump allocator for per-block `f32` scratch buffers.
///
/// Processors borrow zeroed slices from the arena during a process call; the
/// graph calls [`BlockArena::reset`] before each node so its capacity is reused
/// without per-block heap allocation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlockArena {
    f32_cells: Vec<f32>,
    used_f32: usize,
}

impl BlockArena {
    /// Creates an arena with no capacity.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates an arena pre-sized for `capacity` `f32` cells.
    pub fn with_f32_capacity(capacity: usize) -> Self {
        Self {
            f32_cells: vec![0.0; capacity],
            used_f32: 0,
        }
    }

    /// Releases all outstanding allocations, keeping the backing capacity.
    pub fn reset(&mut self) {
        self.used_f32 = 0;
    }

    /// Returns the total `f32` capacity of the arena.
    pub fn f32_capacity(&self) -> usize {
        self.f32_cells.len()
    }

    /// Allocates and zero-fills a `len`-cell `f32` slice.
    ///
    /// Fails if the request overflows or exceeds the remaining capacity.
    pub fn alloc_f32(&mut self, len: usize) -> Result<&mut [f32]> {
        let start = self.used_f32;
        let end = start
            .checked_add(len)
            .ok_or_else(|| Error::Eval("audio block arena allocation overflow".to_owned()))?;
        if end > self.f32_cells.len() {
            return Err(Error::Eval(format!(
                "audio block arena exhausted: requested {len} f32 cells with {} remaining",
                self.f32_cells.len().saturating_sub(start)
            )));
        }
        self.used_f32 = end;
        let cells = &mut self.f32_cells[start..end];
        cells.fill(0.0);
        Ok(cells)
    }
}
