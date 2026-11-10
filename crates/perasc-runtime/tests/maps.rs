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
    live: TypedFunc<(), i32>,
    sip: TypedFunc<(i32, i32), i64>,
    seed: TypedFunc<(i64, i64), ()>,
    map_new: TypedFunc<i32, i32>,
    find_i: TypedFunc<(i32, i64), i32>,
    find_s: TypedFunc<(i32, i32), i32>,
    put_i: TypedFunc<(i32, i64, i64), ()>,
    put_s: TypedFunc<(i32, i32, i64), ()>,
    value: TypedFunc<(i32, i32), i64>,
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
        live: f!("live_count"),
        sip: f!("map_sip"),
        seed: f!("map_seed"),
        map_new: f!("map_new"),
        find_i: f!("map_find_i"),
        find_s: f!("map_find_s"),
        put_i: f!("map_put_i"),
        put_s: f!("map_put_s"),
        value: f!("map_value"),
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
        let mut keys = Vec::new();
        for e in 0..used {
            let at = entries + e * 24;
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
                .call(&mut rt.store, 0)
                .unwrap_or_else(|e| unreachable!("{e}"));
            let mut model = Model::default();
            // Few keys, many operations: replacing, removing and growing.
            let span = [4, 50, 3000][round % 3];
            for _ in 0..2500 {
                let key = i64::try_from(random.next(span)).unwrap_or(0) * 0x1_0000_0001 - 7;
                match random.next(10) {
                    0..=4 => {
                        let value = i64::try_from(random.next(1 << 40)).unwrap_or(0);
                        rt.put_i
                            .call(&mut rt.store, (m, key, value))
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
                                    .call(&mut rt.store, (m, e))
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
            .call(&mut rt.store, 1)
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
                    rt.put_s
                        .call(&mut rt.store, (m, key, value))
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
                                .call(&mut rt.store, (m, e))
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
