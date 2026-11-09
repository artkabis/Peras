# La commande `perasc`

```text
perasc <commande> [options]
```

## Commandes

| Commande                   | Fait                                                           |
| -------------------------- | -------------------------------------------------------------- |
| `perasc check <fichiers…>` | vérifie chaque fichier et signale chaque erreur, sans compiler |
| `perasc build <fichier>`   | vérifie, puis compile un fichier `.peras` en WebAssembly       |
| `perasc explain <code>`    | affiche la page d’un code d’erreur, comme `E0006`              |
| `perasc --version`, `-V`   | affiche la version                                             |
| `perasc --help`, `-h`      | affiche l’aide, dans votre langue                              |

## Options

| Option                        | S’applique à     | Sens                                                                                                                                                                                           |
| ----------------------------- | ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--lang en\|fr`               | toutes           | la langue des messages ; par défaut : la variable `PERAS_LANG`, puis la langue du système                                                                                                      |
| `--json`                      | `check`, `build` | les diagnostics en JSON sur la sortie standard                                                                                                                                                 |
| `--color auto\|always\|never` | `check`, `build` | les couleurs des diagnostics ; `auto` (par défaut) colore un terminal, et la variable `NO_COLOR` les désactive                                                                                 |
| `--out <dossier>`             | `build`          | où écrire les fichiers ; par défaut : `dist`                                                                                                                                                   |
| `--backend direct\|rust`      | `build`          | le générateur de code ; `rust` demande une toolchain Rust et sert aux tests du projet                                                                                                          |
| `--no-optimize`               | `build`          | saute les optimisations de l’IR                                                                                                                                                                |
| `--allocator debug\|tlsf`     | `build`          | l’allocateur d’un module qui utilise le tas : `debug`, par défaut, détecte chaque bug mémoire ; `tlsf` alloue en temps borné, pour la production ([0002](../decisions/0002-tlsf-allocator.md)) |

## Codes de sortie

| Code | Signifie                                     |
| ---- | -------------------------------------------- |
| `0`  | succès                                       |
| `1`  | la source contient des erreurs               |
| `2`  | un problème d’usage, de fichier ou d’outil   |
| `3`  | un bug du compilateur : merci de le signaler |

## Exemples

```sh
perasc check src/*.peras --lang fr
perasc build src/physics.peras --out public/wasm
perasc check src/app.peras --json > diagnostics.json
perasc explain E0108
```
