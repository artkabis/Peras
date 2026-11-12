//! Maps and sets in the runtime (decision 0005), run in wasmi on both
//! allocators: SipHash-1-3 against a reference written here, itself checked
//! against the `SipHash` paper's test vector; then thousands of random
//! operations, integer keys and string keys, each result and the insertion
//! order compared with a model in Rust, and no block left alive.

use std::collections::HashMap;

use perasc_runtime::{DEBUG_ALLOCATOR, TLSF_ALLOCATOR};
use wasmi::{Caller, Engine, Instance, Linker, Memory, Module, Store, TypedFunc};

/// SipHash-c-d, as the paper writes it, with the key `k0`, `k1`.
fn siphash(c: usize, d: usize, k0: u64, k1: u64, message: &[u8]) -> u64 {
    let mut v = [
        k0 ^ 0x736f_6d65_7073_6575,
        k1 ^ 0x646f_7261_6e64_6f6d,
        k0 ^ 0x6c79_6765_6e65_7261,
        k1 ^ 0x7465_6462_7974_6573,
    ];
    let round = |v: &mut [u64; 4]| {
        v[0] = v[0].wrapping_add(v[1]);
        v[1] = v[1].rotate_left(13) ^ v[0];
        v[0] = v[0].rotate_left(32);
        v[2] = v[2].wrapping_add(v[3]);
        v[3] = v[3].rotate_left(16) ^ v[2];
        v[0] = v[0].wrapping_add(v[3]);
        v[3] = v[3].rotate_left(21) ^ v[0];
        v[2] = v[2].wrapping_add(v[1]);
        v[1] = v[1].rotate_left(17) ^ v[2];
        v[2] = v[2].rotate_left(32);
    };
    let word = |v: &mut [u64; 4], m: u64| {
        v[3] ^= m;
        for _ in 0..c {
            round(v);
        }
        v[0] ^= m;
    };
    let (chunks, remainder) = message.as_chunks::<8>();
    for chunk in chunks {
        word(&mut v, u64::from_le_bytes(*chunk));
    }
    let mut last = (message.len() as u64) << 56;
    for (i, &b) in remainder.iter().enumerate() {
        last |= u64::from(b) << (8 * i);
    }
    word(&mut v, last);
    v[2] ^= 0xff;
    for _ in 0..d {
        round(&mut v);
    }
    v[0] ^ v[1] ^ v[2] ^ v[3]
}

#[test]
fn the_reference_gives_the_papers_vector() {
    // SipHash-2-4, key 00..0f, message 00..0e: the paper's appendix A.
    let message: Vec<u8> = (0..15).collect();
    assert_eq!(
        siphash(2, 4, 0x0706_0504_0302_0100, 0x0f0e_0d0c_0b0a_0908, &message),
        0xa129_ca61_49be_45e5
    );
}

struct Runtime {
    store: Store<()>,
    memory: Memory,
    new: TypedFunc<(i32, i32, i32), i32>,
    release: TypedFunc<(i32, i32), ()>,
    clone: TypedFunc<(i32, i32), i32>,
    live: TypedFunc<(), i32>,
    sip: TypedFunc<(i32, i32), i64>,
    seed: TypedFunc<(i64, i64), ()>,
    map_new: TypedFunc<(i32, i32), i32>,
    find_i: TypedFunc<(i32, i64), i32>,
    find_s: TypedFunc<(i32, i32), i32>,
    put_i: TypedFunc<(i32, i64), i32>,
    put_s: TypedFunc<(i32, i32), i32>,
    put_at_i: TypedFunc<(i32, i32, i64), i32>,
    put_at_s: TypedFunc<(i32, i32, i32), i32>,
    put_at_s_lent: TypedFunc<(i32, i32, i32), i32>,
    store_value: TypedFunc<(i32, i32, i32, i64), ()>,
    store_h: TypedFunc<(i32, i32, i32, i32), ()>,
    value: TypedFunc<(i32, i32, i32), i64>,
    value_h: TypedFunc<(i32, i32, i32), i32>,
    delete_i: TypedFunc<(i32, i64), i32>,
    delete_s: TypedFunc<(i32, i32), i32>,
    clear: TypedFunc<i32, ()>,
}

fn runtime(wasm: &[u8]) -> Runtime {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let mut linker = Linker::new(&engine);
    linker
        .func_wrap(
            "peras",
            "fault",
            |_: Caller<'_, ()>, kind: i32, _: i32, _: i32, _: i32| -> Result<(), wasmi::Error> {
                Err(wasmi::Error::new(format!("memory fault {kind}")))
            },
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    linker
        .func_wrap(
            "peras",
            "float_text",
            |_: Caller<'_, ()>, _: f64, _: i32, _: i32| 0_i32,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    linker
        .func_wrap(
            "peras",
            "float_parse",
            |_: Caller<'_, ()>, _: i32, _: i32| 0_f64,
        )
        .unwrap_or_else(|e| unreachable!("{e}"));
    let module = Module::new(&engine, wasm).unwrap_or_else(|e| unreachable!("{e}"));
    let instance: Instance = linker
        .instantiate_and_start(&mut store, &module)
        .unwrap_or_else(|e| unreachable!("{e}"));
    macro_rules! f {
        ($name:literal) => {
            instance
                .get_typed_func(&store, $name)
                .unwrap_or_else(|e| unreachable!("{}: {e}", $name))
        };
    }
    Runtime {
        memory: instance
            .get_memory(&store, "memory")
            .unwrap_or_else(|| unreachable!("memory")),
        new: f!("buf_new"),
        release: f!("buf_release"),
        clone: f!("buf_clone"),
        live: f!("live_count"),
        sip: f!("map_sip"),
        seed: f!("map_seed"),
        map_new: f!("map_new"),
        find_i: f!("map_find_i"),
        find_s: f!("map_find_s"),
        put_i: f!("map_put_i"),
        put_s: f!("map_put_s"),
        put_at_i: f!("map_put_at_i"),
        put_at_s: f!("map_put_at_s"),
        put_at_s_lent: f!("map_put_at_s_lent"),
        store_value: f!("map_store"),
        store_h: f!("map_store_h"),
        value: f!("map_value"),
        value_h: f!("map_value_h"),
        delete_i: f!("map_delete_i"),
        delete_s: f!("map_delete_s"),
        clear: f!("map_clear"),
        store,
    }
}

impl Runtime {
    /// A new string holding `bytes`.
    fn string(&mut self, bytes: &[u8]) -> i32 {
        let n = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
        let h = self
            .new
            .call(&mut self.store, (n, 1, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let data = self.read_i32(h + 8);
        self.memory
            .write(&mut self.store, usize::try_from(data).unwrap_or(0), bytes)
            .unwrap_or_else(|e| unreachable!("{e}"));
        self.memory
            .write(
                &mut self.store,
                usize::try_from(h).unwrap_or(0),
                &n.to_le_bytes(),
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
        h
    }

    fn read_i32(&self, at: i32) -> i32 {
        let mut bytes = [0; 4];
        self.memory
            .read(&self.store, usize::try_from(at).unwrap_or(0), &mut bytes)
            .unwrap_or_else(|e| unreachable!("{e}"));
        i32::from_le_bytes(bytes)
    }

    fn read_i64(&self, at: i32) -> i64 {
        let mut bytes = [0; 8];
        self.memory
            .read(&self.store, usize::try_from(at).unwrap_or(0), &mut bytes)
            .unwrap_or_else(|e| unreachable!("{e}"));
        i64::from_le_bytes(bytes)
    }

    /// The live keys of map `m`, in the order of its entries, and its size.
    fn keys(&self, m: i32) -> (Vec<i64>, i32) {
        let used = self.read_i32(m + 4);
        let entries = self.read_i32(m + 8);
        let stride = 16 + 8 * (self.read_i32(m + 28) & 0xff);
        let mut keys = Vec::new();
        for e in 0..used {
            let at = entries + e * stride;
            if self.read_i32(at + 4) == 1 {
                keys.push(self.read_i64(at + 8));
            }
        }
        (keys, self.read_i32(m))
    }

    /// The bytes of string `h`.
    fn text(&self, h: i32) -> Vec<u8> {
        let len = usize::try_from(self.read_i32(h)).unwrap_or(0);
        let mut bytes = vec![0; len];
        self.memory
            .read(
                &self.store,
                usize::try_from(self.read_i32(h + 8)).unwrap_or(0),
                &mut bytes,
            )
            .unwrap_or_else(|e| unreachable!("{e}"));
        bytes
    }

    fn live_count(&mut self) -> i32 {
        self.live
            .call(&mut self.store, ())
            .unwrap_or_else(|e| unreachable!("{e}"))
    }
}

#[test]
fn the_runtime_hashes_as_siphash_1_3() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        for (k0, k1) in [
            (0x0706_0504_0302_0100_u64, 0x0f0e_0d0c_0b0a_0908_u64),
            (0xdead_beef, 42),
        ] {
            rt.seed
                .call(&mut rt.store, (k0.cast_signed(), k1.cast_signed()))
                .unwrap_or_else(|e| unreachable!("{e}"));
            for n in 0..64_u8 {
                let message: Vec<u8> = (0..n).map(|i| i.wrapping_mul(37).wrapping_add(n)).collect();
                let h = rt.string(&message);
                let at = rt.read_i32(h + 8);
                let got = rt
                    .sip
                    .call(&mut rt.store, (at, i32::from(n)))
                    .unwrap_or_else(|e| unreachable!("{e}"));
                assert_eq!(
                    got.cast_unsigned(),
                    siphash(1, 3, k0, k1, &message),
                    "{n} bytes"
                );
                rt.release
                    .call(&mut rt.store, (h, 0))
                    .unwrap_or_else(|e| unreachable!("{e}"));
            }
        }
        assert_eq!(rt.live_count(), 0);
    }
}

/// A deterministic stream of numbers.
struct Random(u64);

impl Random {
    fn next(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % below
    }
}

/// The model: entries in insertion order, removed ones taken out.
#[derive(Default)]
struct Model {
    order: Vec<i64>,
    values: HashMap<i64, i64>,
}

impl Model {
    fn put(&mut self, key: i64, value: i64) {
        if self.values.insert(key, value).is_none() {
            self.order.push(key);
        }
    }
    fn delete(&mut self, key: i64) -> bool {
        let found = self.values.remove(&key).is_some();
        self.order.retain(|&k| k != key);
        found
    }
}

#[test]
fn integer_keys_behave_as_the_model_in_insertion_order() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x2545_f491_4f6c_dd1d);
        for round in 0..9 {
            let m = rt
                .map_new
                .call(&mut rt.store, (0, 1))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let mut model = Model::default();
            // Few keys, many operations: replacing, removing and growing.
            let span = [4, 50, 3000][round % 3];
            for _ in 0..2500 {
                let key = i64::try_from(random.next(span)).unwrap_or(0) * 0x1_0000_0001 - 7;
                match random.next(10) {
                    0..=4 => {
                        let value = i64::try_from(random.next(1 << 40)).unwrap_or(0);
                        let e = rt
                            .put_i
                            .call(&mut rt.store, (m, key))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        rt.store_value
                            .call(&mut rt.store, (m, e, 0, value))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        model.put(key, value);
                    }
                    5..=6 => {
                        let removed = rt
                            .delete_i
                            .call(&mut rt.store, (m, key))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        assert_eq!(removed == 1, model.delete(key));
                    }
                    7 if random.next(200) == 0 => {
                        rt.clear
                            .call(&mut rt.store, m)
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        model = Model::default();
                    }
                    _ => {
                        let e = rt
                            .find_i
                            .call(&mut rt.store, (m, key))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        match model.values.get(&key) {
                            Some(&want) => {
                                assert!(e >= 0, "{key} not found");
                                let got = rt
                                    .value
                                    .call(&mut rt.store, (m, e, 0))
                                    .unwrap_or_else(|e| unreachable!("{e}"));
                                assert_eq!(got, want);
                            }
                            None => assert_eq!(e, -1, "{key} found"),
                        }
                    }
                }
            }
            let (keys, size) = rt.keys(m);
            assert_eq!(keys, model.order, "insertion order");
            assert_eq!(usize::try_from(size).unwrap_or(0), model.order.len());
            rt.release
                .call(&mut rt.store, (m, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
        }
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

#[test]
fn string_keys_are_owned_released_and_found_by_their_text() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        let m = rt
            .map_new
            .call(&mut rt.store, (1, 1))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut model: Vec<(Vec<u8>, i64)> = Vec::new();
        for _ in 0..3000 {
            let n = random.next(300);
            let text =
                format!("w{n}{}", "é".repeat(usize::try_from(n % 7).unwrap_or(0))).into_bytes();
            match random.next(10) {
                0..=4 => {
                    let value = i64::try_from(random.next(1000)).unwrap_or(0);
                    // The key is taken by the map: a fresh string each time.
                    let key = rt.string(&text);
                    let e = rt
                        .put_s
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    rt.store_value
                        .call(&mut rt.store, (m, e, 0, value))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    match model.iter_mut().find(|(k, _)| *k == text) {
                        Some(entry) => entry.1 = value,
                        None => model.push((text, value)),
                    }
                }
                5..=6 => {
                    let key = rt.string(&text);
                    let removed = rt
                        .delete_s
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    rt.release
                        .call(&mut rt.store, (key, 0))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    let before = model.len();
                    model.retain(|(k, _)| *k != text);
                    assert_eq!(removed == 1, model.len() < before);
                }
                _ => {
                    let key = rt.string(&text);
                    let e = rt
                        .find_s
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    rt.release
                        .call(&mut rt.store, (key, 0))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    match model.iter().find(|(k, _)| *k == text) {
                        Some(&(_, want)) => {
                            let got = rt
                                .value
                                .call(&mut rt.store, (m, e, 0))
                                .unwrap_or_else(|e| unreachable!("{e}"));
                            assert_eq!(got, want);
                        }
                        None => assert_eq!(e, -1),
                    }
                }
            }
        }
        let (keys, size) = rt.keys(m);
        let texts: Vec<Vec<u8>> = keys
            .iter()
            .map(|&h| rt.text(i32::try_from(h).unwrap_or(0)))
            .collect();
        let want: Vec<Vec<u8>> = model.iter().map(|(k, _)| k.clone()).collect();
        assert_eq!(texts, want, "insertion order");
        assert_eq!(usize::try_from(size).unwrap_or(0), model.len());
        // Releasing the map releases every key it owns.
        rt.release
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// Values that own buffers: a string in slot 0, an integer in slot 1. The
/// map releases the string a value held when the key is set again, deleted,
/// cleared, or the map released.
#[test]
fn values_that_own_buffers_are_released_with_their_entries() {
    // Two slots, the first owning a buffer.
    const SHAPE: i32 = 2 | (1 << 8);
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x0123_4567_89ab_cdef);
        let m = rt
            .map_new
            .call(&mut rt.store, (0, SHAPE))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut model: Vec<(i64, Vec<u8>, i64)> = Vec::new();
        for step in 0..3000_i64 {
            let key = i64::try_from(random.next(200)).unwrap_or(0);
            match random.next(10) {
                0..=4 => {
                    let text = format!("v{step}é").into_bytes();
                    let h = rt.string(&text);
                    let e = rt
                        .put_i
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    rt.store_h
                        .call(&mut rt.store, (m, e, 0, h))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    rt.store_value
                        .call(&mut rt.store, (m, e, 1, step))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    match model.iter_mut().find(|(k, _, _)| *k == key) {
                        Some(entry) => {
                            entry.1 = text;
                            entry.2 = step;
                        }
                        None => model.push((key, text, step)),
                    }
                }
                5..=6 => {
                    let removed = rt
                        .delete_i
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    let before = model.len();
                    model.retain(|(k, _, _)| *k != key);
                    assert_eq!(removed == 1, model.len() < before);
                }
                7 if random.next(100) == 0 => {
                    rt.clear
                        .call(&mut rt.store, m)
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    model.clear();
                }
                _ => {
                    let e = rt
                        .find_i
                        .call(&mut rt.store, (m, key))
                        .unwrap_or_else(|e| unreachable!("{e}"));
                    match model.iter().find(|(k, _, _)| *k == key) {
                        Some((_, text, number)) => {
                            let h = rt
                                .value_h
                                .call(&mut rt.store, (m, e, 0))
                                .unwrap_or_else(|e| unreachable!("{e}"));
                            assert_eq!(rt.text(h), *text);
                            let got = rt
                                .value
                                .call(&mut rt.store, (m, e, 1))
                                .unwrap_or_else(|e| unreachable!("{e}"));
                            assert_eq!(got, *number);
                        }
                        None => assert_eq!(e, -1),
                    }
                }
            }
            // The map's blocks, and one string per live entry, nothing more.
            let (keys, _) = rt.keys(m);
            assert_eq!(keys, model.iter().map(|(k, _, _)| *k).collect::<Vec<_>>());
        }
        rt.release
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// A copy of a map owns copies of its string keys and of the buffers of its
/// values: changing or releasing the original leaves it as it was.
#[test]
fn a_copied_map_owns_its_keys_and_values() {
    const SHAPE: i32 = 2 | (1 << 8);
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let m = rt
            .map_new
            .call(&mut rt.store, (1, SHAPE))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut model: Vec<(Vec<u8>, Vec<u8>, i64)> = Vec::new();
        for i in 0..300_i64 {
            let key = format!("k{}", i % 120).into_bytes();
            let text = format!("v{i}é").into_bytes();
            let (k, h) = (rt.string(&key), rt.string(&text));
            let e = rt
                .put_s
                .call(&mut rt.store, (m, k))
                .unwrap_or_else(|e| unreachable!("{e}"));
            rt.store_h
                .call(&mut rt.store, (m, e, 0, h))
                .unwrap_or_else(|e| unreachable!("{e}"));
            rt.store_value
                .call(&mut rt.store, (m, e, 1, i))
                .unwrap_or_else(|e| unreachable!("{e}"));
            match model.iter_mut().find(|(k, _, _)| *k == key) {
                Some(entry) => {
                    entry.1 = text;
                    entry.2 = i;
                }
                None => model.push((key, text, i)),
            }
            if i % 7 == 3 {
                let gone = format!("k{}", (i * 13) % 120).into_bytes();
                let k = rt.string(&gone);
                rt.delete_s
                    .call(&mut rt.store, (m, k))
                    .unwrap_or_else(|e| unreachable!("{e}"));
                rt.release
                    .call(&mut rt.store, (k, 0))
                    .unwrap_or_else(|e| unreachable!("{e}"));
                model.retain(|(k, _, _)| *k != gone);
            }
        }
        let copy = rt
            .clone
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        rt.clear
            .call(&mut rt.store, m)
            .unwrap_or_else(|e| unreachable!("{e}"));
        rt.release
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let (keys, size) = rt.keys(copy);
        assert_eq!(usize::try_from(size).unwrap_or(0), model.len());
        for (h, (key, text, number)) in keys.iter().zip(&model) {
            assert_eq!(rt.text(i32::try_from(*h).unwrap_or(0)), *key);
            let k = rt.string(key);
            let e = rt
                .find_s
                .call(&mut rt.store, (copy, k))
                .unwrap_or_else(|e| unreachable!("{e}"));
            rt.release
                .call(&mut rt.store, (k, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let held = rt
                .value_h
                .call(&mut rt.store, (copy, e, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(rt.text(held), *text);
            let number_read = rt
                .value
                .call(&mut rt.store, (copy, e, 1))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(number_read, *number);
        }
        rt.release
            .call(&mut rt.store, (copy, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// The entry a lookup gave, most of the time, or -1, or one that does not
/// hold the key, which the compiler never gives.
fn entry_given(random: &mut Random, found: i32, past: u64) -> i32 {
    match random.next(4) {
        0 => -1,
        1 => i32::try_from(random.next(past)).unwrap_or(0),
        _ => found,
    }
}

/// `map_put_at_i`, given the entry a lookup found, or -1, or one that does
/// not hold the key: every way, the map ends as `put` would leave it, and no
/// block is left alive (decision 0017).
#[test]
fn a_set_given_its_entry_ends_as_a_set() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        // Integer keys, one value each.
        let m = rt
            .map_new
            .call(&mut rt.store, (0, 1))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut model = Model::default();
        for _ in 0..3000 {
            let key = i64::try_from(random.next(40)).unwrap_or(0);
            let value = i64::try_from(random.next(1000)).unwrap_or(0);
            let found = rt
                .find_i
                .call(&mut rt.store, (m, key))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let entry = entry_given(&mut random, found, 60);
            if random.next(9) == 0 {
                let removed = rt
                    .delete_i
                    .call(&mut rt.store, (m, key))
                    .unwrap_or_else(|e| unreachable!("{e}"));
                assert_eq!(removed == 1, model.delete(key));
                continue;
            }
            let e = rt
                .put_at_i
                .call(&mut rt.store, (m, entry, key))
                .unwrap_or_else(|e| unreachable!("{e}"));
            rt.store_value
                .call(&mut rt.store, (m, e, 0, value))
                .unwrap_or_else(|e| unreachable!("{e}"));
            model.put(key, value);
            let again = rt
                .find_i
                .call(&mut rt.store, (m, key))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(again, e, "the entry given back is the key's");
        }
        let (keys, size) = rt.keys(m);
        assert_eq!(keys, model.order, "insertion order");
        assert_eq!(usize::try_from(size).unwrap_or(0), model.order.len());
        rt.release
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// `map_put_at_s` and `map_put_at_s_lent`, string keys and string values,
/// the same way.
#[test]
fn a_set_of_a_string_given_its_entry_ends_as_a_set() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x9e37_79b9_7f4a_7c15);
        let m = rt
            .map_new
            .call(&mut rt.store, (1, 1 | (1 << 8)))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut texts: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        for round in 0..2000 {
            let word = format!("k{}", random.next(30)).into_bytes();
            let key = rt.string(&word);
            let found = rt
                .find_s
                .call(&mut rt.store, (m, key))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let entry = entry_given(&mut random, found, 40);
            let lent = random.next(2) == 0;
            let e = if lent {
                rt.put_at_s_lent
                    .call(&mut rt.store, (m, entry, key))
                    .unwrap_or_else(|e| unreachable!("{e}"))
            } else {
                rt.put_at_s
                    .call(&mut rt.store, (m, entry, key))
                    .unwrap_or_else(|e| unreachable!("{e}"))
            };
            if lent {
                rt.release
                    .call(&mut rt.store, (key, 0))
                    .unwrap_or_else(|e| unreachable!("{e}"));
            }
            let value = format!("v{round}").into_bytes();
            let h = rt.string(&value);
            rt.store_h
                .call(&mut rt.store, (m, e, 0, h))
                .unwrap_or_else(|e| unreachable!("{e}"));
            texts.insert(word, value);
        }
        for (word, want) in &texts {
            let key = rt.string(word);
            let e = rt
                .find_s
                .call(&mut rt.store, (m, key))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert!(e >= 0, "a key lost");
            let got = rt
                .value_h
                .call(&mut rt.store, (m, e, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(&rt.text(got), want);
            rt.release
                .call(&mut rt.store, (key, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
        }
        rt.release
            .call(&mut rt.store, (m, 0))
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// A map whose keys are integers of 32 bits keeps them in its index
/// (decision 0018): the same operations as the model, keys negative, zero
/// and at the bounds of an `i32` among them, many deletes, growth and a
/// clear; then a copy of the map, which finds what the map finds.
#[test]
fn keys_of_32_bits_kept_in_the_index_behave_as_the_model() {
    for wasm in [DEBUG_ALLOCATOR, TLSF_ALLOCATOR] {
        let mut rt = runtime(wasm);
        let mut random = Random(0x5851_f42d_4c95_7f2d);
        for round in 0..9 {
            let m = rt
                .map_new
                .call(&mut rt.store, (2, 1))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let mut model = Model::default();
            let span = [4, 50, 3000][round % 3];
            for _ in 0..2500 {
                // Keys of 32 bits, widened as the compiler widens an `i32`.
                let key = match random.next(20) {
                    0 => i64::from(i32::MIN),
                    1 => i64::from(i32::MAX),
                    _ => (i64::try_from(random.next(span)).unwrap_or(0) - 17) * 65_537,
                };
                match random.next(10) {
                    0..=4 => {
                        let value = i64::try_from(random.next(1 << 40)).unwrap_or(0);
                        let e = rt
                            .put_i
                            .call(&mut rt.store, (m, key))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        rt.store_value
                            .call(&mut rt.store, (m, e, 0, value))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        model.put(key, value);
                    }
                    5..=6 => {
                        let removed = rt
                            .delete_i
                            .call(&mut rt.store, (m, key))
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        assert_eq!(removed == 1, model.delete(key));
                    }
                    7 if random.next(200) == 0 => {
                        rt.clear
                            .call(&mut rt.store, m)
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        model = Model::default();
                    }
                    _ => check_found(&mut rt, m, key, &model),
                }
            }
            let (keys, size) = rt.keys(m);
            assert_eq!(keys, model.order, "insertion order");
            assert_eq!(usize::try_from(size).unwrap_or(0), model.order.len());
            let copy = rt
                .clone
                .call(&mut rt.store, (m, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            for &key in &model.order {
                check_found(&mut rt, copy, key, &model);
            }
            check_found(&mut rt, copy, i64::from(i32::MIN) + 3, &model);
            for h in [m, copy] {
                rt.release
                    .call(&mut rt.store, (h, 0))
                    .unwrap_or_else(|e| unreachable!("{e}"));
            }
        }
        assert_eq!(rt.live_count(), 0, "blocks left alive");
    }
}

/// `key` is found in `m` with the model's value, or not found when the model
/// does not hold it.
fn check_found(rt: &mut Runtime, m: i32, key: i64, model: &Model) {
    let e = rt
        .find_i
        .call(&mut rt.store, (m, key))
        .unwrap_or_else(|e| unreachable!("{e}"));
    match model.values.get(&key) {
        Some(&want) => {
            assert!(e >= 0, "{key} not found");
            let got = rt
                .value
                .call(&mut rt.store, (m, e, 0))
                .unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(got, want);
        }
        None => assert_eq!(e, -1, "{key} found"),
    }
}
