# Vers un langage complet

Peras couvre aujourd’hui le cœur d’un langage : nombres, structures, fonctions, contrôle du flux et diagnostics. Construire de grandes applications web demande bien plus. Cette page liste ce qui est prévu, groupé en phases, dans l’ordre où elles doivent arriver. Chaque phase se termine par quelque chose d’utilisable, et chaque fonctionnalité arrive comme le reste : vérifiée, expliquée en anglais et en français, testée, et documentée ici.

Les exemples de cette page montrent la syntaxe visée ; le compilateur les refuse aujourd’hui, et les tests de la documentation vérifient qu’il le fait, si bien que cette page est mise à jour à l’arrivée de chaque fonctionnalité.

## Phase 1 : les données sur le tas

Les premiers types qui ont besoin de mémoire, sur les fondations déjà construites : le vérificateur de libérations et l’allocateur de débogage.

- ~~`Array<T>` compilé : indexation avec vérification des bornes, `push`, `pop`, `for…of`~~ : fait pour tous les types d’éléments, chaînes et tableaux compris, avec `copy()`.
- ~~`String` compilé, en UTF-8~~ : fait, avec `+`, `+=`, `==`, `length` en octets et `copy()`.
- ~~`Option<T>` compilé~~ : fait, avec `?` et l’arithmétique vérifiée.
- ~~Les vérifications de possession : déplacements, `inout` et `sink`, alias refusés~~ : fait ([E0204](../errors/E0204.md), [E0206](../errors/E0206.md), [E0207](../errors/E0207.md), [E0209](../errors/E0209.md)), avec `inout` sur tous les types, et l’historique d’une valeur dans les erreurs : où elle a été reçue, déplacée, puis utilisée.

## Phase 2 : un système de types complet

Ce qui permet à un programme de dire précisément ce que ses données peuvent être.

- ~~**Des énumérations avec données, et `match`**, vérifié pour l’exhaustivité~~ : fait, voir [énumérations et match](../guide/enums.md).
- ~~**`Result<T, E>` et `?`**, pour des erreurs qui sont des valeurs~~ : fait, voir [les résultats](../guide/enums.md#les-résultats--des-erreurs-qui-sont-des-valeurs) ; les fonctions d’analyse comme `parse` arrivent avec la bibliothèque standard.
- ~~**Des génériques**, sur les fonctions et les structures, avec des contraintes~~ : fait, sur les énumérations aussi, voir [génériques](../guide/generics.md) ; avec des contraintes prédéfinies, et des interfaces pour les vôtres.
- ~~**Des méthodes sur les structures** (`impl`) et **des interfaces** pour les comportements partagés, résolues à la compilation~~ : fait, voir [méthodes et interfaces](../guide/methods.md).
- ~~Tuples, alias de types, constantes de module compilées~~ : fait, voir [types](types.md#tuples) et [constantes de module](../guide/basics.md#constantes-de-module).

```peras planned
fn parse_port(text: String): Result<u16, ParseError> {
  const n = text.parse::<u32>()?;
  if (n > 65535) {
    return Err(ParseError.OutOfRange);
  }
  return Ok(n.toU16());
}
```

## Phase 3 : une bibliothèque standard

Les outils de tous les jours, avec les mêmes règles : types explicites, aucune allocation cachée, erreurs sous forme de valeurs. En cours : les méthodes de tableaux qui prennent une fonction fléchée, compilées sur place en boucles, et `min`, `max`, `clamp` sont [faites](methods.md) ; l’allocateur de production, TLSF, est venu d’abord ([décision 0002](../decisions/0002-tlsf-allocator.md)). Le texte vient ensuite : des morceaux de texte qui sont des vues sur le texte dont ils viennent, puis des chaînes de méthodes fusionnées en une seule boucle, choisies par le compilateur ([décision 0004](../decisions/0004-strings-without-allocation.md)).

| Domaine     | Quoi                                                                                                                                                                                                                         |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| texte       | ~~`slice`, `split`, `indexOf`, `includes`, `startsWith`, `replace`, `trim`, changement de casse, `join`, `toString` sur les entiers et les booléens, `parseI32`, `parseI64`~~ (fait), flottants en texte et chaînes gabarits |
| tableaux    | ~~`map`, `filter`, `reduce`, `find`, `findIndex`, `some`, `every`~~ (fait, avec les fonctions fléchées), ~~`sort`, `slice`, `concat`, `join`, `reverse`, `includes`, `indexOf`~~ (fait), itérateurs et intervalles           |
| collections | `Map<K, V>`, `Set<T>`, `Deque<T>`                                                                                                                                                                                            |
| nombres     | ~~`min`, `max`, `clamp`, `pow`, `exp`, `log`, `atan2`, `hypot`~~ (fait), constantes, nombres aléatoires avec graine                                                                                                          |
| temps       | durées et instants                                                                                                                                                                                                           |
| données     | JSON, encodage de texte, octets                                                                                                                                                                                              |

```peras build
export fn words(text: String): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}
```

## Phase 4 : le mode géré, pour le reste de l’application

Les modules `.apeiron` compilés en JavaScript, pour tout ce qui parle au navigateur ([spécification](../spec.md#mode-managed)).

- Le DOM, les événements, `fetch`, les minuteries, le stockage, les WebSockets, les Web Workers, avec des API typées.
- `async` et `await`.
- Des fermetures, avec des règles qui empêchent les fuites classiques : un écouteur ou une minuterie est une ressource, libérée avec son propriétaire, et un détecteur de fuites signale celles qui ne le sont pas.
- Des appels entre modules gérés et possédés, avec des conversions vérifiées à la frontière.

```peras planned
import { inside } from "./mandel.peras";

export async fn render(canvas: HTMLCanvasElement): Promise<void> {
  const response = await fetch("/settings.json");
  const settings = Settings.fromJson(await response.text())?;
  draw(canvas, settings, inside);
}
```

## Phase 5 : modules et paquets

- `import` et `export` entre modules Peras.
- L’utilisation typée des bibliothèques JavaScript, à partir de leurs déclarations `.d.ts`.
- Des paquets : un manifeste, des versions, un registre.

## Phase 6 : graphisme et performance

Ce que la spécification appelle la rastérisation ([spécification](../spec.md#rasterisation)).

- `Framebuffer<Rgba8>` et les types de pixels, partagés avec le canvas sans copie.
- Des types et des opérations SIMD.
- La règle `@frame` : une fonction qui dessine une image ne peut pas allouer, et le compilateur le prouve.
- Les allocateurs `Arena` et `Pool<T>`.
- Des liaisons WebGPU.

## Phase 7 : l’outillage

- Une extension d’éditeur et un serveur de langage : erreurs à la frappe, complétion, survol, aller à la définition, renommage.
- Un formateur, pour que tous les fichiers Peras se ressemblent.
- Un lanceur de tests : des tests écrits en Peras, lancés par Vitest.
- `create-peras`, pour démarrer un projet en une commande.
- Un bac à sable dans cette documentation : le compilateur lui-même compilé en WebAssembly, qui tourne dans la page.

## Pourquoi cet ordre

1. **Les données d’abord** : presque tout programme a besoin de tableaux et de texte, et les fondations mémoire pour eux sont prêtes.
2. **Les types ensuite** : les énumérations, `match`, `Result` et les génériques changent la façon d’écrire chaque API suivante ; la bibliothèque standard se conçoit dessus, pas avant.
3. **La bibliothèque standard** donne ensuite à chaque type les méthodes qui le rendent utile.
4. **Le mode géré** ouvre le navigateur : le DOM et `async` font de Peras un langage d’application, pas seulement de calcul.
5. **Modules, graphisme et outillage** suivent, la prise en charge des éditeurs démarrant dès que le système de types est stable.

Ce qui fonctionne déjà est sur la [page d’état](status.md).
