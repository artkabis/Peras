# Types

## Nombres

| Type  | Taille  | Intervalle                | État                                         |
| ----- | ------- | ------------------------- | -------------------------------------------- |
| `i8`  | 8 bits  | −128 à 127                | <span class="status checked">vérifié</span>  |
| `i16` | 16 bits | −32 768 à 32 767          | <span class="status checked">vérifié</span>  |
| `i32` | 32 bits | environ ±2,1 × 10⁹        | <span class="status compiled">compilé</span> |
| `i64` | 64 bits | environ ±9,2 × 10¹⁸       | <span class="status compiled">compilé</span> |
| `u8`  | 8 bits  | 0 à 255                   | <span class="status checked">vérifié</span>  |
| `u16` | 16 bits | 0 à 65 535                | <span class="status checked">vérifié</span>  |
| `u32` | 32 bits | 0 à environ 4,3 × 10⁹     | <span class="status checked">vérifié</span>  |
| `u64` | 64 bits | 0 à environ 1,8 × 10¹⁹    | <span class="status checked">vérifié</span>  |
| `f32` | 32 bits | IEEE 754 simple précision | <span class="status compiled">compilé</span> |
| `f64` | 64 bits | IEEE 754 double précision | <span class="status compiled">compilé</span> |

Les règles, les mêmes dans chaque backend :

- `+`, `-`, `*` sur les entiers arrêtent le programme sur un dépassement dans un build de debug, le défaut de `perasc build` et du serveur de développement de Vite, par une `RangeError` qui le dit, et rebouclent dans un build de production (`--allocator tlsf`), où vérifier coûterait du temps, comme en Rust ; `wrappingAdd`, `wrappingSub` et `wrappingMul` rebouclent exprès dans les deux, `checkedAdd` et les autres renvoient `None` à la place. Un compteur de boucle `i++` sous `i < borne` n’est pas vérifié : il ne peut pas dépasser ;
- la division et le reste entiers par zéro arrêtent le programme, comme `MIN / -1` ;
- les conversions de flottant vers entier arrondissent vers zéro et saturent ; NaN devient 0 ;
- aucune conversion n’est implicite : [E0210](../errors/E0210.md), [E0211](../errors/E0211.md).

### Littéraux

| Forme                       | Exemple                   | Type                                           |
| --------------------------- | ------------------------- | ---------------------------------------------- |
| décimal                     | `42`, `1_000_000`         | le type entier du contexte, `i32` par défaut   |
| hexadécimal, octal, binaire | `0xFF`, `0o755`, `0b1010` | idem                                           |
| flottant                    | `3.14`, `1.5e-3`, `2e10`  | le type flottant du contexte, `f64` par défaut |

Un littéral qui ne tient pas dans son type est une erreur ([E0212](../errors/E0212.md)).

## `bool`

`true` ou `false`. Les conditions doivent être des `bool` ([E0217](../errors/E0217.md)). <span class="status compiled">compilé</span>

## `void`

Le résultat d’une fonction qui ne renvoie rien. <span class="status compiled">compilé</span>

## Structures

`struct Nom { champ: Type; … }` : une valeur faite de champs nommés, construite avec `Nom(…)` et copiée par affectation. Voir [les structures](../guide/structures.md). <span class="status compiled">compilé</span>

## `Array<T>`

Une liste extensible de valeurs de type `T`, écrite `[a, b]`, compilée quand `T` est un nombre, `bool`, ou une structure ou une option de ceux-ci. Voir [les collections](../guide/collections.md#arrayt). <span class="status compiled">compilé</span>

## `String`

Du texte en UTF-8, écrit `"…"` ; sa `length` compte des octets. Voir [les collections](../guide/collections.md#string). <span class="status compiled">compilé</span>

## `Option<T>`

`Some(valeur)` ou `None` : une valeur qui peut être absente. Peras n’a ni `null` ni `undefined` ; JavaScript voit une option vide comme `undefined`. <span class="status compiled">compilé</span>

## Tuples

`(i32, String)` : quelques valeurs de types fixes, sans noms de champs, écrites `(1, "a")` et lues avec `.0`, `.1`. Un tuple est une structure : gardé comme ses champs, il ne coûte rien. JavaScript voit un tableau, `[number, string]`. <span class="status compiled">compilé</span>

```peras build
fn minmax(xs: Array<i32>): (i32, i32) {
  let lo = xs[0];
  let hi = xs[0];
  for (const x of xs) {
    if (x < lo) {
      lo = x;
    }
    if (x > hi) {
      hi = x;
    }
  }
  return (lo, hi);
}

export fn spread(xs: Array<i32>): i32 {
  const range = minmax(xs);
  return range.1 - range.0;
}
```

## Alias de types

`type Meters = f64;` donne un autre nom à un type, interchangeable avec lui ; `type Grid<T> = Array<Array<T>>;` prend des paramètres de type. Un alias ne peut pas se contenir lui-même ([E0232](../errors/E0232.md)). <span class="status compiled">compilé</span>

```peras build
type Meters = f64;
type Grid<T> = Array<Array<T>>;

export fn cells(g: Grid<i32>, width: Meters): f64 {
  return g.length.toF64() * width;
}
```

Deux alias de `f64` se mélangent librement : c’est le même type. Une valeur qui ne doit pas se mélanger avec sa représentation, comme un `UserId` qui n’est pas un `OrderId`, est une structure, `struct UserId { value: i64; }` : gardée comme son champ, elle ne coûte rien à l’exécution.

## Pas encore dans le langage

Les énumérations ont leur page : [énumérations et match](../guide/enums.md). Les résultats y sont aussi : `Result<T, E>`, `Ok(valeur)` ou `Err(erreur)`. <span class="status compiled">compilé</span>

Les fonctions, structures et énumérations génériques ont leur page : [génériques](../guide/generics.md) ; les méthodes et les interfaces aussi : [méthodes et interfaces](../guide/methods.md). <span class="status compiled">compilé</span>

`Map<K, V>` et `Set<T>` : voir [le guide](../guide/collections.md#mapk-v-et-sett). Unions et types de fonctions : voir la [feuille de route](roadmap.md).
