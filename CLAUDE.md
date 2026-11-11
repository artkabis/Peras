# Working on Peras

Read at the start of every session. It holds the rules of the project and the traps already fallen into, so that they are not fallen into again. When a new trap is found, add it here the same day, and add the finding to [docs/en/findings.md](docs/en/findings.md) and its French translation.

## The project's rules

- **Report to the owner in French.** Docs are written in English first, then in French; `node scripts/check-i18n.mjs` checks that every page exists in both.
- **No dependency on another language than Rust**, not even in a benchmark. A competitor that needs an npm package or a C library is cited from its published figures, never installed.
- **A build fails on any error.** Before a commit: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm run check` (format, lint, types, tests, translations, `pnpm audit` with no vulnerability), `pnpm run docs`, and the `--check` of each benchmark touched.
- **pnpm installs nothing published less than 24 hours ago** (`minimumReleaseAge`); never add an exclusion.
- **Commits** follow Conventional Commits, signed off (`git commit -s`), with the attribution lines the session gives. No model name in the repository.
- **No force push.** After a pull request is merged, merge `main` into the branch.
- **Nothing published outside the repository** (forums, social networks) without the owner.

## The environment

- Node: `export PATH=$(ls -d /opt/node*/bin | tail -1):$PATH`.
- Chromium: never `playwright install`; set `PERAS_CHROMIUM=/opt/pw-browsers/chromium`, or `executablePath` in a script.
- A script that imports the repository's packages runs from the repository, not from the scratchpad.
- GitHub Pages must be enabled by the owner (Settings, Pages, Source: GitHub Actions); until it is, nothing is online.

## Studies: the traps already fallen into

Each line is a finding of [docs/en/findings.md](docs/en/findings.md).

- **Register the protocol before any measure**, and test its definitions against a first version before registering: two rules that disagree are found by code, not by reading (M003).
- **A value that could not be measured makes a verdict undetermined**, never a hypothesis that holds (M001).
- **Check every competitor against the rules of the project** before naming it in a protocol (M004).
- **Count alike**: what is left out of one version's count is left out of every version's (M006).
- **Format, lint and commit before a measure**, so that the run is at a known commit; a run started on code that changes afterwards is stopped and run again (M002).
- **Nothing that uses the processor during a measure**: no build, no test. Write, read, plan.
- **A flawed run is kept with its flaw**, archived, and its correction listed as a departure from the protocol.
- **Never say a thing is online, deployed or passing before seeing it**: read the workflow's run, not the workflow's file (M005).
- **"All checks pass" means the CI of the last commit, read on GitHub**, and locally every `--check` script of `scripts/`, not only those of the files touched: a change of defaults breaks the scripts that relied on them (M007).

## Code: the traps already fallen into

- **Edit by exact, unique replacements.** Never replace a slice between two markers: one script deleted every function between `keep_before` and the end of a file. After a scripted edit, `git diff --stat` and a build.
- **The release verifier is right until proven otherwise.** When it refuses code, reduce the case to a few lines, add it as a test that fails, then fix the lowering (F001, F002).
- **A test that passes here and fails in CI is a race until proven otherwise.** Reproduce it by changing the timing (a smaller or larger workload), trace the messages, and make the code wait for an explicit signal rather than raising a timeout (F106).
- **Write Peras as a user would before claiming what it can do.** The frictions found that way go to the findings, with the diagnostic as evidence.

## Peras as it is today

What a program can rely on, and what it cannot yet; the details and their evidence are in the findings and in [the status](docs/en/reference/status.md).

- `Option`: `isSome()`, `isNone()`, `unwrapOr(x)` (always evaluates `x`), and `?`. No `unwrap`, no `let … else`; a `match` whose arm leaves a loop gives no value (F101, F104, F105).
- Structures have no `copy()` (F102). An argument moved is not readable by a later argument of the same call (F103).
- `parallel for`: no call, no allocation, no reduction in a turn (F301).
- Integers: `i32` and `i64` compiled; `i8`, `i16`, `u8`, `u16`, `u32`, `u64` checked only (F302).
- Slow: `parseI32`, maps keyed by strings built per row (F201, F202). Fast: `split`, sorts by a key.
