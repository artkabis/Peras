# What works today

Peras is young. This page says, feature by feature, how far each one goes:

- <span class="status compiled">compiled</span> `perasc build` turns it into WebAssembly;
- <span class="status checked">checked</span> `perasc check` accepts it, with every rule enforced, but `perasc build` reports [E0900](../errors/E0900.md);
- <span class="status planned">planned</span> designed in the [spec](../spec.md) or the [roadmap](roadmap.md), not in the language yet: the compiler refuses it.

## Types

| Feature                                                | Status                                        |
| ------------------------------------------------------ | --------------------------------------------- |
| `i32`, `i64`, `f32`, `f64`, `bool`                     | <span class="status compiled">compiled</span> |
| `i8`, `i16`, `u8`, `u16`, `u32`, `u64`                 | <span class="status checked">checked</span>   |
| structures, nested, as parameters and results          | <span class="status compiled">compiled</span> |
| `Option<T>`, `Some`, `None`, `?`                       | <span class="status compiled">compiled</span> |
| `Array<T>` of numbers, booleans, structures, options   | <span class="status compiled">compiled</span> |
| `String`, in UTF-8                                     | <span class="status compiled">compiled</span> |
| `Array<T>` of strings or arrays                        | <span class="status checked">checked</span>   |
| `Result<T, E>`, enumerations and unions, `match`       | <span class="status planned">planned</span>   |
| generics, interfaces and traits, methods on structures | <span class="status planned">planned</span>   |
| `Map`, `Set`, tuples, closures                         | <span class="status planned">planned</span>   |
| `Rc<T>`, `Framebuffer`, pixel types, SIMD              | <span class="status planned">planned</span>   |

## Statements and expressions

| Feature                                                      | Status                                                                                                          |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| `let`, `const`, assignments, `+=` and the others, `++`, `--` | <span class="status compiled">compiled</span>                                                                   |
| arithmetic, comparisons, bitwise and logical operators       | <span class="status compiled">compiled</span>, except `%` on floats <span class="status checked">checked</span> |
| `if`, `while`, `for`, `break`, `continue`                    | <span class="status compiled">compiled</span>                                                                   |
| `for…of`                                                     | <span class="status compiled">compiled</span>                                                                   |
| functions, recursion, `export`                               | <span class="status compiled">compiled</span>                                                                   |
| `sink` parameters                                            | <span class="status compiled">compiled</span>                                                                   |
| `inout` parameters, `&` at the call                          | <span class="status compiled">compiled</span>                                                                   |
| ownership: moves, borrows, aliasing refused, releases        | <span class="status compiled">compiled</span>                                                                   |
| module-level `const`                                         | <span class="status checked">checked</span>                                                                     |
| `import` between modules                                     | <span class="status planned">planned</span>                                                                     |

## Methods

Every built-in method, with its status, is in the [method reference](methods.md).

## Modes and tooling

| Feature                                                       | Status                                                                |
| ------------------------------------------------------------- | --------------------------------------------------------------------- |
| owned mode, `.peras`, to WebAssembly                          | <span class="status compiled">compiled</span>, for the features above |
| managed mode, `.apeiron`, to JavaScript                       | <span class="status planned">planned</span>                           |
| diagnostics in English and French, `--json`, `perasc explain` | available                                                             |
| Vite plugin, TypeScript declarations                          | available                                                             |
| debug allocator, release verifier                             | built and tested, waiting for the first heap type                     |
| `async`, the DOM, `fetch`, events                             | <span class="status planned">planned</span>, in managed mode          |
| editor support (LSP), formatter, test runner                  | <span class="status planned">planned</span>                           |

What comes next, and in which order: [toward a complete language](roadmap.md).
