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
| `min(b)`, `max(b)`                                   | tout nombre | le plus petit, le plus grand                           | <span class="status compiled">compilé</span> |
| `clamp(lo, hi)`                                      | tout nombre | la valeur, relevée à `lo`, abaissée à `hi`             | <span class="status compiled">compilé</span> |

```peras build
export fn wrapped(a: i32): i32 {
  return a.wrappingMul(3).abs();
}
```

### En texte

| Méthode      | Sur                  | Donne                                                                         | État                                         |
| ------------ | -------------------- | ----------------------------------------------------------------------------- | -------------------------------------------- |
| `toString()` | entiers, et booléens | les chiffres décimaux, après `-` si négatif ; `true` ou `false`, une `String` | <span class="status compiled">compilé</span> |

Le texte est celui qu’écrit `String()` en JavaScript pour la même valeur. Les flottants viennent ensuite : leur forme exacte la plus courte demande un algorithme à elle.

```peras build
export fn label(count: i64, done: bool): String {
  return count.toString() + " items, done: " + done.toString();
}
```

### Flottants

| Méthode                   | Donne                                                                | État                                         |
| ------------------------- | -------------------------------------------------------------------- | -------------------------------------------- |
| `sqrt()`                  | la racine carrée ; NaN en dessous de zéro                            | <span class="status compiled">compilé</span> |
| `floor()`, `ceil()`       | arrondi vers le bas, vers le haut                                    | <span class="status compiled">compilé</span> |
| `round()`                 | arrondi au plus proche, les moitiés vers le haut, comme `Math.round` | <span class="status compiled">compilé</span> |
| `sin()`, `cos()`, `tan()` | trigonométrie, en radians                                            | <span class="status compiled">compilé</span> |
| `exp()`, `log()`          | `e` à la puissance de la valeur, le logarithme naturel               | <span class="status compiled">compilé</span> |
| `pow(b)`                  | la valeur à la puissance `b`                                         | <span class="status compiled">compilé</span> |
| `atan2(x)`                | sur `y`, l’angle du point `(x, y)`, en radians                       | <span class="status compiled">compilé</span> |
| `hypot(b)`                | `sqrt(a * a + b * b)`                                                | <span class="status compiled">compilé</span> |

WebAssembly n’a pas d’instruction pour `round`, `exp`, `log`, la trigonométrie, `pow` et `atan2` : un module qui s’en sert les importe du `Math` de JavaScript, et ils donnent donc exactement ce que donne JavaScript. Un `f32` est calculé comme un `f64`, puis rearrondi.

## `Array<T>`

| Membre    | Donne                                              | État                                         |
| --------- | -------------------------------------------------- | -------------------------------------------- |
| `length`  | le nombre d’éléments, un `i32`                     | <span class="status compiled">compilé</span> |
| `push(x)` | ajoute `x` à la fin                                | <span class="status compiled">compilé</span> |
| `pop()`   | retire et donne le dernier élément, `None` si vide | <span class="status compiled">compilé</span> |
| `copy()`  | un nouveau tableau avec les mêmes éléments         | <span class="status compiled">compilé</span> |

### Ordre, morceaux et recherche

| Méthode             | Donne                                                                                                           | Statut                                       |
| ------------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `slice(start, end)` | un nouveau tableau des éléments de `start` à `end` ; un négatif compte depuis la fin, au-delà est ramené dedans | <span class="status compiled">compilé</span> |
| `concat(other)`     | un nouveau tableau de ses éléments, puis de ceux de `other`                                                     | <span class="status compiled">compilé</span> |
| `reverse()`         | renverse les éléments, sur place : le tableau est un `let`                                                      | <span class="status compiled">compilé</span> |
| `sort()`            | trie des nombres par ordre croissant, sur place, en temps n log n, sans allouer                                 | <span class="status compiled">compilé</span> |
| `includes(x)`       | si un élément vaut `x` ; nombres, booléens et chaînes                                                           | <span class="status compiled">compilé</span> |
| `indexOf(x)`        | l’indice du premier élément égal à `x`, ou `-1`                                                                 | <span class="status compiled">compilé</span> |

```peras build
export fn median(samples: Array<f64>): f64 {
  let sorted = samples.copy();
  sorted.sort();
  return sorted[sorted.length / 2];
}
```

### Avec une fonction fléchée

Chacune de ces méthodes prend une fonction fléchée, écrite sur place, `(x) => …`, et la compile comme le corps d’une boucle : aucune valeur de fonction, aucun appel à l’exécution. Ses paramètres prêtent les éléments, comme `for…of` : ils sont lus, jamais déplacés. <span class="status compiled">compilé</span>

| Méthode               | Donne                                                                                  | Statut                                       |
| --------------------- | -------------------------------------------------------------------------------------- | -------------------------------------------- |
| `map(f)`              | un nouveau tableau des `f(x)` de chaque élément                                        | <span class="status compiled">compilé</span> |
| `filter(f)`           | un nouveau tableau des copies des éléments pour lesquels `f(x)` vaut `true`            | <span class="status compiled">compilé</span> |
| `reduce(f, start)`    | `f(acc, x)` replié sur les éléments, depuis `start` ; nombres et structures de nombres | <span class="status compiled">compilé</span> |
| `find(f)`             | une copie du premier élément pour lequel `f(x)` vaut `true`, ou `None`                 | <span class="status compiled">compilé</span> |
| `findIndex(f)`        | l’indice du premier élément pour lequel `f(x)` vaut `true`, ou `-1`                    | <span class="status compiled">compilé</span> |
| `some(f)`, `every(f)` | si `f(x)` vaut `true` pour un élément, pour tous ; s’arrête dès que c’est connu        | <span class="status compiled">compilé</span> |

```peras build
export fn summary(xs: Array<i32>, words: Array<String>): i32 {
  const evens = xs.filter((x) => x % 2 == 0);
  const total = evens.map((x) => x * 10).reduce((acc, x) => acc + x, 0);
  const long = words.find((w) => w.length > 3).unwrapOr("");
  let n = total + long.length + xs.findIndex((x) => x > 100);
  if (xs.some((x) => x < 0)) {
    n += 1;
  }
  return n;
}
```

## `String`

| Membre   | Donne                                  | État                                         |
| -------- | -------------------------------------- | -------------------------------------------- |
| `length` | la longueur                            | <span class="status compiled">compilé</span> |
| `copy()` | une nouvelle chaîne avec le même texte | <span class="status compiled">compilé</span> |

Les indices et les longueurs comptent les octets UTF-8, comme `length`. Un indice tombé au milieu d’un caractère recule à son début : une tranche est toujours un texte valide. Chaque méthode qui donne une chaîne en donne une nouvelle ; la chaîne sur laquelle on l’appelle ne change pas.

| Méthode                                       | Donne                                                                                                 | Statut                                       |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------- |
| `indexOf(s)`                                  | l’indice en octets du premier `s`, ou `-1`                                                            | <span class="status compiled">compilé</span> |
| `includes(s)`, `startsWith(s)`, `endsWith(s)` | si `s` s’y trouve, au début, à la fin                                                                 | <span class="status compiled">compilé</span> |
| `slice(start, end)`                           | les octets de `start` à `end` ; un négatif compte depuis la fin, au-delà des bornes est ramené dedans | <span class="status compiled">compilé</span> |
| `trim()`                                      | sans les espaces, tabulations et retours à la ligne aux deux bouts                                    | <span class="status compiled">compilé</span> |
| `toUpperCase()`, `toLowerCase()`              | les lettres ASCII en majuscules ou en minuscules ; les autres caractères inchangés                    | <span class="status compiled">compilé</span> |
| `repeat(n)`                                   | le texte `n` fois                                                                                     | <span class="status compiled">compilé</span> |
| `replace(from, to)`, `replaceAll(from, to)`   | le premier `from`, ou tous, remplacés par `to`                                                        | <span class="status compiled">compilé</span> |
| `split(separator)`                            | les morceaux entre chaque séparateur, un `Array<String>` ; avec `""`, les caractères                  | <span class="status compiled">compilé</span> |
| `parseI32()`, `parseI64()`                    | `Some(n)` quand le texte est un entier dans l’intervalle, sinon `None` ; voir plus bas                | <span class="status compiled">compilé</span> |

```peras build
export fn tidy(line: String): String {
  const fields = line.split(",").map((f) => f.trim().toLowerCase());
  const named = fields.filter((f) => !f.startsWith("#"));
  return named.join(" ").replaceAll("  ", " ");
}
```

`parseI32()` et `parseI64()` sont strictes : le texte est un `+`, un `-` ou rien, puis des chiffres ASCII, et rien d’autre. Ni espace, ni `_`, ni `0x`, ni exposant ; une valeur hors de l’intervalle du type donne `None`, jamais un résultat tronqué. Appliquez `trim()` d’abord si le texte peut porter des espaces.

```peras build
export fn port(text: String): i32 {
  const n = text.trim().parseI32().unwrapOr(-1);
  return n.clamp(-1, 65535);
}
```

Sur un `Array<String>`, `join(separator)` donne ses chaînes avec `separator` entre chaque paire. <span class="status compiled">compilé</span>

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
