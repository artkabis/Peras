# 0006: Licences: an open core under the AGPL, compiled programs free, tools proprietary

_French translation: [../../fr/decisions/0006-licensing.md](../../fr/decisions/0006-licensing.md)._

## Context

Until this decision, Peras had no licence. The README said "to be decided", and the manifests declared `MIT OR Apache-2.0` as a placeholder. A repository without a licence cannot be used or contributed to safely; a placeholder that reads as a permissive licence says the opposite of what the project wants. An outside review of the project named the missing licence among the reasons not to adopt it yet.

What the project wants:

- the language stays open source: anyone can read, build, change and share it;
- its code cannot be taken into a closed product, sold or offered as a service, without that product's code being opened too;
- programs written in Peras belong to their authors, under the terms they choose, closed ones included: a language whose output carries its licence would not be used;
- the tools grafted onto Peras, the memory leak analyzer and the VS Code extension, are kept under much stricter terms.

An open-source licence, in the sense of the Open Source Initiative, cannot forbid selling. What it can do is require that a modified version, when shared, be shared with its source under the same licence: that is copyleft.

## Options

| Licence                     | What it requires                                                                     | Why not                                                                    |
| --------------------------- | ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| MIT, Apache-2.0             | nothing but the notice                                                               | anyone can close Peras into a product: the opposite of the goal            |
| MPL-2.0                     | modified files of Peras are published                                                | Peras can be built into a closed product, its own files aside              |
| GPL-3.0                     | a modified Peras, when distributed, is published under the GPL                       | a modified Peras run only on a server, as a service, need not be published |
| **AGPL-3.0**                | the GPL's requirement, also when a modified Peras is offered to users over a network | chosen                                                                     |
| PolyForm Noncommercial, BSL | no commercial use at all, or not before a date                                       | not open source: distributions, companies and most contributors stay away  |

## Decision

1. **The core is under the AGPL-3.0-or-later**: the compiler and its crates, the runtime, the build plugins (`vite-plugin-peras`, `vitest-peras`, `create-peras`), the website and the documentation. The full text is in [LICENSE](../../../LICENSE).
2. **Compiled programs are free, by an exception.** The runtime, the allocators and the JavaScript loader are written into every compiled program: under the AGPL alone, every program compiled with Peras would be under the AGPL. The [Peras Runtime Exception](../../../RUNTIME-EXCEPTION.md), modelled on the GCC Runtime Library Exception, lets anyone use and distribute a compiled program under terms of their choice. It does not cover Peras itself, nor the runtime taken out of a compiled program and used on its own: those stay under the AGPL, so the runtime cannot be lifted into another product.
3. **Examples are free.** The code examples of the documentation and of `examples/` are also under the BSD Zero Clause Licence: copying them into a program asks nothing of it.
4. **The tools are proprietary.** The memory leak analyzer and the VS Code extension will live in a separate, private repository, all rights reserved, distributed under an end-user licence, free or paid as the project decides. They use Peras through what it publishes (its command line, its diagnostics in JSON, its compiled modules), never by taking AGPL code into them, so that their own code stays theirs. The leak checks already in Peras stay in the open core: the debug allocator that catches a block released twice or left alive, and `__perasHeap.checkLeaks()`; the analyzer is a tool on top of them. What else is a tool of this kind, the language server for instance, is decided when it is built.
5. **Contributions keep the door to a commercial licence open.** A company that wants to embed Peras itself in a closed product can be offered a commercial licence; that needs the rights on every contribution. [CONTRIBUTING.md](../../../CONTRIBUTING.md#licence-of-contributions) asks each contributor to license their contribution under the AGPL with the exception, and to grant the project's owner the right to license it under other terms too; contributors keep their copyright.

## To check with a lawyer before a first release

- The exception is written for this project, after the GCC Runtime Library Exception; a lawyer should read it before a release is published under it.
- Most of this repository was written with an AI assistant, under the direction of the project's owner. Whether and how copyright attaches to such code differs between countries; that matters most for selling commercial licences.
- The name "Peras" is not protected by any licence; a trademark, if one is wanted, is a separate step.

## Consequences

- The manifests declare `AGPL-3.0-or-later`, `examples/fib` `0BSD`. `cargo deny` checks the licences of dependencies as before; the workspace's own crates, all `publish = false`, are left out of that check.
- Every dependency of the compiler is under a licence compatible with the AGPL-3.0 (MIT, Apache-2.0, BSD, ISC, Unicode, Zlib); `libFuzzer` (NCSA) is used only by the fuzz targets.
- The roadmap says which tools are proprietary.
