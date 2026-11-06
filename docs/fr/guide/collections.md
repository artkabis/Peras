# Tableaux, chaînes et options

Les options sont compilées. Les tableaux et les chaînes font partie du langage et sont entièrement vérifiés, mais pas encore compilés : `perasc check` les accepte, `perasc build` signale [E0900](../errors/E0900.md). Ils viennent ensuite, avec le tas.

## `Array<T>`

Une liste extensible de valeurs d’un même type, écrite `[a, b, c]` :

```peras
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
```

| Membre       | Donne                                                   |
| ------------ | ------------------------------------------------------- |
| `xs.length`  | le nombre d’éléments, un `i32`                          |
| `xs.push(x)` | ajoute `x` à la fin                                     |
| `xs.pop()`   | retire le dernier élément : `Option<T>`, `None` si vide |
| `xs[i]`      | l’élément à l’indice `i`                                |

## `String`

Du texte en UTF-8, écrit entre guillemets doubles avec les échappements habituels (`\n`, `\t`, `\"`, `\\`, `\u{1F600}`) :

```peras
export fn greeting(name: String): String {
  return "Hello, " + name;
}

export fn is_empty(s: String): bool {
  return s.length == 0;
}
```

`+` joint deux chaînes, `==` et `!=` les comparent, `s.length` donne la longueur. Le fait qu’une longueur compte des octets ou des caractères sera fixé quand les chaînes seront compilées.

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

> `pop()` sur un tableau renvoie une option ; il sera compilé avec les tableaux.

> Un ensemble complet de méthodes sur les chaînes et les tableaux (`split`, `slice`, `map`, `filter`…) est prévu : voir la [feuille de route](../reference/roadmap.md).

Ensuite : [la mémoire](memory.md).
