# Contribuer à Peras

_English version: [CONTRIBUTING.md](CONTRIBUTING.md), qui fait foi._

## Anglais d'abord, puis français

L'anglais fait foi pour les commentaires de code, la documentation, les messages du compilateur et les messages de commit. Tout texte visible par l'utilisateur arrive en anglais **et** en français dans la même pull request :

- messages du compilateur : `crates/*/messages/en.ftl` et `fr.ftl`, mêmes clés ;
- documentation : `docs/en/<page>` et `docs/fr/<page>`, mêmes chemins ;
- fichiers racine : `README.md` / `README.fr.md`, `CONTRIBUTING.md` / `CONTRIBUTING.fr.md`, `RUNTIME-EXCEPTION.md` / `RUNTIME-EXCEPTION.fr.md` (le texte anglais est celui qui s’applique).

`node scripts/check-i18n.mjs` le vérifie en local ; sinon la CI bloque la fusion.

## Tolérance zéro

- Aucun avertissement : `cargo clippy -- -D warnings`, `eslint --max-warnings 0`.
- Aucun `unsafe` : chaque crate hérite de `unsafe_code = "forbid"`.
- Aucune CVE connue : `cargo deny` (avis RustSec) et `pnpm audit` bloquent la fusion.
- Aucun snapshot non relu : `cargo insta review` avant de pousser.

Tout lancer en local avant de pousser :

```sh
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm run check
```

## Codes d'erreur

Une nouvelle erreur du compilateur arrive complète, dans une seule pull request :

1. un nouveau code dans le registre (`E` + quatre chiffres), sans jamais réutiliser un code retiré ;
2. son message dans `en.ftl` puis `fr.ftl` ;
3. une correction proposée ;
4. un test UI dans `tests/ui/` ;
5. sa page dans `docs/en/errors/` puis `docs/fr/errors/`, utilisée par `perasc explain`. Un test compile l’exemple de chaque page, qui doit produire le code, et sa correction, qui ne le doit pas. Les fragments sont permis : `…` remplace n’importe quoi, et les instructions sont essayées dans une fonction. Écris les caractères invisibles sous la forme `<U+202E>`.

## Fuzzing

`fuzz/` contient trois cibles [cargo-fuzz](https://rust-fuzz.github.io/book/cargo-fuzz.html), lancées 30 minutes chaque nuit par la CI :

- `lex` : le lexer ne panique jamais, et ses tokens restent dans l’ordre à l’intérieur de la source ;
- `parse` : le parser ne panique jamais et ne fait jamais déborder sa pile ;
- `pipeline` : chaque diagnostic s’affiche dans les deux langues, et une source sans erreur se compile en un module WebAssembly valide, jamais en bug du compilateur.

```sh
rustup toolchain install nightly
cargo install --locked cargo-fuzz
sh fuzz/seed.sh   # toutes les sources Peras du dépôt comme corpus de départ
cd fuzz && cargo +nightly fuzz run pipeline corpus/pipeline seeds -- -max_total_time=600
```

Un crash laisse son entrée dans `fuzz/artifacts/<cible>/`. Corrige-le avec un test de régression qui utilise cette entrée, dans le crate qui a paniqué.

## Licence des contributions

Peras est sous AGPL-3.0-or-later avec l’[exception de runtime de Peras](RUNTIME-EXCEPTION.fr.md) ([décision 0006](docs/fr/decisions/0006-licensing.md)). En ouvrant une pull request, vous confirmez avoir le droit d’en soumettre le contenu, et vous acceptez que :

- votre contribution soit sous AGPL-3.0-or-later avec l’exception de runtime de Peras ;
- vous accordiez aussi au propriétaire du projet une licence perpétuelle, mondiale, non exclusive et gratuite pour utiliser, modifier et licencier votre contribution à d’autres conditions, commerciales comprises, afin que Peras puisse être proposé sous licence commerciale à qui en a besoin ;
- vous gardiez le droit d’auteur sur votre contribution.

Ajoutez une ligne `Signed-off-by:` à vos commits (`git commit -s`) pour en garder la trace.

## Commits et pull requests

Les titres de pull request suivent [Conventional Commits](https://www.conventionalcommits.org) : `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Le changelog est rédigé à partir d’eux. Les versions suivent semver et restent en `0.x` tant que le langage n'est pas stable.

## Dépendances

Les dépendances restent à leur dernière version stable, dès qu'une version a au moins 24 heures (`minimumReleaseAge`) : les paquets compromis sont en général retirés dans ce délai. N'ajoute jamais d'entrée `minimumReleaseAgeExclude` ; un correctif de sécurité arrive tout de même immédiatement via les alertes de vulnérabilité de Renovate. Les GitHub Actions sont épinglées sur un SHA de commit. Renovate ouvre les pull requests de mise à jour ; la CI tranche. Une montée de version du toolchain (`rust-toolchain.toml`) a sa propre pull request.
