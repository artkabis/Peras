# Énumérations, match et résultats

Une énumération est une valeur qui est l’une de plusieurs variantes, chacune avec ses données. `match` la décompose, et le compilateur vérifie que chaque variante est traitée : en oublier une est une erreur, pas un bug découvert en production. <span class="status compiled">compilé</span>

## Déclarer une énumération

```peras build
export enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn unit(): Shape {
  return Shape.Rect(1.0, 1.0);
}

export fn nothing(): Shape {
  return Shape.Empty;
}
```

Une valeur s’écrit `Nom.Variante(données)`, ou `Nom.Variante` pour une variante sans données. Les données peuvent être de tout type : nombres, chaînes, tableaux, structures, options, autres énumérations.

## `match`

```peras build
enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn area(s: Shape): f64 {
  return match (s) {
    Circle(r) => 3.14159 * r * r,
    Rect(w, h) => w * h,
    Empty => 0.0,
  };
}
```

Chaque bras est un motif, `=>`, puis une valeur ou un bloc. Les bras sont essayés dans l’ordre, et le premier qui correspond s’exécute.

| Motif                          | Correspond à                                                                     |
| ------------------------------ | -------------------------------------------------------------------------------- |
| `_`                            | tout                                                                             |
| un nom, `x`                    | tout, appelé `x` dans le bras ; une variante sans données, si l’une porte ce nom |
| `0`, `-1`, `true`              | ce nombre, ce booléen                                                            |
| `Circle(r)`, `Shape.Circle(r)` | cette variante ; `r` nomme ses données                                           |
| `Some(0)`, `Rect(w, _)`        | les motifs s’imbriquent : la variante, quand ses données correspondent aussi     |

`match` marche sur les énumérations, les options, les booléens et les entiers ; sur toute autre valeur, seuls `_` et les noms s’appliquent.

## Chaque cas, vérifié

Un `match` doit couvrir chaque valeur qu’il peut recevoir : chaque variante d’une énumération ou d’une option, `true` et `false`, et pour les nombres ou les chaînes un `_` final ([E0222](../errors/E0222.md)). Ajoutez une variante à `Shape`, et chaque `match` qui l’oublie est signalé.

```peras error E0222
enum Shape {
  Circle(f64),
  Rect(f64, f64),
  Empty,
}

export fn sides(s: Shape): i32 {
  return match (s) {
    Circle(_) => 0,
    Rect(_, _) => 4,
  };
}
```

Un bras qu’aucune valeur ne peut atteindre, après `_` ou après une variante déjà couverte, est aussi une erreur ([E0224](../errors/E0224.md)) ; de même qu’une variante que l’énumération n’a pas ([E0223](../errors/E0223.md)).

## Des bras avec des instructions

```peras build
export fn sign(n: i32): i32 {
  match (n) {
    0 => {
      return 0;
    }
    _ => {}
  }
  if (n < 0) {
    return -1;
  }
  return 1;
}
```

Un `match` dont les bras sont des blocs est une instruction : son `;` est facultatif, et `return`, `break` et `continue` marchent dans ses bras. Quand chaque bras renvoie, la fonction n’a besoin de rien après.

## Les options

Une option est une énumération à deux variantes, `None` et `Some(valeur)` :

```peras build
export fn describe(o: Option<i32>): i32 {
  return match (o) {
    Some(0) => 100,
    Some(x) => x,
    None => -1,
  };
}
```

## Les résultats : des erreurs qui sont des valeurs

`Result<T, E>` est l’énumération de ce qui peut échouer : `Ok(valeur)` ou `Err(erreur)`. Une fonction qui peut échouer le dit dans son type, et son appelant ne peut pas oublier l’erreur : il doit la traiter avec `match`, ou la transmettre avec `?`.

```peras build
export enum ParseError {
  Empty,
  NotADigit(i32),
}

fn digit(code: i32): Result<i32, ParseError> {
  if (code < 48 || code > 57) {
    return Err(ParseError.NotADigit(code));
  }
  return Ok(code - 48);
}

export fn parse(codes: Array<i32>): Result<i32, ParseError> {
  if (codes.length == 0) {
    return Err(ParseError.Empty);
  }
  let n = 0;
  for (const code of codes) {
    n = n * 10 + digit(code)?;
  }
  return Ok(n);
}

export fn port(codes: Array<i32>): i32 {
  return match (parse(codes)) {
    Ok(n) => n,
    Err(Empty) => -1,
    Err(NotADigit(_)) => -2,
  };
}
```

`digit(code)?` donne le nombre dans un `Ok`, ou renvoie aussitôt l’`Err` depuis `parse`. `?` demande une fonction qui renvoie un `Result` avec le même type d’erreur, ou une `Option` pour `?` sur une option ([E0225](../errors/E0225.md)).

| Méthode                 | Donne                             |
| ----------------------- | --------------------------------- |
| `r.isOk()`, `r.isErr()` | la variante                       |
| `r.unwrapOr(secours)`   | la valeur de l’`Ok`, ou `secours` |

En JavaScript, un résultat est `{ kind: "Ok", values: [valeur] }` ou `{ kind: "Err", values: [erreur] }`.

## La mémoire

Une variante peut contenir des chaînes et des tableaux. La valeur examinée n’est que prêtée aux bras : un nom dans un motif lit ses données, et ne peut pas les déplacer ([E0209](../errors/E0209.md)) ; écrivez `.copy()` pour les garder. À l’intérieur, une énumération contient les données de chaque variante, les inutilisées vides, si bien qu’elle possède toujours les mêmes buffers : le compilateur les libère comme les autres.

## En JavaScript

Une énumération passe la frontière comme un objet, `{ kind: "Circle", values: [1.5] }`, déclaré à TypeScript comme une union : TypeScript vérifie alors lui aussi le `kind` de chaque cas.

```ts
import { area } from "./shapes.peras";

area({ kind: "Rect", values: [2, 3] }); // 6
```

Ensuite : [tableaux, chaînes et options](collections.md).
