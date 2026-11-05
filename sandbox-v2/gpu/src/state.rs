//! Double-buffered grid state living in GPU storage buffers.

use crate::layout::{CellRaw, MAX_CELLS};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Gpu;

const CELL_BYTES: u64 = std::mem::size_of::<CellRaw>() as u64;

/// Why a [`GridState`] could not be created.
#[derive(Debug, PartialEq, Eq)]
pub enum GridError {
    /// Width or height is zero.
    Empty,
    /// `w * h` exceeds [`MAX_CELLS`].
    TooLarge {
        /// Requested width.
        w: u32,
        /// Requested height.
        h: u32,
    },
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::Empty => write!(f, "grid dimensions must be non-zero"),
            GridError::TooLarge { w, h } => {
                write!(f, "grid {w}x{h} exceeds the {MAX_CELLS}-cell limit")
            }
        }
    }
}

impl std::error::Error for GridError {}

/// One of the two cell buffers.
struct Buffers {
    cells: wgpu::Buffer,
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Two GPU cell buffers; `cur` selects the current one.
pub struct GridState {
    id: u64,
    w: u32,
    h: u32,
    bufs: [Buffers; 2],
    cur: usize,
}

impl GridState {
    /// Allocates two zeroed `w` x `h` buffers. Rejects empty grids and grids
    /// above [`MAX_CELLS`] before allocating anything.
    ///
    /// Note: values stored here are not sanitized. Later shader stages clamp
    /// NaN/Inf in float fields; uploaded data is trusted as-is.
    pub fn new(gpu: &Gpu, w: u32, h: u32) -> Result<GridState, GridError> {
        if w == 0 || h == 0 {
            return Err(GridError::Empty);
        }
        if u64::from(w) * u64::from(h) > u64::from(MAX_CELLS) {
            return Err(GridError::TooLarge { w, h });
        }
        let size = u64::from(w) * u64::from(h) * CELL_BYTES;
        let make = |label| Buffers {
            cells: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        };
        Ok(GridState {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            w,
            h,
            bufs: [make("cells_a"), make("cells_b")],
            cur: 0,
        })
    }

    /// Process-unique id of this grid (lets passes cache bind groups per grid).
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// 0 or 1: which of the two buffers is current.
    pub(crate) fn current_index(&self) -> usize {
        self.cur
    }

    /// Buffer by index (0 or 1).
    pub(crate) fn buffer(&self, index: usize) -> &wgpu::Buffer {
        &self.bufs[index].cells
    }

    /// Grid width in cells.
    pub fn width(&self) -> u32 {
        self.w
    }

    /// Grid height in cells.
    pub fn height(&self) -> u32 {
        self.h
    }

    /// The current buffer.
    pub fn current(&self) -> &wgpu::Buffer {
        &self.bufs[self.cur].cells
    }

    /// The non-current buffer.
    pub fn other(&self) -> &wgpu::Buffer {
        &self.bufs[1 - self.cur].cells
    }

    /// Makes the other buffer current.
    pub fn swap(&mut self) {
        self.cur = 1 - self.cur;
    }

    /// Writes `cells` (row-major, `w * h` long) into the current buffer.
    ///
    /// # Panics
    /// If `cells.len() != w * h`.
    pub fn upload(&self, gpu: &Gpu, cells: &[CellRaw]) {
        assert_eq!(cells.len() as u64, u64::from(self.w) * u64::from(self.h));
        gpu.queue
            .write_buffer(self.current(), 0, bytemuck::cast_slice(cells));
    }

    /// Copies the current buffer back to the CPU (blocking).
    pub fn readback(&self, gpu: &Gpu) -> Vec<CellRaw> {
        let size = u64::from(self.w) * u64::from(self.h) * CELL_BYTES;
        let staging = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(self.current(), 0, &staging, 0, size);
        gpu.queue.submit(Some(enc.finish()));
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll readback");
        let view = staging.slice(..).get_mapped_range().expect("mapped range");
        let out = bytemuck::cast_slice(&view).to_vec();
        drop(view);
        staging.unmap();
        out
    }
}
