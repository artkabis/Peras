# A2, ledger: runs of perasc check (runs/ledger-*.json)

run 0, 7 errors, all syntax (no edit offered):

- E0102 destructuring `const [a, b, c, d] = line.split(",")` -> parts[i], length check (cause: no destructuring)
- E0100 x2 spread `[...entries]` -> entries.copy() (cause: no spread); and `.sort((a, b) => cmp)` -> sortBy key (cause: no comparator sort) — not an error yet, rewritten with the spread
- E0104 x2 ternary `c ? a : b` -> let + if (cause: no conditional expression)
- E0100 + E0104 `[...map.entries()]` and `map(([m, c]) => …)` -> loop over the map, `e.0`, `e.1` (cause: no destructured parameters; no spread)
  run 1, 10 errors:
- E0200 x2 `Number(x)`, `Number.isNaN` -> parseF64(), isNone (cause: JavaScript globals Number/String absent)
- E0210 x2 `let balance = 0` is i32, then i64 added -> annotate `: i64` (cause: a variable takes i32 from its literal, not from its later use)
- E0215 `toFixed` on i64 -> toF64() / 100.0 (cause: toFixed only on floats)
- E0212 `2147483648` does not fit i32 (cause: an integer number above 2^31 from JS)
- E0215 floor of integer division (cause: same)
- E0210 index of f64 (cause: same, number as index)
- E0200 x2 `String(month).padStart(2, "0")` -> a pad2 written by hand (cause: String() and padStart absent)
  run 2, 1 error: E0210 Math.round gives f64 for an i64 field -> .toI64(), offered
  run 3, 7 errors: E0209 x7 move out of an array element or a structure field -> .copy(), offered (cause: ownership, elements and fields lent)
  run 4: builds. Its result differed from TypeScript's: seed * 1103515245 exceeds 2^53, JavaScript's number loses digits, the i64 does not; generator rewritten in f64 as the TS's number (cause: a JS number used as an integer; i64 exact, f64 as JS)
  run 5: builds; same result as TypeScript, twice on one instance.
  Also latent, kept from the TS: `label ?? ""` lost by the literal translation, parts[3] would stop the program on a line without label; restored by hand.
