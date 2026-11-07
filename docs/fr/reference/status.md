# Ce qui fonctionne aujourd’hui

Peras est jeune. Cette page dit, fonctionnalité par fonctionnalité, jusqu’où va chacune :

- <span class="status compiled">compilé</span> `perasc build` la transforme en WebAssembly ;
- <span class="status checked">vérifié</span> `perasc check` l’accepte, avec toutes les règles appliquées, mais `perasc build` signale [E0900](../errors/E0900.md) ;
- <span class="status planned">prévu</span> conçu dans la [spécification](../spec.md) ou la [feuille de route](roadmap.md), pas encore dans le langage : le compilateur le refuse.

## Types

| Fonctionnalité                                                | État                                         |
| ------------------------------------------------------------- | -------------------------------------------- |
| `i32`, `i64`, `f32`, `f64`, `bool`                            | <span class="status compiled">compilé</span> |
| `i8`, `i16`, `u8`, `u16`, `u32`, `u64`                        | <span class="status checked">vérifié</span>  |
| structures, imbriquées, en paramètres et en résultats         | <span class="status compiled">compilé</span> |
| `Option<T>`, `Some`, `None`, `?`                              | <span class="status compiled">compilé</span> |
| `Array<T>` de nombres, booléens, structures, options          | <span class="status compiled">compilé</span> |
| `String`, en UTF-8                                            | <span class="status compiled">compilé</span> |
| `Array<T>` de chaînes ou de tableaux                          | <span class="status checked">vérifié</span>  |
| `Result<T, E>`, énumérations et unions, `match`               | <span class="status planned">prévu</span>    |
| génériques, interfaces et traits, méthodes sur les structures | <span class="status planned">prévu</span>    |
| `Map`, `Set`, tuples, fermetures                              | <span class="status planned">prévu</span>    |
| `Rc<T>`, `Framebuffer`, types de pixels, SIMD                 | <span class="status planned">prévu</span>    |

## Instructions et expressions

| Fonctionnalité                                                  | État                                                                                                                 |
| --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `let`, `const`, affectations, `+=` et les autres, `++`, `--`    | <span class="status compiled">compilé</span>                                                                         |
| opérateurs arithmétiques, de comparaison, bit à bit et logiques | <span class="status compiled">compilé</span>, sauf `%` sur les flottants <span class="status checked">vérifié</span> |
| `if`, `while`, `for`, `break`, `continue`                       | <span class="status compiled">compilé</span>                                                                         |
| `for…of`                                                        | <span class="status compiled">compilé</span>                                                                         |
| fonctions, récursion, `export`                                  | <span class="status compiled">compilé</span>                                                                         |
| paramètres `sink`                                               | <span class="status compiled">compilé</span>                                                                         |
| paramètres `inout`, `&` à l’appel                               | <span class="status compiled">compilé</span>                                                                         |
| possession : déplacements, emprunts, alias refusés, libérations | <span class="status compiled">compilé</span>                                                                         |
| `const` au niveau du module                                     | <span class="status checked">vérifié</span>                                                                          |
| `import` entre modules                                          | <span class="status planned">prévu</span>                                                                            |

## Méthodes

Chaque méthode intégrée, avec son état, est dans la [référence des méthodes](methods.md).

## Modes et outillage

| Fonctionnalité                                                    | État                                                                             |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| mode possédé, `.peras`, vers WebAssembly                          | <span class="status compiled">compilé</span>, pour les fonctionnalités ci-dessus |
| mode géré, `.apeiron`, vers JavaScript                            | <span class="status planned">prévu</span>                                        |
| diagnostics en anglais et en français, `--json`, `perasc explain` | disponible                                                                       |
| plugin Vite, déclarations TypeScript                              | disponible                                                                       |
| allocateur de débogage, vérificateur de libérations               | construits et testés, en attente du premier type sur le tas                      |
| `async`, le DOM, `fetch`, les événements                          | <span class="status planned">prévu</span>, en mode géré                          |
| prise en charge des éditeurs (LSP), formateur, lanceur de tests   | <span class="status planned">prévu</span>                                        |

Ce qui vient ensuite, et dans quel ordre : [vers un langage complet](roadmap.md).
