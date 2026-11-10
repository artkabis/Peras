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

| Domaine     | Quoi                                                                                                                                                                                                                       |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| texte       | ~~`slice`, `split`, `indexOf`, `includes`, `startsWith`, `replace`, `trim`, changement de casse, `join`, `toString` sur les entiers et les booléens, `parseI32`, `parseI64`, chaînes gabarits, flottants en texte~~ (fait) |
| tableaux    | ~~`map`, `filter`, `reduce`, `find`, `findIndex`, `some`, `every`~~ (fait, avec les fonctions fléchées), ~~`sort`, `slice`, `concat`, `join`, `reverse`, `includes`, `indexOf`~~ (fait), itérateurs et intervalles         |
| collections | ~~`Map<K, V>`, `Set<T>`~~ (fait pour les valeurs nombres et booléens ; parcours et frontière JavaScript ensuite, [décision 0005](../decisions/0005-maps-and-sets.md)), `Deque<T>`                                          |
| nombres     | ~~`min`, `max`, `clamp`, `pow`, `exp`, `log`, `atan2`, `hypot`~~ (fait), constantes, nombres aléatoires avec graine                                                                                                        |
| temps       | durées et instants                                                                                                                                                                                                         |
| données     | JSON, encodage de texte, octets                                                                                                                                                                                            |

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
- Deux outils à part du cœur ouvert, propriétaires ([décision 0006](../decisions/0006-licensing.md)) : l’extension VS Code et un analyseur de fuites mémoire, construits sur ce que Peras publie. Les contrôles de fuites de l’allocateur de débogage restent dans le cœur.

## Avant d’utiliser Peras en production

Une revue extérieure du projet, en novembre 2026, a déconseillé de l’utiliser en production pour l’instant. Ses faits qui tiennent, vérifiés dans ce dépôt, sont gardés ici ; chacun est du travail pour les phases ci-dessus, pas un verdict sur la direction du langage.

- **Version 0.0.1, sans promesse de stabilité** : jusqu’à la 1.0, toute version peut casser le langage. Une politique écrite, qui dit ce qui peut changer et comment un changement est annoncé, vient avant tout utilisateur extérieur.
- **La moitié de la conception manque** : les modules `.apeiron` et leur compilateur vers JavaScript (phase 4) n’existent pas encore ; `perasc-codegen-js` est une crate vide, et `std/` ne contient que son README.
- **Un seul mainteneur** : le projet dépend d’une personne, aidée d’un assistant d’IA ; il n’a pas encore de communauté.
- **Pas de bibliothèques à lui** : le plan est d’utiliser celles de npm à travers leurs déclarations (phase 5), pas de les réécrire ; d’ici là, un module Peras fait un travail précis au sein d’une application TypeScript.
- **Des workers, mais pas de mémoire partagée ; pas de GPU** : un module tourne tel quel dans des Web Workers, une instance chacun, mesuré 2,6 à 2,9 fois plus rapide sur 4 cœurs là où le travail l’emporte sur la copie des données, et plus lent sinon ([Web Workers](../../../bench/README.fr.md#web-workers)) ; la mémoire partagée entre workers n’est pas encore produite (la décision [0003](../decisions/0003-runtime-proved-at-compile-time.md) prévoit des déplacements entre fils), ni WebGPU (phase 6).
- **La possession s’apprend** : les diagnostics expliquent chaque refus, avec l’historique de la valeur et la correction, mais il n’existe pas encore de savoir communautaire sur lequel s’appuyer.
- **Comparé à Rust et AssemblyScript tels qu’on les écrit habituellement, pas encore à leurs experts** : Peras va à la vitesse du Rust idiomatique, devant là où son compilateur en fait plus (boucles SIMD avec conditions, tri par base, passage du texte), et 2,4 à 7,3 fois plus vite qu’AssemblyScript là où il alloue ou parcourt des tableaux ([face à Rust et AssemblyScript](../../../bench/README.fr.md#face-à-rust-et-assemblyscript)).
- **Une licence manquait** : réglé par la [décision 0006](../decisions/0006-licensing.md).

Aujourd’hui, Peras a sa place là où son risque reste petit : quelques fonctions de calcul, compilées en WebAssembly et appelées depuis une application TypeScript à travers le plugin Vite.

## Pourquoi cet ordre

1. **Les données d’abord** : presque tout programme a besoin de tableaux et de texte, et les fondations mémoire pour eux sont prêtes.
2. **Les types ensuite** : les énumérations, `match`, `Result` et les génériques changent la façon d’écrire chaque API suivante ; la bibliothèque standard se conçoit dessus, pas avant.
3. **La bibliothèque standard** donne ensuite à chaque type les méthodes qui le rendent utile.
4. **Le mode géré** ouvre le navigateur : le DOM et `async` font de Peras un langage d’application, pas seulement de calcul.
5. **Modules, graphisme et outillage** suivent, la prise en charge des éditeurs démarrant dès que le système de types est stable.

Ce qui fonctionne déjà est sur la [page d’état](status.md).
