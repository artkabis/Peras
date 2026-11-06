# Tableaux, chaînes et options

Ces trois types font partie du langage et sont entièrement vérifiés, mais pas encore compilés : `perasc check` les accepte, `perasc build` signale [E0900](../errors/E0900.md). Les tableaux viennent ensuite, avec le tas.

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

Une valeur qui peut être absente : `Some(valeur)` ou `None`. Peras n’a pas de `null` : l’absence est dans le type, et le compilateur oblige à la traiter.

```peras
export fn last_or(xs: Array<i32>, fallback: i32): i32 {
  let copy = xs;
  return copy.pop().unwrapOr(fallback);
}

export fn half(n: i32): Option<i32> {
  if (n % 2 != 0) {
    return None;
  }
  return Some(n / 2);
}
```

| Méthode                    | Donne                                       |
| -------------------------- | ------------------------------------------- |
| `o.isSome()`, `o.isNone()` | si une valeur est présente                  |
| `o.unwrapOr(secours)`      | la valeur, ou `secours` si elle est absente |

L’arithmétique vérifiée renvoie aussi une option : `a.checkedAdd(b)` vaut `None` en cas de dépassement.

> Un ensemble complet de méthodes sur les chaînes et les tableaux (`split`, `slice`, `map`, `filter`…) est prévu : voir la [feuille de route](../reference/roadmap.md).

Ensuite : [la mémoire](memory.md).
