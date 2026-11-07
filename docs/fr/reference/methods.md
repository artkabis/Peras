# Méthodes intégrées

Chaque méthode que fournit le langage, par type. Cette page est vérifiée face au compilateur : une méthode absente d’ici, ou listée ici mais inconnue du compilateur, fait échouer les tests.

## Nombres

### Conversions

| Méthode                                   | Sur         | Donne                            | État                                         |
| ----------------------------------------- | ----------- | -------------------------------- | -------------------------------------------- |
| `toI32()`, `toI64()`                      | tout nombre | la valeur dans ce type entier    | <span class="status compiled">compilé</span> |
| `toF32()`, `toF64()`                      | tout nombre | la valeur dans ce type flottant  | <span class="status compiled">compilé</span> |
| `toI8()`, `toI16()`                       | tout nombre | la valeur dans ce type entier    | <span class="status checked">vérifié</span>  |
| `toU8()`, `toU16()`, `toU32()`, `toU64()` | tout nombre | la valeur dans ce type non signé | <span class="status checked">vérifié</span>  |

Depuis un flottant, les conversions arrondissent vers zéro et saturent ; entre entiers, elles gardent les bits de poids faible.

### Arithmétique

| Méthode                                              | Sur         | Donne                                                  | État                                         |
| ---------------------------------------------------- | ----------- | ------------------------------------------------------ | -------------------------------------------- |
| `abs()`                                              | tout nombre | la valeur absolue ; reboucle pour le plus petit entier | <span class="status compiled">compilé</span> |
| `wrappingAdd(b)`, `wrappingSub(b)`, `wrappingMul(b)` | entiers     | le résultat rebouclé, comme `+`, `-`, `*`              | <span class="status compiled">compilé</span> |
| `checkedAdd(b)`, `checkedSub(b)`, `checkedMul(b)`    | entiers     | `Some(résultat)`, ou `None` en cas de dépassement      | <span class="status compiled">compilé</span> |

```peras build
export fn wrapped(a: i32): i32 {
  return a.wrappingMul(3).abs();
}
```

### Flottants

| Méthode                   | Donne                                     | État                                         |
| ------------------------- | ----------------------------------------- | -------------------------------------------- |
| `sqrt()`                  | la racine carrée ; NaN en dessous de zéro | <span class="status compiled">compilé</span> |
| `floor()`, `ceil()`       | arrondi vers le bas, vers le haut         | <span class="status compiled">compilé</span> |
| `round()`                 | arrondi au plus proche                    | <span class="status checked">vérifié</span>  |
| `sin()`, `cos()`, `tan()` | trigonométrie, en radians                 | <span class="status checked">vérifié</span>  |

## `Array<T>`

| Membre    | Donne                                              | État                                         |
| --------- | -------------------------------------------------- | -------------------------------------------- |
| `length`  | le nombre d’éléments, un `i32`                     | <span class="status compiled">compilé</span> |
| `push(x)` | ajoute `x` à la fin                                | <span class="status compiled">compilé</span> |
| `pop()`   | retire et donne le dernier élément, `None` si vide | <span class="status compiled">compilé</span> |
| `copy()`  | un nouveau tableau avec les mêmes éléments         | <span class="status compiled">compilé</span> |

## `String`

| Membre   | Donne                                  | État                                         |
| -------- | -------------------------------------- | -------------------------------------------- |
| `length` | la longueur                            | <span class="status compiled">compilé</span> |
| `copy()` | une nouvelle chaîne avec le même texte | <span class="status compiled">compilé</span> |

## `Option<T>`

| Méthode                | Donne                      | État                                         |
| ---------------------- | -------------------------- | -------------------------------------------- |
| `isSome()`, `isNone()` | si une valeur est présente | <span class="status compiled">compilé</span> |
| `unwrapOr(secours)`    | la valeur, ou `secours`    | <span class="status compiled">compilé</span> |

## `Result<T, E>`

| Méthode             | Donne                             | État                                         |
| ------------------- | --------------------------------- | -------------------------------------------- |
| `isOk()`, `isErr()` | la variante                       | <span class="status compiled">compilé</span> |
| `unwrapOr(secours)` | la valeur de l’`Ok`, ou `secours` | <span class="status compiled">compilé</span> |

Un membre inconnu est refusé avec la liste de ceux qui existent ([E0215](../errors/E0215.md)). Beaucoup d’autres méthodes sont prévues : voir la [feuille de route](roadmap.md#phase-3--une-bibliothèque-standard).
