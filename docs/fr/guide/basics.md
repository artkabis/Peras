# Valeurs et variables

## Variables

`let` déclare une variable modifiable, `const` une variable qui ne l’est pas. Le type vient de la valeur, ou s’écrit après deux-points.

```peras build
export fn area(width: f64, height: f64): f64 {
  const scale = 2.0;
  let total: f64 = width * height;
  total = total * scale;
  return total;
}
```

Peras est strict sur les noms, pour garder le code lisible :

- un nom n’est déclaré qu’une fois par fonction : pas de masquage ([E0202](../errors/E0202.md)) ;
- chaque variable est lue au moins une fois ([E0203](../errors/E0203.md)) ; préfixez son nom par `_` pour la garder volontairement ;
- une `const` n’est jamais réaffectée ([E0205](../errors/E0205.md)).

```peras error E0202
export fn twice(a: i32): i32 {
  let a = a * 2;
  return a;
}
```

## Nombres

| Type                      | Valeurs                     |
| ------------------------- | --------------------------- |
| `i8`, `i16`, `i32`, `i64` | entiers signés              |
| `u8`, `u16`, `u32`, `u64` | entiers non signés          |
| `f32`, `f64`              | nombres à virgule flottante |

L’arithmétique entière reboucle en cas de dépassement, de la même façon dans chaque backend. Une division par zéro arrête le programme (un _trap_) : elle ne renvoie jamais une valeur inventée.

Les littéraux prennent le type que demande leur contexte, `i32` et `f64` quand rien ne le fixe. Ils s’écrivent en hexadécimal, en binaire, avec des séparateurs et des exposants :

```peras build
export fn literals(): i64 {
  const mask: i64 = 0xFF_FF;
  const flags: i64 = 0b1010;
  const million: i64 = 1_000_000;
  return mask + flags + million;
}
```

```peras build
export fn tiny(): f64 {
  return 1.5e-3;
}
```

> Aujourd’hui, `i32`, `i64`, `f32` et `f64` sont compilés en WebAssembly ; les types sur 8 et 16 bits et les non signés sont vérifiés, pas encore compilés.

## Aucune conversion implicite

Mélanger deux types de nombres est une erreur : on convertit explicitement, si bien qu’une perte de précision est toujours visible.

```peras error E0210
export fn mean(total: i32, count: i64): i64 {
  return total / count;
}
```

```peras build
export fn mean(total: i32, count: i64): i64 {
  return total.toI64() / count;
}
```

Convertir un flottant en entier arrondit vers zéro et sature : `(1e20).toI32()` vaut le plus grand `i32`, jamais une valeur indéfinie.

## Booléens

`bool` vaut `true` ou `false`, et une condition doit être un `bool` : Peras n’a pas de valeurs « truthy ».

```peras error E0217
export fn sign(n: i32): i32 {
  if (n) {
    return 1;
  }
  return 0;
}
```

```peras build
export fn sign(n: i32): i32 {
  if (n != 0) {
    return 1;
  }
  return 0;
}
```

`==` et `!=` comparent strictement, comme `===` et `!==` en JavaScript ; Peras n’a pas de `===` ([E0006](../errors/E0006.md)).

## Commentaires

```peras build
// Un commentaire de ligne.
/* Un commentaire de bloc,
   sur plusieurs lignes. */
export fn one(): i32 {
  return 1; // après du code, aussi
}
```

Ensuite : [les fonctions](functions.md).
