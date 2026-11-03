# Contribuer à Peras

*English version: [CONTRIBUTING.md](CONTRIBUTING.md), qui fait foi.*

## Anglais d'abord, puis français

L'anglais fait foi pour les commentaires de code, la documentation, les messages du compilateur et les messages de commit. Tout texte visible par l'utilisateur arrive en anglais **et** en français dans la même pull request :

- messages du compilateur : `crates/*/messages/en.ftl` et `fr.ftl`, mêmes clés ;
- documentation : `docs/en/<page>` et `docs/fr/<page>`, mêmes chemins ;
- fichiers racine : `README.md` / `README.fr.md`, `CONTRIBUTING.md` / `CONTRIBUTING.fr.md`.

`node scripts/check-i18n.mjs` le vérifie en local ; sinon la CI bloque la fusion.

## Tolérance zéro

- Aucun avertissement : `cargo clippy -- -D warnings`, `eslint --max-warnings 0`.
- Aucun `unsafe` : chaque crate hérite de `unsafe_code = "forbid"`.
- Aucune CVE connue : `cargo deny`, `cargo audit` et `pnpm audit` bloquent la fusion.
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
5. sa page dans `docs/en/errors/` puis `docs/fr/errors/`, utilisée par `perasc explain`.

## Commits et pull requests

Les titres de pull request suivent [Conventional Commits](https://www.conventionalcommits.org) : `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Le changelog en est généré. Les versions suivent semver et restent en `0.x` tant que le langage n'est pas stable.

## Dépendances

Les dépendances restent à leur dernière version. Renovate ouvre les pull requests de mise à jour ; la CI tranche. Une montée de version du toolchain (`rust-toolchain.toml`) a sa propre pull request.
