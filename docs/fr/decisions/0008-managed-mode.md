# 0008 : Le mode managed, compilé en JavaScript

_Version anglaise, qui fait foi : [../../en/decisions/0008-managed-mode.md](../../en/decisions/0008-managed-mode.md)._

## Contexte

La [spécification](../spec.md#mode-managed) donne à Peras deux modes, fixés par l’extension du fichier : les modules owned (`.peras`), compilés en WebAssembly, et les modules managed (`.apeiron`), compilés en JavaScript pour ce qui parle au navigateur. Les modules owned sont construits. `perasc-codegen-js` est une crate vide, et le plugin Vite refuse les fichiers `.apeiron`. Une application écrit encore son interface en TypeScript, et appelle ses modules owned à travers un chargeur qui convertit chaque valeur à la frontière.

Cette décision fixe ce qu’est un module managed, et ce que sa première étape construit.

## Ce que d’autres ont fait

- **ReScript et Elm** compilent un langage typé en JavaScript lisible, avec leur propre modèle de données, et appellent JavaScript par des liaisons déclarées.
- **Kotlin/JS et Scala.js** compilent un langage de la JVM en JavaScript : un `Long` exact coûte un objet par valeur ; l’arithmétique des `Int` est donc repliée, et `Long` est émulé.
- **AssemblyScript** a la forme de TypeScript et ne se compile qu’en WebAssembly ; une application le mêle à TypeScript à la main.
- **TypeScript** efface ses types : un `number` annoté `i32` n’existe pas ; rien ne vérifie ce qu’un module reçoit de JavaScript.

## Décision

1. **Le ramasse-miettes possède les valeurs ordinaires.** Un module managed n’a ni déplacements ni emprunts : `let ys = xs` partage le tableau, comme en JavaScript. `sink`, `parallel for`, et `inout` sur une valeur que JavaScript copie y sont refusés ([E0243](../errors/E0243.md)). La possession revient, plus tard, pour les **ressources** seulement : écouteurs, minuteurs, sockets, comme la spécification le prévoit.
2. **Les nombres gardent leur sens.** `number` est le nom de `f64` dans un module managed. `i32`, `i64` et `f64` gardent la sémantique qu’ils ont dans un module owned, pour qu’une fonction passée d’un mode à l’autre calcule la même chose : un `i32` se replie, `(a + b) | 0` et `Math.imul` ; un `i64` est un `BigInt`, replié par `BigInt.asIntN(64, …)` ; une division par zéro lève une exception comme elle déclenche un trap.
3. **Les chaînes sont celles de JavaScript.** `length` et `slice` comptent des unités UTF-16 dans un module managed, des octets d’UTF-8 dans un module owned ; le texte est le même, ses indices non. Un module managed ne convertit pas ses chaînes : le DOM et `fetch` parlent UTF-16.
4. **Les données gardent les formes de la frontière.** Une structure est un objet avec ses champs, une énumération `{ kind, values }`, `Option<T>` une valeur ou `undefined`, un tuple un tableau, `Map` et `Set` ceux de JavaScript : les formes que le chargeur owned donne déjà. Une valeur passe d’un module managed à un module owned sans format à elle.
5. **`async` et `await`, dans les modules managed seulement.** Une `async fn` rend `Promise<T>`, et son corps rend `T` ; `await` n’est permis qu’à l’intérieur ([E0242](../errors/E0242.md)). Un module owned n’attend jamais.
6. **Un module managed importe des fonctions owned.** `import { mean } from "./stats.peras"` vérifie l’appel d’après la signature de la fonction, lue dans le module owned lui-même, et passe par son chargeur. Les nombres, les booléens, les chaînes, et les tableaux, options, résultats, tables et ensembles de ceux-ci passent ; une structure ou une énumération est le type de l’autre module et ne passe pas encore ([E0244](../errors/E0244.md)). Un module owned n’appelle jamais un module managed.

## Étape 1, et la suite

L’étape 1 construit le cœur : fonctions, structures, énumérations et `match`, tableaux, chaînes, tables et ensembles, `async` et `await`, compilés en JavaScript, et les imports de modules owned, par la CLI et le plugin Vite.

L’étape 2 type le navigateur : le DOM, les évènements, `fetch` et les minuteurs, puis les ressources et leur libération, avec le détecteur de fuites que décrit la spécification.

## Non retenu

- **WasmGC.** Il partage le ramasse-miettes de JavaScript sans libérer plus tôt, et complique l’interopérabilité et le débogage ([spécification](../spec.md#mode-managed)).
- **`number` comme seul nombre.** Le choix de JavaScript ferait calculer une fonction différemment dans chaque mode ; `number` est proposé, comme `f64`.
- **Des chaînes en UTF-8 partout.** Chaque appel au DOM convertirait.

## État

**Étape 1 construite.** `perasc build app.apeiron` écrit `app.js` et `app.d.ts`, et compile les modules owned qu’il importe ; le plugin Vite compile les fichiers `.apeiron`, et écrit `app.d.apeiron.ts`.

- **Les vérifications.** Un module managed saute la passe de possession. `number` et `Promise<T>` ne se résolvent que là ; une `async fn` déclare `Promise<T>` et son corps rend `T`. `import` lit le module owned à côté, le vérifie, et déclare chaque fonction exportée avec sa signature ; le chemin doit nommer un fichier `.peras` dans le même dossier ou en dessous.
- **Le générateur** (`perasc-codegen-js`) écrit du JavaScript à partir de l’arbre syntaxique et des types du vérificateur, avec le nom auquel se résout chaque variable : chaque locale reçoit un nom unique dans sa fonction, et un mot que JavaScript réserve reçoit un `$`. Les fonctions génériques sont compilées une fois par instance, comme pour WebAssembly. Un `match` ou un `?` qui est une valeur écrit ses instructions avant l’expression, les opérandes d’avant gardés dans des temporaires pour que l’ordre d’évaluation ne change pas, et `&&` ne les exécute que sur son chemin. Les fonctions du runtime (`$at`, `$div`…) ne sont écrites dans un module que s’il les utilise.
- **Les mêmes résultats.** Une source compilée des deux façons, `kernels.peras` et `kernels.apeiron`, donne les mêmes résultats sur 21 appels dans Node : arithmétique `i32` repliée, `i64`, `f32`, conversions saturées, tables, tri, `match`, `?`, gabarits, et les mêmes erreurs, une `RangeError` au même texte pour un indice hors limites, une `WebAssembly.RuntimeError` pour une division par zéro. Dans Chromium, un module managed qui appelle un module owned tourne depuis le serveur de développement de Vite et depuis son build de production.
- Chaque source des tests UI, des fixtures de bout en bout et de la documentation qui se vérifie comme module managed, 104 en tout, se compile en un JavaScript que Node analyse ; la cible de fuzz compile aussi tout texte comme module managed.

- **La vitesse du JavaScript écrit à la main.** `node scripts/bench-managed.mjs` exécute quatre noyaux compilés des deux façons face au même travail écrit à la main, dans Node : le module managed va à 0,93 à 1,31 fois sa vitesse, le module owned à 1,02 à 1,91 fois, sauf là où il copie des tableaux JavaScript à l’aller et au retour ([résultats](../../../bench/README.fr.md#modules-managed-benchmanaged)). Les contrôles de bornes sont prouvés inutiles dans une boucle comptée sur un tableau dont le corps n’appelle rien, comme en WebAssembly : sans, la somme tournait à 0,83 – 0,86.
- **`inout` dans un module managed.** Un paramètre est en lecture seule dans les deux modes ; un paramètre `inout`, d’un type que JavaScript partage (un tableau, une table, un ensemble, une structure), est changé sur place pour son appelant, et ne peut pas être remplacé par une autre valeur. Un type que JavaScript copie est refusé ([E0243](../errors/E0243.md)).

Pas encore : le navigateur typé et les ressources (étape 2), les fonctions comme valeurs, les structures et les énumérations à travers un import, les options d’options, refusées par `E0900` car JavaScript ne distingue pas `Some(None)` de `None`.

## Conséquences

- Les résultats d’une fonction sont les mêmes dans les deux modes, sauf les indices des chaînes, qui comptent des unités UTF-16 dans un module managed.
- L’arithmétique exacte des `i64` coûte un `BigInt` dans un module managed ; `number` y est le choix rapide.
