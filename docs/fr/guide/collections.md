# Tableaux, chaînes et options

Les options, les tableaux et les chaînes sont compilés. Tableaux et chaînes vivent sur le tas, avec un seul propriétaire chacun : la page [mémoire](memory.md) en donne les règles.

## `Array<T>`

Une liste extensible de valeurs d’un même type, écrite `[a, b, c]`. Ses éléments vivent sur le tas, dans la mémoire du module. <span class="status compiled">compilé</span>

```peras build
export fn evens(n: i32): Array<i32> {
  let found: Array<i32> = [];
  for (let i = 0; i < n; i++) {
    if (i % 2 == 0) {
      found.push(i);
    }
  }
  return found;
}

export fn first(xs: Array<i32>): i32 {
  if (xs.length == 0) {
    return -1;
  }
  return xs[0];
}

export fn scale(xs: inout Array<f64>, k: f64): void {
  for (let i = 0; i < xs.length; i++) {
    xs[i] = xs[i] * k;
  }
}
```

| Membre       | Donne                                                         |
| ------------ | ------------------------------------------------------------- |
| `xs.length`  | le nombre d’éléments, un `i32`                                |
| `xs.push(x)` | ajoute `x` à la fin                                           |
| `xs.pop()`   | retire le dernier élément : `Option<T>`, `None` si vide       |
| `xs[i]`      | l’élément à l’indice `i`                                      |
| `xs[i] = v`  | le remplace ; `+=` et les autres, `++` et `--` marchent aussi |
| `xs.copy()`  | un nouveau tableau avec les mêmes éléments                    |

**Les indices sont vérifiés.** Un indice hors de `0` à `xs.length - 1` arrête le programme avec une `RangeError`, avant de toucher la mémoire. `push` fait de la place au besoin, en la doublant, si bien qu’ajouter des éléments un par un coûte un temps constant chacun, en moyenne.

Aujourd’hui, un tableau contient des nombres ou des booléens : `i32`, `i64`, `f32`, `f64`, `bool`. Les tableaux de structures, de chaînes ou de tableaux sont vérifiés, pas encore compilés.

### Un propriétaire par tableau

Un tableau a un seul propriétaire. `let ys = xs;`, un `return` ou un argument passé à un paramètre `sink` le **déplace** : `xs` ne peut plus servir ([E0204](../errors/E0204.md)) tant qu’il ne reçoit pas une nouvelle valeur. Pour garder les deux, déplace une copie, `xs.copy()`. Le compilateur libère chaque tableau quand la portée de son propriétaire se termine ; tu n’écris jamais `free`. Les règles sont sur la page [mémoire](memory.md).

```peras build
fn total(xs: sink Array<i32>): i32 {
  let sum = 0;
  for (const x of xs) {
    sum += x;
  }
  return sum;
}

export fn twice(): i32 {
  let xs = [1, 2, 3];
  const first = total(xs.copy());
  return first + total(xs);
}
```

### En JavaScript

Un tableau passe la frontière par copie : un tableau JavaScript entre, un nouveau tableau JavaScript sort. Un paramètre `inout` est recopié dans le tableau passé.

```ts
import { evens, scale } from "./numbers.peras";

evens(7); // [0, 2, 4, 6]
const xs = [1, 2, 3];
scale(xs, 2); // xs vaut maintenant [2, 4, 6]
```

TypeScript voit `number[]`, `bigint[]` pour `i64` et `boolean[]` pour `bool` ; un paramètre que la fonction ne fait que lire est `readonly number[]`.

## `String`

Du texte en UTF-8, écrit entre guillemets doubles avec les échappements habituels (`\n`, `\t`, `\"`, `\\`, `\u{1F600}`). Une chaîne vit sur le tas, comme un tableau, et suit les mêmes règles : un seul propriétaire, déplacée par l’affectation, libérée par le compilateur. <span class="status compiled">compilé</span>

```peras build
export fn greeting(name: String): String {
  return "Hello, " + name;
}

export fn is_empty(s: String): bool {
  return s.length == 0;
}

export fn repeat(s: String, n: i32): String {
  let out = "";
  for (let i = 0; i < n; i++) {
    out += s;
  }
  return out;
}
```

| Opération      | Donne                                  |
| -------------- | -------------------------------------- |
| `a + b`        | une nouvelle chaîne, `a` puis `b`      |
| `s += t`       | ajoute `t` à la fin de `s`, sur place  |
| `a == b`, `!=` | si les deux contiennent le même texte  |
| `s.length`     | le nombre d’**octets** en UTF-8        |
| `s.copy()`     | une nouvelle chaîne avec le même texte |

**`length` compte des octets**, comme en Rust et comme la mémoire qu’occupe la chaîne : `"é".length` vaut 2, un emoji 4. La `length` de JavaScript compte plutôt des unités UTF-16 ; les méthodes sur les caractères arrivent avec la [bibliothèque standard](../reference/roadmap.md#phase-3--une-bibliothèque-standard).

Un littéral est stocké une seule fois dans le module, et copié dans un nouveau buffer là où il sert. En JavaScript, une chaîne passe la frontière par copie, encodée et décodée en UTF-8 ; TypeScript voit `string`. Une fonction exportée ne peut pas prendre une chaîne `inout`, puisqu’une chaîne JavaScript ne change jamais.

Les caractères invisibles qui peuvent réordonner ce que vous lisez, l’attaque « Trojan Source », sont refusés dans le code source ([E0007](../errors/E0007.md)).

## `Option<T>`

Une valeur qui peut être absente : `Some(valeur)` ou `None`. Peras n’a pas de `null` : l’absence est dans le type, et le compilateur oblige à la traiter. <span class="status compiled">compilé</span>

```peras build
export fn half(n: i32): Option<i32> {
  if (n % 2 != 0) {
    return None;
  }
  return Some(n / 2);
}

export fn quarter(n: i32): Option<i32> {
  const h = half(n)?;
  return half(h);
}

export fn half_or(n: i32, fallback: i32): i32 {
  return half(n).unwrapOr(fallback);
}
```

| Méthode ou opérateur       | Donne                                                                       |
| -------------------------- | --------------------------------------------------------------------------- |
| `o.isSome()`, `o.isNone()` | si une valeur est présente                                                  |
| `o.unwrapOr(secours)`      | la valeur, ou `secours` si elle est absente ; `secours` est toujours évalué |
| `o?`                       | la valeur ; si elle est absente, la fonction renvoie `None` aussitôt        |

`?` est permis dans une fonction qui renvoie une option. L’arithmétique vérifiée renvoie aussi une option : `a.checkedAdd(b)` vaut `None` en cas de dépassement, là où `a + b` reboucle.

```peras build
export fn total(a: i32, b: i32, c: i32): Option<i32> {
  const ab = a.checkedAdd(b)?;
  return ab.checkedAdd(c);
}
```

Une option contient n’importe quel type compilé, structure comprise. Elle ne coûte aucune allocation : c’est un `bool` suivi de la valeur. JavaScript voit la valeur, ou `undefined` :

```ts
import { half } from "./maths.peras";

half(10); // 5
half(7); // undefined
```

La déclaration TypeScript le dit : `half(n: number): number | undefined`.

> Un ensemble complet de méthodes sur les chaînes et les tableaux (`split`, `slice`, `map`, `filter`…) est prévu : voir la [feuille de route](../reference/roadmap.md).

Ensuite : [la mémoire](memory.md).
