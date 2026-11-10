# Peras Runtime Exception

_Version française, à titre d’information : [RUNTIME-EXCEPTION.fr.md](RUNTIME-EXCEPTION.fr.md)._

Version 1.0

This is an additional permission under section 7 of the GNU Affero General Public License, version 3 or any later version (the "AGPL"), granted by the copyright holders of Peras. Peras is licensed under the AGPL ([LICENSE](LICENSE)); this exception adds to it and takes nothing away.

## Definitions

- **Peras** is the software in this repository: the `perasc` compiler, its crates, its build plugins, its runtime and its documentation.
- **Runtime Code** is the part of Peras that the compiler or a Peras build plugin writes into the output of a compilation: the runtime and the allocators of `crates/perasc-runtime`, the JavaScript loader and the TypeScript declarations that `perasc` generates, and the code that `vite-plugin-peras` inserts into a bundle.
- **Compiled Program** is what compiling source code with Peras produces, the Runtime Code it contains included, combined or not with other code.

## Permission

You may copy, modify, convey and use a Compiled Program, the Runtime Code it contains included, under terms of your choice, proprietary terms included. The AGPL does not apply to a Compiled Program, nor to the code it is combined with, because of the Runtime Code it contains.

The source code you write and compile with Peras is yours: the AGPL never applied to it.

## Limits

This permission does not cover:

- Peras itself, or a modified Peras: the compiler, its crates and its build plugins, whether used or conveyed on their own or inside another product, stay under the AGPL;
- Runtime Code taken out of a Compiled Program and used or conveyed on its own, or inside a product that is not a Compiled Program: the AGPL applies to it;
- a Compiled Program made by a modified Peras whose source code is not available under the AGPL with this exception.

## Examples

The code examples in the documentation (`docs/`) and in `examples/` may also be used without any condition, under the BSD Zero Clause License (0BSD): copying them into a program asks nothing of it.
