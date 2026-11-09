//! The TLSF allocator, run in an interpreter: blocks are reused, merged with
//! their free neighbours, and given back to the end of the heap; a long
//! random run checks it against a model of what should be alive, and checks
//! the layout of the heap after every step.

use perasc_runtime::{DEBUG_ALLOCATOR, Fault, HEAP_BASE, TLSF_ALLOCATOR};
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
    check_leaks: TypedFunc<(), ()>,
    end_fn: TypedFunc<(), i32>,
    buf_new: TypedFunc<(i32, i32, i32), i32>,
    buf_reserve: TypedFunc<(i32, i32, i32, i32), ()>,
    buf_release: TypedFunc<(i32, i32), ()>,
}

impl Heap {
    fn new() -> Result<Self, wasmi::Error> {
        Self::limited(usize::MAX)
    }

    /// An allocator whose memory cannot grow past `max_bytes`.
    fn limited(max_bytes: usize) -> Result<Self, wasmi::Error> {
        let engine = Engine::default();
        let module = Module::new(&engine, TLSF_ALLOCATOR)?;
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
        // Floats as text are the host's; these tests write none.
        linker.func_wrap(
            "peras",
            "float_text",
            |_: Caller<'_, Host>, _x: f64, _single: i32, _at: i32| -> Result<i32, wasmi::Error> {
                Err(wasmi::Error::new("float_text is not called here"))
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
            check_leaks: instance.get_typed_func(&store, "check_leaks")?,
            end_fn: instance.get_typed_func(&store, "heap_end")?,
            buf_new: instance.get_typed_func(&store, "buf_new")?,
            buf_reserve: instance.get_typed_func(&store, "buf_reserve")?,
            buf_release: instance.get_typed_func(&store, "buf_release")?,
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

    fn buf_new(&mut self, cap: u32, size: u32) -> Result<u32, Reported> {
        let result = self.buf_new.call(&mut self.store, (i(cap), i(size), 7));
        self.outcome(result).map(u)
    }

    fn buf_reserve(&mut self, h: u32, extra: u32, size: u32) -> Result<(), Reported> {
        let result = self
            .buf_reserve
            .call(&mut self.store, (i(h), i(extra), i(size), 8));
        self.outcome(result)
    }

    fn buf_release(&mut self, h: u32) -> Result<(), Reported> {
        let result = self.buf_release.call(&mut self.store, (i(h), 9));
        self.outcome(result)
    }

    /// A little-endian `u32` of memory.
    fn word(&self, ptr: u32) -> u32 {
        let b = self.bytes(ptr, 4);
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
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

    /// The first byte the heap's blocks do not use.
    fn end(&mut self) -> u32 {
        u(self.end_fn.call(&mut self.store, ()).unwrap_or(-1))
    }

    /// Walks every block of the heap, checking how they are laid out: they
    /// follow each other up to the end, each knows the one before it, and two
    /// free blocks are never neighbours. Returns the free blocks.
    fn walk(&mut self) -> Vec<(u32, u32)> {
        let end = self.end();
        let mut free = Vec::new();
        let mut block = HEAP_BASE;
        let mut previous = 0;
        let mut previous_free = false;
        while block < end {
            let word = self.word(block);
            let size = word & !15;
            let is_free = word & 1 == 1;
            assert!(
                size >= 32 && size.is_multiple_of(16),
                "block {block}: size {size}"
            );
            assert_eq!(self.word(block + 4), previous, "block {block}: previous");
            assert!(
                !(is_free && previous_free),
                "block {block}: two free neighbours"
            );
            if is_free {
                free.push((block, size));
            }
            previous = block;
            previous_free = is_free;
            block += size;
        }
        assert_eq!(block, end, "the blocks do not reach the end");
        assert!(!previous_free, "a free block ends the heap");
        free
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
fn it_is_valid_webassembly_with_the_exports_of_the_debug_allocator() {
    let exports = |bytes: &[u8]| -> Vec<String> {
        let mut names = Vec::new();
        for payload in wasmparser::Parser::new(0).parse_all(bytes) {
            if let Ok(wasmparser::Payload::ExportSection(reader)) = payload {
                for export in reader.into_iter().flatten() {
                    names.push(export.name.to_owned());
                }
            }
        }
        names.sort();
        names
    };
    assert!(wasmparser::validate(TLSF_ALLOCATOR).is_ok());
    let mut debug = exports(DEBUG_ALLOCATOR);
    debug.push("heap_end".to_owned());
    debug.sort();
    assert_eq!(exports(TLSF_ALLOCATOR), debug);
}

#[test]
fn blocks_are_aligned_and_counted() {
    let mut heap = heap();
    let a = heap.alloc(1, 1).unwrap_or(0);
    let b = heap.alloc(100, 2).unwrap_or(0);
    assert!(a.is_multiple_of(16) && a >= HEAP_BASE && b.is_multiple_of(16) && b > a);
    // Rounded up to 16 bytes each.
    assert_eq!(heap.live(), (2, 16 + 112));
    heap.walk();
}

#[test]
fn a_released_block_is_reused_and_the_heap_does_not_grow() {
    let mut heap = heap();
    let keep = heap.alloc(64, 1).unwrap_or(0);
    let first = heap.alloc(200, 2).unwrap_or(0);
    let guard = heap.alloc(16, 3).unwrap_or(0);
    let end = heap.end();
    assert_eq!(heap.release(first, 4), Ok(()));
    for step in 0..10_000 {
        let again = heap.alloc(200, 5).unwrap_or(0);
        assert_eq!(again, first, "step {step}");
        assert_eq!(heap.release(again, 6), Ok(()));
    }
    assert_eq!(heap.end(), end);
    assert!(keep < first && first < guard);
    heap.walk();
}

#[test]
fn neighbours_merge_so_that_a_larger_block_fits_where_two_were() {
    let mut heap = heap();
    let a = heap.alloc(1000, 1).unwrap_or(0);
    let b = heap.alloc(1000, 2).unwrap_or(0);
    let c = heap.alloc(1000, 3).unwrap_or(0);
    let end = heap.end();
    // Released in an order that merges each time, both ways.
    assert_eq!(heap.release(a, 4), Ok(()));
    assert_eq!(heap.walk(), vec![(a - 16, 1008 + 16)]);
    assert_eq!(heap.release(b, 5), Ok(()));
    assert_eq!(heap.walk(), vec![(a - 16, 2 * 1024)]);
    let big = heap.alloc(2000, 6).unwrap_or(0);
    assert_eq!(big, a);
    assert_eq!(heap.end(), end);
    // `big` took 2016 of the 2048 bytes: the 32 left stay free, and merge
    // with `c` when it is released, last, giving the end of the heap back.
    assert_eq!(heap.walk(), vec![(a - 16 + 2016, 32)]);
    assert_eq!(heap.release(c, 7), Ok(()));
    assert_eq!(heap.end(), a - 16 + 2016);
    assert_eq!(heap.release(big, 8), Ok(()));
    assert_eq!(heap.end(), HEAP_BASE);
    assert_eq!(heap.live(), (0, 0));
    assert_eq!(heap.check_leaks(), Ok(()));
}

#[test]
fn a_larger_free_block_is_split() {
    let mut heap = heap();
    let big = heap.alloc(4000, 1).unwrap_or(0);
    let _guard = heap.alloc(16, 2).unwrap_or(0);
    assert_eq!(heap.release(big, 3), Ok(()));
    let small = heap.alloc(100, 4).unwrap_or(0);
    assert_eq!(small, big);
    // The rest is free, right after it.
    assert_eq!(heap.walk(), vec![(big - 16 + 128, 4016 - 128)]);
}

#[test]
fn double_and_invalid_releases_are_caught() {
    let mut heap = heap();
    let a = heap.alloc(32, 1).unwrap_or(0);
    let _b = heap.alloc(32, 2).unwrap_or(0);
    assert_eq!(heap.release(a, 3), Ok(()));
    assert_eq!(
        heap.release(a, 4),
        Err(fault(Fault::DoubleRelease, a, 0, 4))
    );
    for bad in [0, 8, a + 8, 1 << 20] {
        assert_eq!(
            heap.release(bad, 5).map_err(|r| r.fault),
            Err(Some(Fault::InvalidRelease)),
            "{bad}"
        );
    }
}

#[test]
fn leaks_are_counted() {
    let mut heap = heap();
    let _a = heap.alloc(10, 1).unwrap_or(0);
    let _b = heap.alloc(10, 2).unwrap_or(0);
    assert_eq!(heap.check_leaks(), Err(fault(Fault::Leak, 0, 0, 2)));
}

#[test]
fn memory_grows_and_runs_out_cleanly() {
    let mut heap = heap();
    assert_eq!(
        heap.alloc(0x4000_0001, 9),
        Err(fault(Fault::TooLarge, 0x4000_0001, 9, 0))
    );
    let mut heap = Heap::limited(16 << 20).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(heap.alloc(6 << 20, 10).is_ok());
    assert!(heap.alloc(6 << 20, 11).is_ok());
    assert_eq!(
        heap.alloc(6 << 20, 12),
        Err(fault(Fault::OutOfMemory, 0, 12, 0))
    );
    assert_eq!(heap.live(), (2, 12 << 20));
    heap.walk();
}

#[test]
fn buffers_grow_and_release_the_buffers_they_own() {
    let mut heap = heap();
    let outer = heap.buf_new(0, 4 | 0x100).unwrap_or(0);
    for _ in 0..50 {
        let inner = heap.buf_new(3, 8).unwrap_or(0);
        assert_eq!(heap.buf_reserve(outer, 1, 4 | 0x100), Ok(()));
        let (len, data) = (heap.word(outer), heap.word(outer + 8));
        heap.write(data + 4 * len, &inner.to_le_bytes());
        heap.write(outer, &(len + 1).to_le_bytes());
    }
    assert_eq!(heap.buf_release(outer), Ok(()));
    assert_eq!(heap.live(), (0, 0));
    assert_eq!(heap.end(), HEAP_BASE);
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
/// never overlap and keep their bytes, the heap's layout holds after every
/// step, and once everything is released, the heap is empty again.
#[test]
fn a_random_run_matches_the_model_and_leaves_an_empty_heap() {
    let mut heap = heap();
    let mut rng = XorShift(0x2545_F491_4F6C_DD1D);
    let mut live: Vec<(u32, u32, u8)> = Vec::new();
    let mut peak = 0_u32;
    for step in 0..6_000_u32 {
        let release = !live.is_empty() && rng.next() % 100 < 48;
        if release {
            let index = (rng.next() as usize) % live.len();
            let (ptr, size, fill) = live.swap_remove(index);
            assert_eq!(
                heap.bytes(ptr, size),
                vec![fill; size as usize],
                "step {step}"
            );
            assert_eq!(heap.release(ptr, step), Ok(()), "step {step}");
        } else {
            let size = match rng.next() % 100 {
                0..=2 => rng.next() % 100_000,
                3..=20 => rng.next() % 4_000,
                _ => rng.next() % 300,
            };
            let ptr = heap.alloc(size, step).unwrap_or(0);
            assert!(
                ptr.is_multiple_of(16) && ptr >= HEAP_BASE,
                "step {step}: {ptr}"
            );
            for &(other, other_size, _) in &live {
                assert!(
                    ptr + size <= other || other + other_size <= ptr,
                    "step {step}: overlap"
                );
            }
            let fill = u8::try_from(step % 200).unwrap_or(0) + 1;
            heap.write(ptr, &vec![fill; size as usize]);
            live.push((ptr, size, fill));
        }
        if step % 50 == 0 {
            heap.walk();
        }
        let (count, _) = heap.live();
        assert_eq!(count, u32::try_from(live.len()).unwrap_or(0), "step {step}");
        peak = peak.max(heap.live().1);
    }
    heap.walk();
    // Fragmentation stays bounded: the heap is never much more than what was
    // alive at its peak.
    let used = heap.end() - HEAP_BASE;
    assert!(
        used <= 2 * peak + (1 << 17),
        "heap {used} for a peak of {peak}"
    );
    for (ptr, _, _) in live.drain(..) {
        assert_eq!(heap.release(ptr, 0), Ok(()));
    }
    assert_eq!(heap.live(), (0, 0));
    assert_eq!(heap.end(), HEAP_BASE);
    assert_eq!(heap.walk(), Vec::new());
}
