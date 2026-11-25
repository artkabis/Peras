# A1, notes: runs of perasc check (runs/notes-*.json)

A managed module. Written with Peras's own forms for declarations (struct for the interfaces), as the guide teaches first.

run 0, 4 errors, all syntax (no edit offered): E0100 x2 spread `[...map.entries()]`, `[...notes, note]` (cause: no spread); E0104 `([t, c]) => …` (cause: no destructured parameters); E0100 `replaceChildren(...items)` (cause: no spread of arguments); `hits[0]?.id ?? -1` rewritten with them (cause: no optional chaining)
run 1, 18 errors:

- E0215, E0238 x2 `sort((a, b) => b.updated - a.updated)` (cause: no comparator sort) -> sortByDescending
- E0210 `1000 + i` where an f64 is expected (cause: a number is i32 or f64) -> .toF64(), offered
- 14 errors of the browser written as in TypeScript: `HTMLElement`, `localStorage`, `Date.now()`, `addEventListener`, `removeEventListener`, `clearTimeout`, `replaceChildren`, `value` on an element, `setTimeout(f, 150)`, a cleanup function exported (cause: the browser is typed with Peras's own names, `storage`, `now()`, `on` giving a resource, `Group`, `Slot`, `setTimeout(Duration, f)`, `asInput()`) -> the page rewritten with the browser reference
  run 2, 1 error: E0245 `group` used after it was given away, where it was returned in an `if` that leaves: a bug of perasc, F123, fixed: a branch that leaves is no path to what follows
  run 3: builds; same result as TypeScript, twice on one instance.
