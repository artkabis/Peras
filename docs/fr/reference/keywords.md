# Mots-clés

Ces mots ne peuvent pas nommer une variable, une fonction ou un type.

## En usage

| Mot-clé              | Rôle                                                                                  |
| -------------------- | ------------------------------------------------------------------------------------- |
| `fn`                 | déclare une fonction                                                                  |
| `struct`             | déclare une structure                                                                 |
| `enum`, `match`      | déclarent une énumération, décomposent une valeur ([énumérations](../guide/enums.md)) |
| `let`, `const`       | déclarent une variable, une constante                                                 |
| `export`             | rend une fonction ou une structure visible depuis JavaScript                          |
| `return`             | quitte une fonction, avec son résultat                                                |
| `if`, `else`         | choisit entre deux branches                                                           |
| `while`, `for`, `of` | boucles ; `of` est contextuel, dans `for…of` seulement                                |
| `break`, `continue`  | quittent une boucle, passent à son tour suivant                                       |
| `true`, `false`      | les deux valeurs `bool`                                                               |
| `inout`, `sink`      | conventions de passage ([fonctions](../guide/functions.md#conventions-de-passage))    |
| `impl`, `interface`  | méthodes, interfaces ([méthodes et interfaces](../guide/methods.md))                  |
| `type`               | alias de types ([types](types.md#alias-de-types))                                     |

## Réservés

Réservés à des fonctionnalités conçues mais pas encore construites ; les utiliser comme nom est une erreur dès aujourd’hui, pour que le code écrit maintenant continue de fonctionner quand elles arriveront.

| Mot-clé          | Pour                                                                  |
| ---------------- | --------------------------------------------------------------------- |
| `trait`          | gardé libre : Peras les appelle des interfaces                        |
| `class`          | refusé volontairement : Peras utilise des structures et des fonctions |
| `import`         | modules                                                               |
| `async`, `await` | code asynchrone, en mode géré                                         |
