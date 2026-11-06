//! The debug allocator, run in an interpreter: each memory bug it promises to
//! catch is committed on purpose, and a long random run checks it against a
//! model of what should be alive.

use perasc_runtime::{DEBUG_ALLOCATOR, FRESH, Fault, HEAP_BASE, POISON, QUARANTINE, RED_ZONE};
use wasmi::{
    Caller, Engine, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

/// A fault reported through `peras.fault`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reported {
    fault: Option<Fault>,
    ptr: u32,
    alloc_site: u32,
    free_site: u32,
}

#[allow(clippy::cast_sign_loss)]
fn u(x: i32) -> u32 {
    x as u32
}

#[allow(clippy::cast_possible_wrap)]
fn i(x: u32) -> i32 {
    x as i32
}

/// What the host keeps: the last fault, and a memory limit.
struct Host {
    reported: Option<Reported>,
    limits: StoreLimits,
}

/// One instance of the allocator.
struct Heap {
    store: Store<Host>,
    memory: Memory,
    alloc: TypedFunc<(i32, i32), i32>,
    release: TypedFunc<(i32, i32), ()>,
    live_count: TypedFunc<(), i32>,
    live_bytes: TypedFunc<(), i32>,
    next_live: TypedFunc<i32, i32>,
    block_site: TypedFunc<i32, i32>,
    check_leaks: TypedFunc<(), ()>,
}

impl Heap {
    fn new() -> Result<Self, wasmi::Error> {
        Self::limited(usize::MAX)
    }

    /// An allocator whose memory cannot grow past `max_bytes`.
    fn limited(max_bytes: usize) -> Result<Self, wasmi::Error> {
        let engine = Engine::default();
        let module = Module::new(&engine, DEBUG_ALLOCATOR)?;
        let host = Host {
            reported: None,
            limits: StoreLimitsBuilder::new().memory_size(max_bytes).build(),
        };
        let mut store = Store::new(&engine, host);
        store.limiter(|host| &mut host.limits);
        let mut linker = Linker::new(&engine);
        linker.func_wrap(
            "peras",
            "fault",
            |mut caller: Caller<'_, Host>,
             kind: i32,
             ptr: i32,
             alloc_site: i32,
             free_site: i32|
             -> Result<(), wasmi::Error> {
                caller.data_mut().reported = Some(Reported {
                    fault: Fault::from_kind(kind),
                    ptr: u(ptr),
                    alloc_site: u(alloc_site),
                    free_site: u(free_site),
                });
                Err(wasmi::Error::new("peras fault"))
            },
        )?;
        let instance = linker.instantiate_and_start(&mut store, &module)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| wasmi::Error::new("no memory"))?;
        Ok(Self {
            alloc: instance.get_typed_func(&store, "alloc")?,
            release: instance.get_typed_func(&store, "release")?,
            live_count: instance.get_typed_func(&store, "live_count")?,
            live_bytes: instance.get_typed_func(&store, "live_bytes")?,
            next_live: instance.get_typed_func(&store, "next_live")?,
            block_site: instance.get_typed_func(&store, "block_site")?,
            check_leaks: instance.get_typed_func(&store, "check_leaks")?,
            memory,
            store,
        })
    }

    /// Allocates, or returns the fault.
    fn alloc(&mut self, bytes: u32, site: u32) -> Result<u32, Reported> {
        let result = self.alloc.call(&mut self.store, (i(bytes), i(site)));
        self.outcome(result).map(u)
    }

    /// Releases, or returns the fault.
    fn release(&mut self, ptr: u32, site: u32) -> Result<(), Reported> {
        let result = self.release.call(&mut self.store, (i(ptr), i(site)));
        self.outcome(result)
    }

    fn check_leaks(&mut self) -> Result<(), Reported> {
        let result = self.check_leaks.call(&mut self.store, ());
        self.outcome(result)
    }

    fn outcome<T>(&mut self, result: Result<T, wasmi::Error>) -> Result<T, Reported> {
        result.map_err(|_| {
            self.store.data_mut().reported.take().unwrap_or(Reported {
                fault: None,
                ptr: 0,
                alloc_site: 0,
                free_site: 0,
            })
        })
    }

    fn live(&mut self) -> (u32, u32) {
        let count = self.live_count.call(&mut self.store, ()).unwrap_or(-1);
        let bytes = self.live_bytes.call(&mut self.store, ()).unwrap_or(-1);
        (u(count), u(bytes))
    }

    /// Every live block, with its allocation site, in address order.
    fn live_blocks(&mut self) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        let mut ptr = 0;
        loop {
            ptr = self.next_live.call(&mut self.store, ptr).unwrap_or(0);
            if ptr == 0 {
                return out;
            }
            let site = self.block_site.call(&mut self.store, ptr).unwrap_or(-1);
            out.push((u(ptr), u(site)));
        }
    }

    fn bytes(&self, ptr: u32, len: u32) -> Vec<u8> {
        let data = self.memory.data(&self.store);
        let start = ptr as usize;
        data.get(start..start + len as usize)
            .unwrap_or_default()
            .to_vec()
    }

    fn write(&mut self, ptr: u32, bytes: &[u8]) {
        let start = ptr as usize;
        if let Some(target) = self
            .memory
            .data_mut(&mut self.store)
            .get_mut(start..start + bytes.len())
        {
            target.copy_from_slice(bytes);
        }
    }
}

fn heap() -> Heap {
    Heap::new().unwrap_or_else(|e| unreachable!("the allocator does not load: {e}"))
}

fn fault(kind: Fault, ptr: u32, alloc_site: u32, free_site: u32) -> Reported {
    Reported {
        fault: Some(kind),
        ptr,
        alloc_site,
        free_site,
    }
}

#[test]
fn the_module_is_valid_webassembly() {
    let validated = wasmparser::Validator::new().validate_all(DEBUG_ALLOCATOR);
    assert!(validated.is_ok(), "{:?}", validated.err());
}

#[test]
fn blocks_are_aligned_fresh_and_counted() {
    let mut heap = heap();
    let a = heap.alloc(10, 1);
    let b = heap.alloc(0, 2);
    let c = heap.alloc(33, 3);
    let (a, b, c) = (a.unwrap_or(0), b.unwrap_or(0), c.unwrap_or(0));
    for ptr in [a, b, c] {
        assert!(ptr >= HEAP_BASE && ptr.is_multiple_of(16), "{ptr}");
    }
    assert!(a + 10 <= b && b <= c, "blocks overlap: {a} {b} {c}");
    assert_eq!(heap.bytes(a, 10), [FRESH; 10]);
    // The red zone starts right after the 10 bytes asked for.
    assert_eq!(heap.bytes(a + 10, 6), [RED_ZONE; 6]);
    assert_eq!(heap.live(), (3, 43));
    assert_eq!(heap.live_blocks(), [(a, 1), (b, 2), (c, 3)]);
    assert_eq!(heap.release(b, 20), Ok(()));
    assert_eq!(heap.live(), (2, 43));
    assert_eq!(heap.live_blocks(), [(a, 1), (c, 3)]);
}

#[test]
fn released_memory_is_poisoned() {
    let mut heap = heap();
    let ptr = heap.alloc(24, 1).unwrap_or(0);
    heap.write(ptr, &[7; 24]);
    assert_eq!(heap.release(ptr, 2), Ok(()));
    assert_eq!(heap.bytes(ptr, 32), [POISON; 32]);
}

#[test]
fn a_double_release_is_caught_with_both_sites() {
    let mut heap = heap();
    let ptr = heap.alloc(8, 11).unwrap_or(0);
    assert_eq!(heap.release(ptr, 12), Ok(()));
    assert_eq!(
        heap.release(ptr, 13),
        Err(fault(Fault::DoubleRelease, ptr, 11, 13))
    );
}

#[test]
fn releasing_anything_but_a_live_block_is_caught() {
    let mut heap = heap();
    let ptr = heap.alloc(64, 1).unwrap_or(0);
    for wrong in [0, 16, HEAP_BASE, ptr + 16, ptr + 1, 0x7FFF_FFF0] {
        assert_eq!(
            heap.release(wrong, 5).map_err(|r| r.fault),
            Err(Some(Fault::InvalidRelease)),
            "{wrong}"
        );
    }
    assert_eq!(heap.live(), (1, 64));
}

#[test]
fn a_write_past_the_end_is_caught_at_release() {
    let mut heap = heap();
    let ptr = heap.alloc(10, 4).unwrap_or(0);
    // One byte past the 10 asked for, still inside the rounded block.
    heap.write(ptr + 10, &[0]);
    assert_eq!(heap.release(ptr, 6), Err(fault(Fault::Overflow, ptr, 4, 6)));
}

#[test]
fn a_write_after_release_is_caught_when_the_block_leaves_quarantine() {
    let mut heap = heap();
    let victim = heap.alloc(16, 8).unwrap_or(0);
    assert_eq!(heap.release(victim, 9), Ok(()));
    heap.write(victim + 3, &[1]);
    // Push the victim out of quarantine with other releases.
    let mut caught = None;
    for n in 0..QUARANTINE {
        let other = heap.alloc(48, 100).unwrap_or(0);
        if let Err(reported) = heap.release(other, 100 + n) {
            caught = Some(reported);
            break;
        }
    }
    assert_eq!(caught, Some(fault(Fault::UseAfterRelease, victim, 8, 9)));
}

#[test]
fn memory_is_reused_only_after_quarantine() {
    let mut heap = heap();
    let first = heap.alloc(32, 1).unwrap_or(0);
    assert_eq!(heap.release(first, 1), Ok(()));
    let mut seen = Vec::new();
    for _ in 0..QUARANTINE {
        let ptr = heap.alloc(32, 2).unwrap_or(0);
        seen.push(ptr);
        assert_eq!(heap.release(ptr, 2), Ok(()));
    }
    assert!(!seen.contains(&first), "reused while quarantined");
    // The first block has left quarantine: the next block of its size reuses it.
    assert_eq!(heap.alloc(32, 3), Ok(first));
    assert_eq!(heap.bytes(first, 32), [FRESH; 32]);
}

#[test]
fn leaks_are_reported_with_the_site_that_allocated() {
    let mut heap = heap();
    let kept = heap.alloc(5, 42).unwrap_or(0);
    let released = heap.alloc(5, 43).unwrap_or(0);
    assert_eq!(heap.release(released, 44), Ok(()));
    assert_eq!(heap.check_leaks(), Err(fault(Fault::Leak, kept, 42, 1)));
    assert_eq!(heap.release(kept, 45), Ok(()));
    assert_eq!(heap.check_leaks(), Ok(()));
}

#[test]
fn memory_grows_and_runs_out_cleanly() {
    let mut heap = heap();
    // Three pages' worth, past the first 64 KiB.
    let big = heap.alloc(200_000, 7).unwrap_or(0);
    assert!(big > 0);
    heap.write(big + 200_000, &[1]);
    assert_eq!(
        heap.release(big, 7).map_err(|r| r.fault),
        Err(Some(Fault::Overflow))
    );
    assert_eq!(
        heap.alloc(0x4000_0001, 9),
        Err(fault(Fault::TooLarge, 0x4000_0001, 9, 0))
    );
    // With memory capped at 16 MiB, the third block of 6 MiB cannot fit.
    let mut heap = Heap::limited(16 << 20).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(heap.alloc(6 << 20, 10).is_ok());
    assert!(heap.alloc(6 << 20, 11).is_ok());
    assert_eq!(
        heap.alloc(6 << 20, 12),
        Err(fault(Fault::OutOfMemory, 0, 12, 0))
    );
    // Nothing was half-allocated.
    assert_eq!(heap.live(), (2, 12 << 20));
}

/// A small deterministic generator, so that a failure can be replayed.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        u32::try_from(self.0 >> 32).unwrap_or(0)
    }
}

/// Thousands of random allocations and releases against a model: live blocks
/// never overlap, keep the bytes written into them, and match the counters.
#[test]
fn a_random_run_matches_the_model() {
    let mut heap = heap();
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    // (ptr, size, fill byte, site)
    let mut live: Vec<(u32, u32, u8, u32)> = Vec::new();
    // 5,000 steps: about 2,200 releases, so about 2,000 blocks leave quarantine
    // and are checked on the way.
    for step in 0..5_000_u32 {
        let release = !live.is_empty() && rng.next() % 100 < 45;
        if release {
            let index = (rng.next() as usize) % live.len();
            let (ptr, size, fill, _) = live.swap_remove(index);
            assert_eq!(
                heap.bytes(ptr, size),
                vec![fill; size as usize],
                "step {step}"
            );
            assert_eq!(heap.release(ptr, step), Ok(()), "step {step}");
        } else {
            // Mostly small blocks, sometimes large ones.
            let size = if rng.next().is_multiple_of(50) {
                rng.next() % 20_000
            } else {
                rng.next() % 300
            };
            let ptr = heap.alloc(size, step).unwrap_or(0);
            assert!(
                ptr.is_multiple_of(16) && ptr >= HEAP_BASE,
                "step {step}: {ptr}"
            );
            for &(other, other_size, _, _) in &live {
                assert!(
                    ptr + size <= other || other + other_size <= ptr,
                    "step {step}: overlap"
                );
            }
            let fill = u8::try_from(step % 200).unwrap_or(0) + 1;
            heap.write(ptr, &vec![fill; size as usize]);
            live.push((ptr, size, fill, step));
        }
    }
    let count = u32::try_from(live.len()).unwrap_or(0);
    let bytes: u32 = live.iter().map(|&(_, size, _, _)| size).sum();
    assert_eq!(heap.live(), (count, bytes));
    let mut expected: Vec<(u32, u32)> = live.iter().map(|&(ptr, _, _, site)| (ptr, site)).collect();
    expected.sort_unstable();
    assert_eq!(heap.live_blocks(), expected);
}
