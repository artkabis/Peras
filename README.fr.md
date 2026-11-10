# Peras

_English version: [README.md](README.md), qui fait foi._

Peras est un langage web à la syntaxe TypeScript et à la rigueur de Rust. Les modules `.peras` sont compilés en WebAssembly avec une mémoire exacte, sans ramasse-miettes ; les modules `.apeiron` sont compilés en JavaScript, disciplinés par le compilateur.

> Peras (πέρας) signifie _la limite_ en grec. Pour les pythagoriciens, tout ce qui est bon naît de la limite imposée à l'illimité (_apeiron_).

**État :** version 0.0.1, en phase 3 de la [feuille de route](docs/fr/reference/roadmap.md), la bibliothèque standard ([journal des modifications](CHANGELOG.fr.md), [ce qui est compilé aujourd’hui](docs/fr/reference/status.md)). `perasc` vérifie la syntaxe, les noms, les types et la possession d’une source, et la compile en WebAssembly sans toolchain Rust : nombres, structures, énumérations et `match`, options et résultats, génériques, méthodes et interfaces, tableaux et chaînes avec leurs méthodes, fonctions fléchées pour `map`, `filter` et les autres, et chaînes gabarits. Chaque bloc de mémoire est libéré par le compilateur, vérifié par un vérificateur, avec un allocateur de débogage en développement et TLSF en production. `vite-plugin-peras` exécute les modules dans le navigateur. `Map` et `Set` ([décision 0005](docs/fr/decisions/0005-maps-and-sets.md)) et les modules `.apeiron` viennent ensuite.

**Vitesse**, mesurée dans Chromium face à du JavaScript écrit pour la vitesse ([banc d’essai](bench/README.fr.md#banc-dessai-dans-le-navigateur-benchbrowser)) : 5,4 fois plus rapide sur les entiers 64 bits, 1,6 à 2,4 fois sur les nombres en tableaux, le tri et le texte, la même vitesse sur les structures et l’arithmétique flottante seule, et plus lent sur les appels très courts à travers la frontière. Face à JavaScript tel qu’on l’écrit habituellement, les écarts sont plus grands ; les deux sont publiés.

## Piliers

1. **Rasterisation** : framebuffers typés partagés avec le Canvas sans copie.
2. **Mémoire** : propriété unique, libération déterministe, aucune allocation dans une boucle de rendu.
3. **Syntaxe accessible** : un développeur TypeScript est productif en une journée.

Chaque diagnostic bloque le build et explique la cause, l'historique de la valeur et la correction, en anglais ou en français.

## Documentation

- [Guide](docs/fr/guide/introduction.md), du premier programme à l’échange avec JavaScript
- [Référence](docs/fr/reference/status.md) : état, types, opérateurs, méthodes, mots-clés, CLI, [feuille de route](docs/fr/reference/roadmap.md)
- [Codes d’erreur](docs/fr/errors/index.md), chacun avec sa cause et sa correction
- Décisions : [0001 backend WebAssembly](docs/fr/decisions/0001-webassembly-backend.md), [0002 allocateur TLSF](docs/fr/decisions/0002-tlsf-allocator.md), [0003 un runtime prouvé à la compilation](docs/fr/decisions/0003-runtime-proved-at-compile-time.md), [0004 du texte sans allocation](docs/fr/decisions/0004-strings-without-allocation.md), [0005 tables et ensembles](docs/fr/decisions/0005-maps-and-sets.md)
- [Bancs d’essai](bench/README.fr.md), ce qu’ils mesurent et ce qu’ils ne mesurent pas
- [Spec du langage](docs/fr/spec.md), [architecture](docs/fr/architecture.md), [plan de démarrage du compilateur](docs/fr/kickoff-plan.md)

## Organisation du dépôt

| Chemin                       | Contenu                                              |
| ---------------------------- | ---------------------------------------------------- |
| `crates/perasc-syntax`       | Lexer, parser, AST                                   |
| `crates/perasc-diag`         | Diagnostics, codes d'erreur, messages EN/FR          |
| `crates/perasc-check`        | Noms, types, ownership, ressources, budget de pile   |
| `crates/perasc-ir`           | IR typée partagée par les deux backends              |
| `crates/perasc-codegen-rs`   | `.peras` → Rust → WASM                               |
| `crates/perasc-codegen-wasm` | `.peras` → WASM, en direct, sans toolchain Rust      |
| `crates/perasc-codegen-js`   | `.apeiron` → JavaScript                              |
| `crates/perasc-driver`       | Pipeline et orchestration du build                   |
| `crates/perasc-runtime`      | Runtime WebAssembly : allocateurs, chaînes, tableaux |
| `crates/perasc`              | Interface en ligne de commande                       |
| `packages/vite-plugin-peras` | Plugin Vite                                          |
| `packages/vitest-peras`      | Adaptateur Vitest                                    |
| `packages/create-peras`      | Création de projet                                   |
| `examples/fib`               | `fib` en Peras, exécuté dans le navigateur via Vite  |
| `std/`                       | Bibliothèque standard, écrite en Peras               |
| `tests/ui/`                  | Fichiers source et leurs diagnostics attendus        |
| `docs/en`, `docs/fr`         | Documentation, anglais d'abord                       |

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

Peras est un logiciel libre sous [GNU Affero General Public License, version 3 ou ultérieure](LICENSE), avec l’[exception de runtime de Peras](RUNTIME-EXCEPTION.fr.md) : les programmes que vous compilez avec Peras vous appartiennent, aux conditions de votre choix, fermées comprises ; Peras lui-même, modifié ou non, reste sous AGPL, y compris proposé comme service. Les exemples de code de la documentation et de `examples/` sont sous 0BSD. L’analyseur de fuites mémoire et l’extension VS Code sont des outils séparés et propriétaires. Pourquoi : [décision 0006](docs/fr/decisions/0006-licensing.md).

Copyright © 2026 Artkabis et les contributeurs de Peras.
