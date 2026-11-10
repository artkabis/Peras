# The `perasc` command

```text
perasc <command> [options]
```

## Commands

| Command                  | Does                                                                                                                                                                             |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `perasc check <files…>`  | checks every file and reports every error, without building                                                                                                                      |
| `perasc build <file>`    | checks, then compiles one `.peras` file to WebAssembly, or one `.apeiron` file to JavaScript with the owned modules it imports ([managed modules](../guide/managed.md#building)) |
| `perasc explain <code>`  | shows the page of an error code, such as `E0006`                                                                                                                                 |
| `perasc --version`, `-V` | shows the version                                                                                                                                                                |
| `perasc --help`, `-h`    | shows the help, in your language                                                                                                                                                 |

## Options

| Option                        | Applies to       | Meaning                                                                                                                                                                                          |
| ----------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `--lang en\|fr`               | all              | the language of messages; default: the `PERAS_LANG` variable, then the system's language                                                                                                         |
| `--json`                      | `check`, `build` | diagnostics as JSON on standard output                                                                                                                                                           |
| `--color auto\|always\|never` | `check`, `build` | colours in diagnostics; `auto` (the default) colours a terminal, and the `NO_COLOR` variable turns colours off                                                                                   |
| `--out <dir>`                 | `build`          | where to write the files; default: `dist`                                                                                                                                                        |
| `--backend direct\|rust`      | `build`          | the code generator; `rust` needs a Rust toolchain and is used by the project's tests                                                                                                             |
| `--no-optimize`               | `build`          | skips the IR optimizations                                                                                                                                                                       |
| `--allocator debug\|tlsf`     | `build`          | the allocator of a module that uses the heap: `debug`, the default, catches every memory bug; `tlsf` allocates in bounded time, for production ([0002](../decisions/0002-tlsf-allocator.md))     |
| `--simd on\|off`              | `build`          | WebAssembly SIMD in loops and the runtime: `on`, the default, is faster; `off` loads in engines without it, such as Safari before 16.4 ([0004](../decisions/0004-strings-without-allocation.md)) |
| `--threads on\|off`           | `build`          | `parallel for` loops on every core of a cross-origin isolated page, on one thread elsewhere; `off` by default ([0007](../decisions/0007-threads-over-shared-memory.md))                          |
| `--imports built\|source`     | `build`          | a `.apeiron` module's imports of `.peras` files: `built`, the default, builds each next to it and imports its loader; `source` keeps them for a bundler, such as Vite's plugin                   |

## Exit codes

| Code | Means                                   |
| ---- | --------------------------------------- |
| `0`  | success                                 |
| `1`  | the source has errors                   |
| `2`  | a usage, file or tool problem           |
| `3`  | a bug in the compiler: please report it |

## Examples

```sh
perasc check src/*.peras --lang fr
perasc build src/physics.peras --out public/wasm
perasc check src/app.peras --json > diagnostics.json
perasc explain E0108
```
