# A3, particles: runs of perasc check (runs/particles-*.json)

Written with Peras's own forms for declarations (struct/impl for the class), as the guide teaches first.

run 0, 1 error: E0104 `new Uint8ClampedArray(n)` (cause: no `new`, no typed array or framebuffer) -> an `Array<u8>` filled by a loop
run 1, 27 errors:

- E0210 x15, E0215 x1 (ceil of an integer division): a JavaScript number used as an integer and as a float, `width / 2`, `p.x >= width`, an index of f64 (cause: a number is i32 or f64, never both) -> conversions, offered
- E0205 x10 `const p = self.particles[i]; p.x += …` (cause: an element read into a constant is a copy, in TypeScript the object itself); the fix offered, `let`, applied as the protocol asks
- E0205 `const list = grid.get(key); list.push(i)` (cause: a map's `get` gives a copy) -> `let`, offered
- E0215 `Map.values()` (cause: no values() on a map) -> for…of over the entries, `entry.1`
- E0215 `Array.fill` (cause: no fill) -> a loop
  run 2, 8 errors: E0210 x7 conversions, offered; E0215 `push` on `Option<Array>`: `let list` is not narrowed (cause: a let is not narrowed), and a `const` cannot change -> read, change and set back: `grid.get(k).unwrapOr([])`, `push`, `grid.set(k, list)`
  run 3, 2 errors: E0207 `self` passed twice, `self.particles.push(Particle(… self.random() …))` (cause: a borrow rule) -> the random numbers drawn first; E0209 a field moved -> .copy(), offered
  run 4: checks; build fails: the release verifier refuses `Scene_step`, `self.particles = self.particles.filter(…)` on a field of `inout self`; a bug of perasc, F121: the lowering now reports E0900 (inout-replace), and the array is compacted in place by hand (cannot be written as in TypeScript)
  run 5: builds; 12000|858726222 where TypeScript gives 1792|694335550: the `let p` of E0205's fix changed copies, no particle died (F122, E0205's fix now says to write to the element); `p.life / 60` an integer division where TypeScript divides as floats (no error: its quotient met an integer, `Math.min(1, …)`); `sum * 31` overflows an i32 in a production build, which wraps, where TypeScript's number is exact; and a Uint8ClampedArray rounds a half to the even, `round` a half up
  run 6: written through the elements, divided as floats, the sum an f64, a clamp8 written by hand: builds; same result as TypeScript, twice on one instance.
