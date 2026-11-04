# Contributing to Peras

_Version française : [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md)_

## English first, then French

English is the source of truth for code comments, documentation, compiler messages and commit messages. Every user-facing text lands in English **and** French in the same pull request:

- compiler messages: `crates/*/messages/en.ftl` and `fr.ftl`, same keys;
- documentation: `docs/en/<page>` and `docs/fr/<page>`, same paths;
- root files: `README.md` / `README.fr.md`, `CONTRIBUTING.md` / `CONTRIBUTING.fr.md`.

`node scripts/check-i18n.mjs` checks this locally; CI blocks the merge otherwise.

## Zero tolerance

- No warning: `cargo clippy -- -D warnings`, `eslint --max-warnings 0`.
- No `unsafe`: every crate inherits `unsafe_code = "forbid"`.
- No known CVE: `cargo deny` (RustSec advisories) and `pnpm audit` block the merge.
- No unreviewed snapshot: `cargo insta review` before pushing.

Run everything locally before pushing:

```sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm run check
```

## Error codes

A new compiler error ships complete, in one pull request:

1. a new code in the registry (`E` + four digits), never reusing a retired one;
2. its message in `en.ftl` then `fr.ftl`;
3. a suggested fix;
4. a UI test in `tests/ui/`;
5. its page in `docs/en/errors/` then `docs/fr/errors/`, used by `perasc explain`. A test compiles the example of every page, which must produce the code, and its fix, which must not. Fragments are allowed: `…` stands for anything, and statements are tried inside a function. Write invisible characters as `<U+202E>`.

## Fuzzing

`fuzz/` holds three [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html) targets, run for 30 minutes every night by CI:

- `lex`: the lexer never panics, and its tokens stay in order inside the source;
- `parse`: the parser never panics nor overflows its stack;
- `pipeline`: every diagnostic renders in both languages, and a source without errors compiles to a valid WebAssembly module, never to a compiler bug.

```sh
rustup toolchain install nightly
cargo install --locked cargo-fuzz
sh fuzz/seed.sh   # every Peras source of the repository as a starting corpus
cd fuzz && cargo +nightly fuzz run pipeline corpus/pipeline seeds -- -max_total_time=600
```

A crash leaves its input in `fuzz/artifacts/<target>/`. Fix it with a regression test that uses this input, in the crate that panicked.

## Commits and pull requests

Pull request titles follow [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. The changelog is written from them. Versions follow semver and stay `0.x` until the language is stable.

## Dependencies

Dependencies stay on their latest stable versions, once a release is at least 24 hours old (`minimumReleaseAge`): freshly compromised packages are usually pulled within that window. Never add a `minimumReleaseAgeExclude` entry; a security fix still arrives right away through Renovate vulnerability alerts. GitHub Actions are pinned to a commit SHA. Renovate opens the update pull requests; CI decides. A toolchain bump (`rust-toolchain.toml`) gets its own pull request.
