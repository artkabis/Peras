# The `perasc` command

```text
perasc <command> [options]
```

## Commands

| Command                  | Does                                                        |
| ------------------------ | ----------------------------------------------------------- |
| `perasc check <files…>`  | checks every file and reports every error, without building |
| `perasc build <file>`    | checks, then compiles one `.peras` file to WebAssembly      |
| `perasc explain <code>`  | shows the page of an error code, such as `E0006`            |
| `perasc --version`, `-V` | shows the version                                           |
| `perasc --help`, `-h`    | shows the help, in your language                            |

## Options

| Option                        | Applies to       | Meaning                                                                                                        |
| ----------------------------- | ---------------- | -------------------------------------------------------------------------------------------------------------- |
| `--lang en\|fr`               | all              | the language of messages; default: the `PERAS_LANG` variable, then the system's language                       |
| `--json`                      | `check`, `build` | diagnostics as JSON on standard output                                                                         |
| `--color auto\|always\|never` | `check`, `build` | colours in diagnostics; `auto` (the default) colours a terminal, and the `NO_COLOR` variable turns colours off |
| `--out <dir>`                 | `build`          | where to write the files; default: `dist`                                                                      |
| `--backend direct\|rust`      | `build`          | the code generator; `rust` needs a Rust toolchain and is used by the project's tests                           |
| `--no-optimize`               | `build`          | skips the IR optimizations                                                                                     |

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
