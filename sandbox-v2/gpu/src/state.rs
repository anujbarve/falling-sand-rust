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

/// Kinds of sound event counted on the GPU, in counter-buffer order.
///
/// Each pass that produces an event adds 1 to `events[kind]` (a
/// `array<atomic<u32>, 4>` storage buffer owned by [`GridState`]):
/// * `Flow`: a liquid cell (water or lava) moved.
/// * `Impact`: a falling grain or drop moving at least 1 cell/tick (60
///   cells/s) was stopped.
/// * `Crackle`: a flammable cell ignited, or lava froze to rock.
/// * `Boom`: gunpowder ignited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum EventKind {
    /// Liquid moved.
    Flow = 0,
    /// A fast fall was stopped.
    Impact = 1,
    /// Ignition or lava solidifying.
    Crackle = 2,
    /// Gunpowder ignited.
    Boom = 3,
}

/// Number of [`EventKind`]s (length of the event counter buffer).
pub const EVENT_KINDS: usize = 4;

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
    events: wgpu::Buffer,
}

impl GridState {
    /// Allocates two zeroed `w` x `h` buffers. Rejects empty grids and grids
    /// above [`MAX_CELLS`] before allocating anything.
    ///
    /// [`GridState::upload`] stores data as-is; [`GridState::upload_sanitized`]
    /// (the app's load/scene path) replaces NaN/inf first and counts them.
    /// Shader stages also clamp NaN/inf in the fields they own.
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
            events: gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("events"),
                size: (EVENT_KINDS * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
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

    /// The event counter buffer: [`EVENT_KINDS`] `u32` counters indexed by
    /// [`EventKind`]. Passes only add; the owner copies and clears it.
    pub fn events(&self) -> &wgpu::Buffer {
        &self.events
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

    /// Like [`GridState::upload`], but first replaces non-finite floats (a
    /// NaN/inf `temp` becomes ambient, any other non-finite float 0) and
    /// returns how many were replaced. This is the load/brush entry point.
    ///
    /// # Panics
    /// If `cells.len() != w * h`.
    pub fn upload_sanitized(&self, gpu: &Gpu, cells: &[CellRaw]) -> u32 {
        let mut bad = 0;
        let mut fix = |v: f32, default: f32| {
            if v.is_finite() {
                v
            } else {
                bad += 1;
                default
            }
        };
        let clean: Vec<CellRaw> = cells
            .iter()
            .map(|c| CellRaw {
                temp: fix(c.temp, crate::air::AMBIENT_K),
                fuel: fix(c.fuel, 0.0),
                vx: fix(c.vx, 0.0),
                vy: fix(c.vy, 0.0),
                vapor: fix(c.vapor, 0.0),
                pressure: fix(c.pressure, 0.0),
                ..*c
            })
            .collect();
        self.upload(gpu, &clean);
        bad
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
