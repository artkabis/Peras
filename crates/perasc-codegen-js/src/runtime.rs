//! The runtime of managed modules: small functions the generated code
//! calls, each written into a module only when it uses it.

use std::collections::BTreeSet;

/// Each helper: its name, the helpers it calls, and its source.
const HELPERS: &[(&str, &[&str], &str)] = &[
    // An index out of bounds throws the `RangeError` of owned modules,
    // its text given by the build (`BOUNDS`); a division traps as theirs do.
    (
        "$bounds",
        &[],
        "const $bounds = (index, length) => { throw new RangeError(BOUNDS.replace(\"{index}\", String(index)).replace(\"{length}\", String(length))); };",
    ),
    (
        "$trap",
        &[],
        "const $trap = (what) => { throw new WebAssembly.RuntimeError(what); };",
    ),
    (
        "$at",
        &["$bounds"],
        "const $at = (xs, i) => (i >>> 0 < xs.length ? xs[i] : $bounds(i, xs.length));",
    ),
    (
        "$put",
        &["$bounds"],
        "const $put = (xs, i, v) => { if (i >>> 0 >= xs.length) $bounds(i, xs.length); xs[i] = v; };",
    ),
    (
        "$div",
        &["$trap"],
        "const $div = (a, b, signed) => { if (b === 0) $trap(\"divide by zero\"); if (signed && b === -1 && a === -2147483648) $trap(\"divide result unrepresentable\"); return Math.trunc(a / b); };",
    ),
    (
        "$rem",
        &["$trap"],
        "const $rem = (a, b) => { if (b === 0) $trap(\"remainder by zero\"); return a % b; };",
    ),
    (
        "$div64",
        &["$trap"],
        "const $div64 = (a, b, signed) => { if (b === 0n) $trap(\"divide by zero\"); if (signed && b === -1n && a === -9223372036854775808n) $trap(\"divide result unrepresentable\"); return a / b; };",
    ),
    (
        "$rem64",
        &["$trap"],
        "const $rem64 = (a, b) => { if (b === 0n) $trap(\"remainder by zero\"); return a % b; };",
    ),
    ("$min", &[], "const $min = (a, b) => (a < b ? a : b);"),
    ("$max", &[], "const $max = (a, b) => (a > b ? a : b);"),
    (
        "$checked",
        &[],
        "const $checked = (r, lo, hi) => (r >= lo && r <= hi ? r : undefined);",
    ),
    (
        "$checked64",
        &[],
        "const $checked64 = (r, signed) => (r === (signed ? BigInt.asIntN(64, r) : BigInt.asUintN(64, r)) ? r : undefined);",
    ),
    (
        "$saturate",
        &[],
        "const $saturate = (x, lo, hi) => (x !== x ? 0 : x <= lo ? lo : x >= hi ? hi : Math.trunc(x) + 0);",
    ),
    (
        "$saturate64",
        &[],
        "const $saturate64 = (x, signed) => { const lo = signed ? -9223372036854775808n : 0n; const hi = signed ? 9223372036854775807n : 18446744073709551615n; if (x !== x) return 0n; if (x <= Number(lo)) return lo; if (x >= Number(hi)) return hi; return BigInt(Math.trunc(x)); };",
    ),
    (
        "$f32text",
        &[],
        "const $f32text = (x) => { if (Number.isFinite(x)) for (let p = 1; p < 10; p++) { const t = x.toPrecision(p); if (Math.fround(Number(t)) === x) return String(Number(t)); } return String(x); };",
    ),
    (
        "$sort",
        &[],
        "const $sort = (xs, Typed) => { const t = Typed.from(xs).sort(); for (let i = 0; i < t.length; i++) xs[i] = t[i]; };",
    ),
    (
        "$sortBy",
        &["$cmpKey", "$radixBy"],
        "const $sortBy = (xs, f, desc, kind) => { const n = xs.length; if (n < 2) return; const s = desc ? -1 : 1; if (kind === 0 && n >= 256) { const ix = new Uint32Array(n); for (let i = 0; i < n; i++) ix[i] = i; $radixBy(xs, f, desc, ix); const c = xs.slice(); for (let i = 0; i < n; i++) xs[i] = c[ix[i]]; return; } if (kind === 2) { let wide = false; for (let i = 0; i < n && !wide; i++) { const k = f(xs[i]); for (let j = 0; j < k.length; j++) if (k.charCodeAt(j) >= 0xd800) { wide = true; break; } } if (!wide) { xs.sort((a, b) => { const x = f(a), y = f(b); return s * (x < y ? -1 : x > y ? 1 : 0); }); return; } } const d = new Array(n); for (let i = 0; i < n; i++) d[i] = { k: f(xs[i]), v: xs[i] }; d.sort((p, q) => s * $cmpKey(p.k, q.k, kind)); for (let i = 0; i < n; i++) xs[i] = d[i].v; };",
    ),
    (
        "$cmpKey",
        &[],
        "const $cmpKey = (a, b, kind) => { if (kind === 2) { if (a === b) return 0; const n = Math.min(a.length, b.length); for (let i = 0; i < n; i++) { let x = a.charCodeAt(i), y = b.charCodeAt(i); if (x !== y) { if (x >= 0xd800 && y >= 0xd800) { x = x >= 0xe000 ? x - 0x800 : x + 0x2000; y = y >= 0xe000 ? y - 0x800 : y + 0x2000; } return x - y; } } return a.length - b.length; } if (a < b) return -1; if (a > b) return 1; if (a === b) return a === 0 && kind === 0 ? (Object.is(a, -0) ? 0 : 1) - (Object.is(b, -0) ? 0 : 1) : 0; return (a !== a ? 1 : 0) - (b !== b ? 1 : 0); };",
    ),
    (
        "$radixBy",
        &[],
        "const $radixBy = (xs, f, desc, out) => { let ix = out; const n = xs.length, kf = new Float64Array(n), u = new Uint32Array(kf.buffer); for (let i = 0; i < n; i++) kf[i] = f(xs[i]); let hi = new Uint32Array(n), lo = new Uint32Array(n), ix2 = new Uint32Array(n); for (let i = 0; i < n; i++) { let h = u[2 * i + 1], l = u[2 * i]; if (kf[i] !== kf[i]) { h = 0xffffffff; l = 0xffffffff; } else if (h & 0x80000000) { h = ~h; l = ~l; } else h |= 0x80000000; if (desc) { h = ~h; l = ~l; } hi[i] = h >>> 0; lo[i] = l >>> 0; } let hi2 = new Uint32Array(n), lo2 = new Uint32Array(n); const cnt = new Uint32Array(256); for (let p = 0; p < 8; p++) { const w = p < 4 ? lo : hi, sh = (p & 3) * 8; cnt.fill(0); for (let i = 0; i < n; i++) cnt[(w[i] >>> sh) & 255]++; if (cnt[(w[0] >>> sh) & 255] === n) continue; let s = 0; for (let b = 0; b < 256; b++) { const c = cnt[b]; cnt[b] = s; s += c; } for (let i = 0; i < n; i++) { const d = cnt[(w[i] >>> sh) & 255]++; hi2[d] = hi[i]; lo2[d] = lo[i]; ix2[d] = ix[i]; } let t = hi; hi = hi2; hi2 = t; t = lo; lo = lo2; lo2 = t; t = ix; ix = ix2; ix2 = t; } if (ix !== out) out.set(ix); };",
    ),
    (
        "$eq",
        &[],
        "const $eq = (a, b) => { if (a === b) return true; if (typeof a !== \"object\" || typeof b !== \"object\" || a === null || b === null) return false; if (a instanceof Map || a instanceof Set) { if (!(b instanceof a.constructor) || a.size !== b.size) return false; for (const [k, v] of a.entries()) if (!b.has(k) || !$eq(v, a instanceof Map ? b.get(k) : k)) return false; return true; } const ka = Object.keys(a); if (ka.length !== Object.keys(b).length) return false; for (const k of ka) if (!$eq(a[k], b[k])) return false; return true; };",
    ),
    (
        "$trim",
        &[],
        "const $trim = (s) => s.replace(/^[ \\t\\n\\r]+|[ \\t\\n\\r]+$/g, \"\");",
    ),
    (
        "$upper",
        &[],
        "const $upper = (s) => s.replace(/[a-z]+/g, (m) => m.toUpperCase());",
    ),
    (
        "$lower",
        &[],
        "const $lower = (s) => s.replace(/[A-Z]+/g, (m) => m.toLowerCase());",
    ),
    (
        "$split",
        &[],
        "const $split = (s, sep) => (sep === \"\" ? Array.from(s) : s.split(sep));",
    ),
    (
        "$repeat",
        &[],
        "const $repeat = (s, n) => (n > 0 ? s.repeat(n) : \"\");",
    ),
    (
        "$parseI32",
        &[],
        "const $parseI32 = (s) => { if (!/^[+-]?[0-9]+$/.test(s)) return undefined; const n = Number(s); return n >= -2147483648 && n <= 2147483647 ? n : undefined; };",
    ),
    // Resources (decision 0009). In debug, the leak detector reports one
    // collected without being released, with where it was made.
    (
        "$leaks",
        &[],
        "const $leaks = DEBUG ? new FinalizationRegistry((where) => console.error(`peras: a resource made at ${where} was collected without being released: it went on listening or ticking until then`)) : undefined;",
    ),
    (
        "$Resource",
        &["$leaks"],
        "class $Resource { #release; constructor(release, where) { this.#release = release; $leaks?.register(this, where, this); } release() { const release = this.#release; if (release === undefined) return; this.#release = undefined; $leaks?.unregister(this); release(); } [Symbol.dispose ?? Symbol.for(\"Symbol.dispose\")]() { this.release(); } }",
    ),
    ("$release", &[], "const $release = (r) => r.release();"),
    (
        "$on",
        &["$Resource"],
        "const $on = (target, type, handler, where) => { const c = new AbortController(); target.addEventListener(type, handler, { signal: c.signal }); return new $Resource(() => c.abort(), where); };",
    ),
    (
        "$timer",
        &["$Resource"],
        "const $timer = (repeat, ms, f, where) => { const id = repeat ? setInterval(f, ms) : setTimeout(f, ms); return new $Resource(() => (repeat ? clearInterval(id) : clearTimeout(id)), where); };",
    ),
    (
        "$group",
        &["$Resource"],
        "const $group = (where) => { const held = []; let open = true; const group = new $Resource(() => { open = false; while (held.length > 0) held.pop().release(); }, where); group.add = (r) => { if (open) held.push(r); else r.release(); }; group.slot = () => { let current; const slot = { set(r) { current?.release(); if (open) current = r; else { current = undefined; r.release(); } }, clear() { const r = current; current = undefined; r?.release(); } }; held.push({ release: () => slot.clear() }); return slot; }; return group; };",
    ),
    (
        "$sleep",
        &[],
        "const $sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));",
    ),
    (
        "$as",
        &[],
        "const $as = (x, name) => (typeof globalThis[name] === \"function\" && x instanceof globalThis[name] ? x : undefined);",
    ),
    // Values from JavaScript (decision 0010): each checked against the type
    // declared, and rebuilt, so that JavaScript keeps no hold on them.
    (
        "$describe",
        &[],
        "const $describe = (v) => { switch (typeof v) { case \"undefined\": return \"undefined\"; case \"string\": return `the string ${JSON.stringify(v.slice(0, 40))}`; case \"number\": case \"boolean\": return `the ${typeof v} ${String(v)}`; case \"bigint\": return `the bigint ${String(v)}n`; case \"function\": return \"a function\"; default: return v === null ? \"null\" : Array.isArray(v) ? \"an array\" : `an object (${v.constructor?.name ?? \"Object\"})`; } };",
    ),
    (
        "$shape",
        &[],
        "const $ranges = { i8: [-128, 127], i16: [-32768, 32767], i32: [-2147483648, 2147483647], u8: [0, 255], u16: [0, 65535], u32: [0, 4294967295] };\nconst $shape = (s) => { if (typeof s === \"string\") { if (s in $ranges) { const [lo, hi] = $ranges[s]; return (v, bad) => (Number.isInteger(v) && v >= lo && v <= hi ? v + 0 : bad(v)); } switch (s) { case \"f64\": return (v, bad) => (typeof v === \"number\" ? v : bad(v)); case \"f32\": return (v, bad) => (typeof v === \"number\" && (v !== v || Math.fround(v) === v) ? v : bad(v)); case \"i64\": return (v, bad) => (typeof v === \"bigint\" && BigInt.asIntN(64, v) === v ? v : bad(v)); case \"u64\": return (v, bad) => (typeof v === \"bigint\" && BigInt.asUintN(64, v) === v ? v : bad(v)); case \"bool\": return (v, bad) => (typeof v === \"boolean\" ? v : bad(v)); case \"string\": return (v, bad) => (typeof v === \"string\" ? v : bad(v)); case \"void\": return () => undefined; case \"opaque\": return (v, bad) => (v === null || v === undefined ? bad(v) : v); default: return (v) => v; } } switch (s[0]) { case \"array\": { const e = $shape(s[1]); return (v, bad) => { if (!Array.isArray(v)) return bad(v); const n = v.length; const out = new Array(n); for (let i = 0; i < n; i++) out[i] = e(v[i], bad); return out; }; } case \"option\": { const e = $shape(s[1]); return (v, bad) => (v === null || v === undefined ? undefined : e(v, bad)); } case \"set\": { const e = $shape(s[1]); return (v, bad) => { if (!(v instanceof Set)) return bad(v); const out = new Set(); for (const x of v) out.add(e(x, bad)); return out; }; } case \"map\": { const k = $shape(s[1]); const e = $shape(s[2]); return (v, bad) => { if (!(v instanceof Map)) return bad(v); const out = new Map(); for (const [a, b] of v) out.set(k(a, bad), e(b, bad)); return out; }; } case \"tuple\": { const es = s[1].map($shape); return (v, bad) => (Array.isArray(v) && v.length === es.length ? es.map((e, i) => e(v[i], bad)) : bad(v)); } case \"struct\": { const ks = Object.keys(s[1]); const es = ks.map((k) => $shape(s[1][k])); return (v, bad) => { if (typeof v !== \"object\" || v === null) return bad(v); const o = {}; for (let i = 0; i < ks.length; i++) o[ks[i]] = es[i](v[ks[i]], bad); return o; }; } case \"enum\": { const vs = {}; for (const k of Object.keys(s[1])) vs[k] = s[1][k].map($shape); return (v, bad) => (typeof v === \"object\" && v !== null && typeof v.kind === \"string\" && Object.hasOwn(vs, v.kind) && Array.isArray(v.values) && v.values.length === vs[v.kind].length ? { kind: v.kind, values: vs[v.kind].map((e, i) => e(v.values[i], bad)) } : bad(v)); } case \"host\": return (v, bad) => (typeof globalThis[s[1]] === \"function\" && v instanceof globalThis[s[1]] ? v : bad(v)); default: return (v) => v; } };",
    ),
    (
        "$valid",
        &["$shape", "$describe"],
        "const $valid = (s, at, decl) => { const check = $shape(s); const bad = (v) => { throw new TypeError(`peras: ${at} ${$describe(v)} where its declaration says ${decl}`); }; return (v) => check(v, bad); };",
    ),
    (
        "$callback",
        &[],
        "const $callback = (f, checks) => (...xs) => f(...checks.map((c, i) => c(xs[i])));",
    ),
    (
        "$parseI64",
        &[],
        "const $parseI64 = (s) => { if (!/^[+-]?[0-9]+$/.test(s)) return undefined; const n = BigInt(s); return n === BigInt.asIntN(64, n) ? n : undefined; };",
    ),
];

/// The helpers `used` needs, with the ones they call, in a fixed order.
pub fn closure(used: &BTreeSet<&'static str>) -> Vec<&'static str> {
    let mut all: BTreeSet<&'static str> = used.clone();
    loop {
        let more: Vec<&'static str> = HELPERS
            .iter()
            .filter(|(name, _, _)| all.contains(name))
            .flat_map(|(_, needs, _)| needs.iter().copied())
            .filter(|n| !all.contains(n))
            .collect();
        if more.is_empty() {
            break;
        }
        all.extend(more);
    }
    HELPERS
        .iter()
        .map(|(name, _, _)| *name)
        .filter(|n| all.contains(n))
        .collect()
}

/// A helper's source; `bounds` is the text of an index out of bounds, with
/// `{index}` and `{length}` in it.
pub fn source(name: &str, bounds: &str, debug: bool) -> String {
    let source = HELPERS
        .iter()
        .find(|(n, _, _)| *n == name)
        .map_or("", |(_, _, source)| source);
    source
        .replace("BOUNDS", &crate::expr::string_literal(bounds))
        .replace("DEBUG ?", if debug { "true ?" } else { "false ?" })
}
