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

- `+`, `-`, `*` sur les entiers rebouclent en cas de dépassement ; `checkedAdd` et les autres renvoient `None` à la place, `wrappingAdd` et les autres le disent explicitement ;
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

## Pas encore dans le langage

Les énumérations ont leur page : [énumérations et match](../guide/enums.md). `Result<T, E>`, unions, génériques, tuples, `Map`, `Set`, types de fonctions : voir la [feuille de route](roadmap.md).
