//! The WebAssembly runtime of owned Peras modules.
//!
//! Two allocators, with the same exports: the **debug allocator** that
//! decision 0001 requires before the direct backend compiles anything that
//! allocates, and the **TLSF allocator** for production
//! ([`TLSF_ALLOCATOR`]). The debug allocator is a standalone module,
//! [`DEBUG_ALLOCATOR`], written in `runtime/alloc_debug.wat` and assembled when
//! this crate builds. It exports its `memory` and:
//!
//! | Export | Signature | Does |
//! | --- | --- | --- |
//! | `alloc` | `(size, site) -> ptr` | allocates `size` bytes, 16-byte aligned, filled with `0xCD` |
//! | `release` | `(ptr, site)` | checks the block, poisons it with `0xDD`, quarantines it |
//! | `live_count`, `live_bytes` | `() -> i32` | blocks still alive, and their bytes |
//! | `allocations` | `() -> i32` | allocations made since the start |
//! | `next_live` | `(after) -> ptr` | the next live block, `0` to start, `0` at the end |
//! | `block_size`, `block_site`, `block_serial` | `(ptr) -> i32` | what a live block records |
//! | `check_leaks` | `()` | faults with [`Fault::Leak`] if a block is still alive |
//!
//! It imports `peras.fault(kind, ptr, alloc_site, free_site)`, called before it
//! traps. A site is the number perasc gives to a place in the source that
//! allocates or releases; `0` means unknown.

mod prune;

pub use prune::{PruneError, prune};

/// The debug allocator, as a WebAssembly module.
pub const DEBUG_ALLOCATOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/alloc_debug.wasm"));

/// The production allocator, TLSF, as a WebAssembly module: the same exports
/// as [`DEBUG_ALLOCATOR`], every operation in bounded time. It catches only
/// what costs nothing, an invalid or double release; it keeps no site per
/// block, so `next_live` returns 0, and `check_leaks` reports the number of
/// blocks alive, without their site. It also exports `heap_end`, the first
/// byte its blocks do not use.
pub const TLSF_ALLOCATOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/alloc_tlsf.wasm"));

/// [`DEBUG_ALLOCATOR`] without WebAssembly SIMD, for `--simd off`: the same
/// functions, a byte at a time where the other compares sixteen.
pub const DEBUG_ALLOCATOR_SCALAR: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/alloc_debug_scalar.wasm"));

/// [`TLSF_ALLOCATOR`] without WebAssembly SIMD, for `--simd off`.
pub const TLSF_ALLOCATOR_SCALAR: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/alloc_tlsf_scalar.wasm"));

/// Which allocator a module is linked with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Allocator {
    /// Every memory bug caught, for development.
    #[default]
    Debug,
    /// TLSF: bounded time, for production.
    Tlsf,
}

impl Allocator {
    /// The allocator module, with WebAssembly SIMD.
    #[must_use]
    pub const fn module(self) -> &'static [u8] {
        self.runtime(true)
    }

    /// The allocator module, with WebAssembly SIMD or without.
    #[must_use]
    pub const fn runtime(self, simd: bool) -> &'static [u8] {
        match (self, simd) {
            (Self::Debug, true) => DEBUG_ALLOCATOR,
            (Self::Tlsf, true) => TLSF_ALLOCATOR,
            (Self::Debug, false) => DEBUG_ALLOCATOR_SCALAR,
            (Self::Tlsf, false) => TLSF_ALLOCATOR_SCALAR,
        }
    }
}

/// Added to the element size of a buffer whose elements are buffers it owns:
/// releasing or copying it releases or copies them too.
pub const DEEP: u32 = 0x100;

/// Smallest address the heap uses: lower addresses are never valid pointers.
pub const HEAP_BASE: u32 = 4096;

/// Blocks a released block waits behind before its memory can be reused.
pub const QUARANTINE: u32 = 256;

/// Byte written into fresh memory, so that reading it before writing shows.
pub const FRESH: u8 = 0xCD;
/// Byte written into released memory.
pub const POISON: u8 = 0xDD;
/// Byte of the red zone after each block.
pub const RED_ZONE: u8 = 0xFD;

/// A memory bug the debug allocator caught. The number is the `kind` passed to
/// `peras.fault`; the other arguments are described for each kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The memory cannot grow any more. `ptr` is 0; `alloc_site` is the allocation.
    OutOfMemory = 1,
    /// More than 1 GiB requested at once. `ptr` holds the size requested.
    TooLarge = 2,
    /// Bytes past the end of a block were written. Seen when it is released.
    Overflow = 3,
    /// `release` of something that is not a live block.
    InvalidRelease = 4,
    /// `release` of a block already released. `free_site` is the second release.
    DoubleRelease = 5,
    /// A released block was written to. Seen when it leaves quarantine or is reused.
    UseAfterRelease = 6,
    /// A block is still alive at the end. `free_site` holds the number of live blocks.
    Leak = 7,
}

impl Fault {
    /// The fault of a `kind` number, if there is one.
    #[must_use]
    pub const fn from_kind(kind: i32) -> Option<Self> {
        Some(match kind {
            1 => Self::OutOfMemory,
            2 => Self::TooLarge,
            3 => Self::Overflow,
            4 => Self::InvalidRelease,
            5 => Self::DoubleRelease,
            6 => Self::UseAfterRelease,
            7 => Self::Leak,
            _ => return None,
        })
    }
}
