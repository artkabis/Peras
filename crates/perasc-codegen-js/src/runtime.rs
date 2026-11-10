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
        "const $group = (where) => { const held = []; let open = true; const group = new $Resource(() => { open = false; while (held.length > 0) held.pop().release(); }, where); group.add = (r) => { if (open) held.push(r); else r.release(); }; return group; };",
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
