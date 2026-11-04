# Peras

_English version: [README.md](README.md), qui fait foi._

Peras est un langage web à la syntaxe TypeScript et à la rigueur de Rust. Les modules `.peras` sont compilés en WebAssembly avec une mémoire exacte, sans ramasse-miettes ; les modules `.apeiron` sont compilés en JavaScript, disciplinés par le compilateur.

> Peras (πέρας) signifie _la limite_ en grec. Pour les pythagoriciens, tout ce qui est bon naît de la limite imposée à l'illimité (_apeiron_).

**État :** version 0.0.1, fin de la phase 0 du plan de démarrage ([journal des modifications](CHANGELOG.fr.md)). `perasc` vérifie la syntaxe, les noms et les types d’une source, et compile en WebAssembly les fonctions sur des nombres et des booléens, sans toolchain Rust ; `vite-plugin-peras` les exécute dans le navigateur. Structures, tableaux, ownership et modules `.apeiron` viennent ensuite.

## Piliers

1. **Rasterisation** : framebuffers typés partagés avec le Canvas sans copie.
2. **Mémoire** : propriété unique, libération déterministe, aucune allocation dans une boucle de rendu.
3. **Syntaxe accessible** : un développeur TypeScript est productif en une journée.

Chaque diagnostic bloque le build et explique la cause, l'historique de la valeur et la correction, en anglais ou en français.

## Documentation

- [Spec du langage](docs/fr/spec.md)
- [Plan de démarrage du compilateur](docs/fr/kickoff-plan.md)

## Organisation du dépôt

| Chemin                       | Contenu                                             |
| ---------------------------- | --------------------------------------------------- |
| `crates/perasc-syntax`       | Lexer, parser, AST                                  |
| `crates/perasc-diag`         | Diagnostics, codes d'erreur, messages EN/FR         |
| `crates/perasc-check`        | Noms, types, ownership, ressources, budget de pile  |
| `crates/perasc-ir`           | IR typée partagée par les deux backends             |
| `crates/perasc-codegen-rs`   | `.peras` → Rust → WASM                              |
| `crates/perasc-codegen-wasm` | `.peras` → WASM, en direct, sans toolchain Rust     |
| `crates/perasc-codegen-js`   | `.apeiron` → JavaScript                             |
| `crates/perasc-driver`       | Pipeline et orchestration du build                  |
| `crates/perasc`              | Interface en ligne de commande                      |
| `packages/vite-plugin-peras` | Plugin Vite                                         |
| `packages/vitest-peras`      | Adaptateur Vitest                                   |
| `packages/create-peras`      | Création de projet                                  |
| `examples/fib`               | `fib` en Peras, exécuté dans le navigateur via Vite |
| `std/`                       | Bibliothèque standard, écrite en Peras              |
| `tests/ui/`                  | Fichiers source et leurs diagnostics attendus       |
| `docs/en`, `docs/fr`         | Documentation, anglais d'abord                      |

## Démarrer

Prérequis : [rustup](https://rustup.rs), Node 26 LTS (26.13.0+), pnpm 12+.

```sh
# Rust : rustup installe le toolchain épinglé dans rust-toolchain.toml
cargo test --workspace
cargo run -p perasc -- --version

# Vérifier des sources : chaque erreur avec extrait, cause et correction (--lang en|fr, --json)
cargo run -p perasc -- check crates/perasc/tests/fixtures/demo.peras --lang fr
cargo run -p perasc -- explain E0007 --lang fr

# JS : une fois, installe les dernières dépendances de dev stables et mûres (publiées depuis plus de 24 h), les audite et lance tous les contrôles
pnpm run bootstrap
```

Avant le premier push, commite le `pnpm-lock.yaml` créé par le bootstrap : la CI l'installe en mode figé.

## Exécuter Peras dans le navigateur

```sh
cargo build --release -p perasc
pnpm --filter peras-example-fib dev
```

`examples/fib/src/main.ts` importe `fib` depuis `fib.peras` comme n’importe quel module. `vite-plugin-peras` le compile avec le backend direct de `perasc` : en développement, le WebAssembly est intégré au module, et `vite build` l’émet comme un fichier à empreinte, compilé pendant son téléchargement. Une erreur dans la source s’affiche dans l’overlay de Vite avec son extrait, sa cause et sa correction, et fait échouer `vite build`. Le plugin écrit aussi `fib.d.peras.ts` à côté de la source, pour que TypeScript connaisse les vraies signatures (avec `allowArbitraryExtensions`).

```ts
// vite.config.ts
import peras from "vite-plugin-peras";

export default { plugins: [peras({ perasc: "chemin/vers/perasc", lang: "fr" })] };
```

`pnpm run e2e` construit l’exemple, l’ouvre dans un Chromium sans interface sur le serveur de développement et sur le build de production, et vérifie que l’overlay affiche les erreurs. Il faut `target/release/perasc` et un Chromium : `pnpm exec playwright install chromium`, ou `PERAS_CHROMIUM` pointant vers un Chromium installé.

## Contribuer

Voir [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md). En bref : anglais d'abord, puis français dans la même pull request ; aucun avertissement, aucun `unsafe`, aucune CVE connue.

## Licence

À décider (MIT, Apache-2.0, ou les deux). Les manifestes utilisent `MIT OR Apache-2.0` en attendant.
