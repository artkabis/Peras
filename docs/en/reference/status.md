# What works today

Peras is young. This page says, feature by feature, how far each one goes:

- <span class="status compiled">compiled</span> `perasc build` turns it into WebAssembly;
- <span class="status checked">checked</span> `perasc check` accepts it, with every rule enforced, but `perasc build` reports [E0900](../errors/E0900.md);
- <span class="status planned">planned</span> designed in the [spec](../spec.md) or the [roadmap](roadmap.md), not in the language yet: the compiler refuses it.

## Types

| Feature                                                                                                             | Status                                        |
| ------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `i32`, `i64`, `f32`, `f64`, `bool`                                                                                  | <span class="status compiled">compiled</span> |
| `i8`, `i16`, `u8`, `u16`, `u32`, `u64`                                                                              | <span class="status checked">checked</span>   |
| structures, nested, as parameters and results                                                                       | <span class="status compiled">compiled</span> |
| `Option<T>`, `Some`, `None`, `?`                                                                                    | <span class="status compiled">compiled</span> |
| `Array<T>` of any type                                                                                              | <span class="status compiled">compiled</span> |
| `String`, in UTF-8; as a parameter, also UTF-8 bytes, checked ([decision 0020](../decisions/0020-text-as-bytes.md)) | <span class="status compiled">compiled</span> |
| enumerations with data, `match`, exhaustiveness checked                                                             | <span class="status compiled">compiled</span> |
| `Result<T, E>`, `Ok`, `Err`, `?`                                                                                    | <span class="status compiled">compiled</span> |
| generics, constraints `Eq`, `Ord`, `Numeric`, `Copy`…                                                               | <span class="status compiled">compiled</span> |
| methods (`impl`), interfaces as constraints                                                                         | <span class="status compiled">compiled</span> |
| tuples, type aliases                                                                                                | <span class="status compiled">compiled</span> |
| template literals, `` `${x} items` ``                                                                               | <span class="status compiled">compiled</span> |
| `Map`, `Set` with integer, boolean and string keys ([decision 0005](../decisions/0005-maps-and-sets.md))            | <span class="status compiled">compiled</span> |
| closures                                                                                                            | <span class="status planned">planned</span>   |
| `Rc<T>`, `Framebuffer`, pixel types, SIMD                                                                           | <span class="status planned">planned</span>   |

## Statements and expressions

| Feature                                                                                                            | Status                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| `let`, `const`, assignments, `+=` and the others, `++`, `--`                                                       | <span class="status compiled">compiled</span>                                                                         |
| arithmetic, comparisons, bitwise and logical operators, `%` on floats too, `==` on arrays of strings and of arrays | <span class="status compiled">compiled</span>                                                                         |
| `if`, `while`, `for`, `break`, `continue`                                                                          | <span class="status compiled">compiled</span>                                                                         |
| `for…of`                                                                                                           | <span class="status compiled">compiled</span>                                                                         |
| `parallel for`, on threads with `--threads on`                                                                     | <span class="status compiled">compiled</span>                                                                         |
| functions, recursion, `export`                                                                                     | <span class="status compiled">compiled</span>                                                                         |
| `sink` parameters                                                                                                  | <span class="status compiled">compiled</span>                                                                         |
| `inout` parameters, `&` at the call                                                                                | <span class="status compiled">compiled</span>                                                                         |
| ownership: moves, borrows, aliasing refused, releases                                                              | <span class="status compiled">compiled</span>                                                                         |
| module-level `const`                                                                                               | <span class="status compiled">compiled</span>                                                                         |
| module-level `let` ([decision 0023](../decisions/0023-module-variables.md))                                        | <span class="status compiled">compiled</span>, numbers and booleans, and structures, enumerations and options of them |
| `import` between owned modules, compiled into one module ([modules](../guide/modules.md))                          | <span class="status compiled">compiled</span>, functions, structures and enumerations that are not generic            |
| `import` of owned functions, structures and enumerations into a managed module                                     | <span class="status compiled">compiled</span>, numbers, strings, structures, enumerations and collections of them     |

## Methods

Every built-in method, with its status, is in the [method reference](methods.md).

## Modes and tooling

| Feature                                                                                                            | Status                                                                |
| ------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- |
| owned mode, `.peras`, to WebAssembly                                                                               | <span class="status compiled">compiled</span>, for the features above |
| managed mode, `.apeiron`, to JavaScript ([guide](../guide/managed.md))                                             | <span class="status compiled">compiled</span>, for the features above |
| diagnostics in English and French, `--json`, `perasc explain`                                                      | available                                                             |
| Vite plugin, TypeScript declarations                                                                               | available                                                             |
| debug allocator, release verifier                                                                                  | in use in every module that allocates                                 |
| `async`, `await`, `number`, in managed mode                                                                        | <span class="status compiled">compiled</span>                         |
| the DOM, events, a canvas, timers, `fetch`, storage ([browser](browser.md))                                        | <span class="status compiled">compiled</span>, in managed mode        |
| listeners and timers as resources, released with their owner or when replaced, leaks reported in debug             | <span class="status compiled">compiled</span>, in managed mode        |
| `extern` blocks: JavaScript and npm called with declared types, each value checked                                 | <span class="status compiled">compiled</span>, in managed mode        |
| arrow functions with statements, capturing what they use, as values ([guide](../guide/managed.md#arrow-functions)) | <span class="status compiled">compiled</span>, in managed mode        |
| editor support (LSP), formatter, test runner                                                                       | <span class="status planned">planned</span>                           |

What comes next, and in which order: [toward a complete language](roadmap.md).
