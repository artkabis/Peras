# Peras

*English version: [README.md](README.md), qui fait foi.*

Peras est un langage web à la syntaxe TypeScript et à la rigueur de Rust. Les modules `.peras` sont compilés en WebAssembly avec une mémoire exacte, sans ramasse-miettes ; les modules `.apeiron` sont compilés en JavaScript, disciplinés par le compilateur.

> Peras (πέρας) signifie *la limite* en grec. Pour les pythagoriciens, tout ce qui est bon naît de la limite imposée à l'illimité (*apeiron*).

**État :** jour 1 du plan de démarrage. Le dépôt compile, passe les lints et les tests, mais le compilateur ne compile encore rien.

## Piliers

1. **Rasterisation** : framebuffers typés partagés avec le Canvas sans copie.
2. **Mémoire** : propriété unique, libération déterministe, aucune allocation dans une boucle de rendu.
3. **Syntaxe accessible** : un développeur TypeScript est productif en une journée.

Chaque diagnostic bloque le build et explique la cause, l'historique de la valeur et la correction, en anglais ou en français.

## Documentation

- [Spec du langage](docs/fr/spec.md)
- [Plan de démarrage du compilateur](docs/fr/kickoff-plan.md)

## Organisation du dépôt

| Chemin | Contenu |
| --- | --- |
| `crates/perasc-syntax` | Lexer, parser, AST |
| `crates/perasc-diag` | Diagnostics, codes d'erreur, messages EN/FR |
| `crates/perasc-check` | Noms, types, ownership, ressources, budget de pile |
| `crates/perasc-ir` | IR typée partagée par les deux backends |
| `crates/perasc-codegen-rs` | `.peras` → Rust → WASM |
| `crates/perasc-codegen-js` | `.apeiron` → JavaScript |
| `crates/perasc-driver` | Pipeline et orchestration du build |
| `crates/perasc` | Interface en ligne de commande |
| `packages/vite-plugin-peras` | Plugin Vite |
| `packages/vitest-peras` | Adaptateur Vitest |
| `packages/create-peras` | Création de projet |
| `std/` | Bibliothèque standard, écrite en Peras |
| `tests/ui/` | Fichiers source et leurs diagnostics attendus |
| `docs/en`, `docs/fr` | Documentation, anglais d'abord |

## Démarrer

Prérequis : [rustup](https://rustup.rs), Node 24+, pnpm 10+.

```sh
# Rust : rustup installe le toolchain épinglé dans rust-toolchain.toml
cargo test --workspace
cargo run -p perasc -- --version

# JS : une fois, installe les dernières dépendances de dev, les audite et lance tous les contrôles
pnpm run bootstrap
```

Avant le premier push, commite le `pnpm-lock.yaml` créé par le bootstrap : la CI l'installe en mode figé.

## Contribuer

Voir [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md). En bref : anglais d'abord, puis français dans la même pull request ; aucun avertissement, aucun `unsafe`, aucune CVE connue.

## Licence

À décider (MIT, Apache-2.0, ou les deux). Les manifestes utilisent `MIT OR Apache-2.0` en attendant.
