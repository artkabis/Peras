# Peras — Plan de démarrage du compilateur

_Dernière mise à jour : 2026-11-03. Traduction de [la version anglaise](../en/kickoff-plan.md), qui fait foi._

La phase 0 se termine quand une fonction `.peras` est compilée en WASM, s'exécute dans le navigateur via Vite, et que chaque erreur de syntaxe affiche un extrait, une cause et une correction en anglais et en français.

## Organisation du dépôt

Un seul monorepo contient un workspace Cargo pour le compilateur et un workspace pnpm pour l'outillage JS, afin qu'une seule exécution de CI vérifie les deux.

```text
peras/
├─ Cargo.toml              workspace Cargo
├─ rust-toolchain.toml     version de Rust épinglée
├─ pnpm-workspace.yaml
├─ crates/
│  ├─ perasc-syntax/       lexer, parser, AST, positions source
│  ├─ perasc-diag/         diagnostics, codes d'erreur, messages EN/FR
│  ├─ perasc-check/        noms, types, ownership, ressources, budget de pile
│  ├─ perasc-ir/           IR typée partagée par les deux backends
│  ├─ perasc-codegen-rs/   .peras → source Rust
│  ├─ perasc-codegen-js/   .apeiron → JavaScript + source maps
│  ├─ perasc-driver/       pipeline, config, appels cargo et wasm-bindgen
│  └─ perasc/              interface en ligne de commande
├─ std/                    bibliothèque standard écrite en .peras (raster, math)
├─ packages/
│  ├─ vite-plugin-peras/
│  ├─ vitest-peras/
│  └─ create-peras/
├─ tests/ui/               entrées .peras / .apeiron + diagnostics attendus
├─ docs/en/                documentation anglaise (référence)
├─ docs/fr/                documentation française
└─ .github/workflows/
```

Chaque crate ne dépend que des crates situées avant elle dans le pipeline. `perasc-diag` fait exception : toutes les crates l'utilisent.

## Choix techniques

Le compilateur est écrit en Rust et maîtrise chaque étape qui produit un diagnostic, car les erreurs détaillées sont un pilier du projet. Les crates existantes servent là où elles ne font que produire une sortie.

| Étape             | Choix                                                                                                                                                                                                                        | Raison                                                                                                                          |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Lexer             | `logos`                                                                                                                                                                                                                      | Lexer généré rapide, positions exactes pour chaque token                                                                        |
| Parser            | Descente récursive écrite à la main, Pratt pour les expressions                                                                                                                                                              | Contrôle total de la récupération d'erreurs et des messages ; la syntaxe s'éloigne de TS (`match`, `struct`, `inout`, `@frame`) |
| Stockage de l'AST | Arène + identifiants internés                                                                                                                                                                                                | IDs peu coûteux, aucune durée de vie qui fuit dans l'API                                                                        |
| Vérifications     | Noms et types en un seul parcours (un nom déjà signalé devient un type erreur, compatible avec tout), puis ownership → ressources → budget de pile ; les vérifications ne tournent que sur un fichier sans erreur de syntaxe | Une faute donne une erreur, et aucune vérification ne signale les conséquences d’un arbre cassé                                 |
| IR                | IR typée et simplifiée dans `perasc-ir`                                                                                                                                                                                      | Une seule entrée pour les deux backends, facile à tester en snapshot                                                            |
| Backend Rust      | Tokens émis, formatés avec `prettyplease`, compilés par cargo pour `wasm32-unknown-unknown`, puis `wasm-bindgen`                                                                                                             | Rust généré lisible, rustc comme filet de sécurité final                                                                        |
| Backend JS        | AST et codegen `oxc`                                                                                                                                                                                                         | Générateur JS et source maps éprouvés, inutile d'écrire le nôtre                                                                |
| Incrémentalité    | Requêtes conçues pour `salsa` dès le départ, `salsa` adopté à l'étape `--watch`                                                                                                                                              | Évite une réécriture à l'arrivée du LSP                                                                                         |
| Sûreté            | `#![forbid(unsafe_code)]` dans toutes les crates du compilateur                                                                                                                                                              | Le compilateur s'applique la règle qu'il impose                                                                                 |

Cela remplace l'idée du « parser oxc étendu » d'une version antérieure de la [spec](spec.md) : oxc reste pour la sortie JS, le parser est le nôtre.

## Moteur de diagnostics

`perasc-diag` est construit avant le parser, pour que la toute première erreur de syntaxe ait déjà son format final.

- **Un seul type de données** : un `Diagnostic` contient un code stable, une sévérité (toujours erreur), des positions annotées, une cause, l'historique de la valeur, une correction proposée sous forme de diff et un lien vers `perasc explain`.
- **Rendu** : notre propre moteur de rendu, au format exact de la [spec](spec.md#format-dun-diagnostic) : extrait avec plusieurs annotations, cause, correction en diff, `perasc explain`. `ariadne` était le premier choix, mais il code en dur des mots anglais (« Help », « Note ») et ne sait pas afficher une correction en diff. Les caractères invisibles et bidirectionnels d’un extrait s’affichent sous la forme `<U+202E>` : une erreur ne réordonne jamais le terminal.
- **JSON** : le même `Diagnostic` sérialisé avec `serde` pour `--json`, l'overlay Vite, la CI et le futur LSP.
- **Messages en deux langues** : les textes vivent dans des fichiers Project Fluent, `messages/en.ftl` et `messages/fr.ftl`. Le code ne référence que des identifiants de message, jamais de chaînes brutes.
- **Registre des codes d'erreur** : un fichier liste chaque code (`E0001`…), son identifiant de message et ses pages de doc dans `docs/en/errors/` et `docs/fr/errors/`.
- **Toutes les erreurs en une passe** : le parser se rétablit à chaque fin d'instruction, et chaque passe de vérification signale toutes ses erreurs, dans la limite de 50 par fichier.

## Contrôles qualité

Chaque pull request doit passer tous ces contrôles ; un seul échec bloque la fusion.

| Contrôle                | Outil                                                         | Bloque si                               |
| ----------------------- | ------------------------------------------------------------- | --------------------------------------- |
| Formatage               | `cargo fmt --check`, Prettier                                 | le moindre écart                        |
| Lints                   | `cargo clippy -- -D warnings`, ESLint                         | le moindre avertissement                |
| Tests unitaires et UI   | `cargo test`, snapshots `insta` sur `tests/ui/`               | un échec ou un snapshot non relu        |
| Tests de l'outillage JS | Vitest pour le plugin Vite                                    | un échec                                |
| Bout en bout            | build de la démo avec Vite, exécution headless                | erreur de build ou mauvais résultat     |
| Dépendances Rust        | `cargo audit`, `cargo deny`                                   | une CVE connue, une licence interdite   |
| Dépendances JS          | `pnpm audit`                                                  | une CVE connue                          |
| Traductions             | script comparant `en.ftl` et `fr.ftl`, `docs/en` et `docs/fr` | une clé ou une page absente en français |
| Fuzzing                 | `cargo-fuzz` sur le lexer et le parser, exécution nocturne    | un crash                                |

Les dépendances restent à leur dernière version : Renovate ouvre les PR de mise à jour, et les contrôles ci-dessus tranchent.

## Les deux premières semaines

Dix jours ouvrés, chacun se terminant sur quelque chose qui tourne ou qui est testé.

1. **Workspace et CI** : workspaces Cargo et pnpm, toolchain épinglée, tous les contrôles qualité branchés et verts sur un projet vide.
2. **Diagnostics** : type `Diagnostic`, registre des codes, rendu terminal, fichiers Fluent en anglais et en français, sortie `--json`.
3. **Lexer** : tokens `logos` avec positions, une erreur pour chaque caractère invalide, premiers tests en snapshot.
4. **Parser, partie 1** : fonctions, `let` et `const`, littéraux `i32` et `f64`, arithmétique en Pratt.
5. **Parser, partie 2** : `struct`, `Array`, `if`, `for`, `return`, récupération à chaque fin d'instruction, 30 tests UI.
6. **Noms et types** : portées, masquage interdit, erreur sur variable inutilisée, aucune conversion numérique implicite.
7. **IR et backend Rust** : abaissement du sous-ensemble vers l'IR, émission de Rust formaté, snapshot de la sortie.
8. **Driver** : `perasc build` appelle cargo pour `wasm32-unknown-unknown` puis `wasm-bindgen`, et signale toute erreur rustc comme un bug du compilateur.
9. **Plugin Vite** : `vite-plugin-peras` charge un fichier `.peras` comme module WASM ; `fib` tourne dans le navigateur.
10. **Consolidation et release** : fuzzing nocturne, doc de chaque code émis jusque-là en anglais puis en français, tag `v0.0.1`.

Critères de sortie : `fib` tourne dans le navigateur, tous les contrôles sont verts, et chaque erreur de syntaxe affiche extrait, cause et correction dans les deux langues.

## Règles de travail

- **Anglais d'abord, puis français** : l'anglais fait foi pour les commentaires de code, la documentation, les messages et les messages de commit. Tout texte visible par l'utilisateur arrive en anglais et en français dans la même pull request ; le contrôle des traductions l'impose.
- **Les codes d'erreur sont permanents** : un code n'est jamais réutilisé ni renuméroté. Un code retiré garde sa page de doc, marquée comme retirée.
- **Chaque nouvelle erreur arrive complète** : code, messages anglais et français, correction proposée, test UI et page `perasc explain`.
- **Conventional Commits** (`feat:`, `fix:`, `docs:`…) pour générer le changelog.
- **Versionnage** : semver, `0.x` tant que le langage n'est pas stable ; tout changement cassant figure dans le changelog.
- **Releases** : binaires `perasc` précompilés par plateforme, publiés sur npm en dépendances optionnelles, comme le font esbuild et oxc.

## Décisions ouvertes

- [ ] Licence : MIT, Apache-2.0, ou les deux comme dans l'écosystème Rust ?
- [ ] Version minimale de Rust pour les contributeurs.
- [ ] Hébergement : nom de l'organisation GitHub, une fois npm, crates.io et le domaine réservés.
- [ ] Faut-il adopter `salsa` avant l'étape `--watch` ?
