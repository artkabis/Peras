# Opérateurs

## Par priorité

Du plus prioritaire au moins prioritaire. Les opérateurs d’une même ligne ont la même priorité et se groupent de gauche à droite.

| Opérateurs        | Sens                                                 |
| ----------------- | ---------------------------------------------------- |
| `-x` `!x` `~x`    | négation, non logique, non bit à bit                 |
| `*` `/` `%`       | multiplication, division, reste                      |
| `+` `-`           | addition, soustraction ; `+` joint aussi les chaînes |
| `<<` `>>`         | décalages ; `>>` garde le signe d’un entier signé    |
| `<` `<=` `>` `>=` | comparaison                                          |
| `==` `!=`         | égalité                                              |
| `&`               | et bit à bit                                         |
| `^`               | ou exclusif bit à bit                                |
| `\|`              | ou bit à bit                                         |
| `&&`              | et logique, court-circuit                            |
| `\|\|`            | ou logique, court-circuit                            |

Les deux opérandes ont toujours le même type : il n’y a pas de conversion implicite ([E0210](../errors/E0210.md)). Un opérateur qui ne s’applique pas à ses opérandes, comme `&&` sur des nombres, est refusé ([E0221](../errors/E0221.md)).

## Ce qui diffère de JavaScript

| JavaScript                  | Peras                                                                         |
| --------------------------- | ----------------------------------------------------------------------------- |
| `===`, `!==`                | n’existent pas : `==` et `!=` sont déjà stricts ([E0006](../errors/E0006.md)) |
| `a < b < c`                 | refusé : écrire `a < b && b < c` ([E0108](../errors/E0108.md))                |
| `a ? b : c`                 | n’existe pas : utiliser `if`                                                  |
| `x++` dans une expression   | `++` et `--` sont des instructions seulement                                  |
| `x = y` dans une expression | l’affectation est une instruction seulement                                   |

## Affectation

`=`, `+=`, `-=`, `*=`, `/=`, `%=`, `++`, `--` sont des instructions. Leur cible est une variable `let`, ou un champ de celle-ci ([E0205](../errors/E0205.md)).

## Emprunt

`&x` passe une variable à un paramètre `inout`, pour que la modification soit visible à l’appel. <span class="status checked">vérifié</span>
