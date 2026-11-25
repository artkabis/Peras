# A4, routes: runs of perasc check (runs/routes-*.json)

Written with Peras's own forms for declarations (struct/impl for the class, enum for the union, while (true) for for (;;)), as the guide teaches first: a bias, fewer errors than a developer who has not read it.
run 0, 3 errors: E0114 x2 swap `[a[i], a[j]] = [a[j], a[i]]` -> a variable (cause: no destructuring); E0102 `for (const [a, b] of pairs)` -> two arrays by index (cause: no destructuring)
run 1, 14 errors:

- E0210 `pop()` gives an option even after a length test -> unwrapOr (cause: pop gives Option)
- E0214 x3 `map(() => …)` takes one argument (cause: an arrow must declare its parameter)
- E0210 x2 `map((s) => [])`, `map((s) => None)` element type unknown (cause: an empty array or None in an arrow does not take the declared type) — shown in run 3
- E0200 x2 `Infinity` (cause: no Infinity)
- E0215 x2 `next.0` on `let next` in `for (let next = q.pop(); next.isSome(); …)` (cause: a let is not narrowed)
- E0210 x2 `at` in `while (at.isSome())` (cause: a let is not narrowed)
- E0210 best[end] f64 into i32 (cause: Infinity replaced, types)
- E0210 `fromJson` gives a Result (cause: JSON read gives Result, as designed)
  run 2, 2 errors: E0113 `Ok(v) => network = v`, E0110 (cause: a match arm is not an assignment or a return; blocks needed)
  run 3, 2 errors: E0210 x2 the arrow's empty array and None (above) -> loops
  run 4, 4 errors: E0209 x4 moves out of elements and of the match's bindings -> .copy(), offered
  run 5: checks; build fails: E0900 `Network.fromJson` in an owned module (cause: JSON not compiled in owned modules; check does not say it) -> network used as made (cannot be written)
  run 6: build fails: IR verifier refuses `route`: a lent parameter moved into a variant, `Route.Missing(from)`; a bug of perasc, F119, fixed: the ownership pass now takes a variant's data
  run 7, 2 errors: E0209 x2 the variant's parameters -> `sink`, offered
  run 8, 2 errors: E0209 x2 elements given to the sink parameters -> .copy(), offered
  run 9: builds; same result as TypeScript, twice on one instance.
