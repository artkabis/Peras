# Modules managed

Un fichier Peras est de l’un de deux genres, fixé par son extension ([décision 0008](../decisions/0008-managed-mode.md)) :

- un module **owned** (possédé), `.peras`, se compile en WebAssembly : une mémoire qu’il gère lui-même, vérifiée à la compilation, pour le calcul ;
- un module **managed** (géré), `.apeiron`, se compile en JavaScript : des valeurs que gère le ramasse-miettes, pour le reste de l’application. Il attend avec `async` et `await`, et appelle les modules owned en les important.

Les deux s’écrivent dans le même langage. Un module managed se compile en un module ES ordinaire, qui tourne partout où tourne JavaScript. <span class="status compiled">compilé</span>

```apeiron build
import { mean } from "./stats.peras";

async fn load(n: i32): Promise<Array<number>> {
  let xs: Array<number> = [];
  for (let i = 0; i < n; i++) {
    xs.push(i.toF64() / 10.0);
  }
  return xs;
}

export async fn report(n: i32): Promise<String> {
  const xs = await load(n);
  return `mean ${mean(xs)} of ${xs.length} values`;
}
```

`mean` tourne en WebAssembly ; `report` en JavaScript.

## Les valeurs appartiennent au ramasse-miettes

Dans un module managed, rien n’est déplacé et rien n’est prêté : affecter ou passer un tableau, une table, un ensemble ou une structure le partage, comme en JavaScript. Un paramètre est en lecture seule, comme dans un module owned ; un paramètre `inout` est changé sur place, et son appelant voit le changement :

```apeiron build
export fn record(log: inout Array<String>, line: String): void {
  log.push(line);
}

export fn twice(): i32 {
  let log: Array<String> = [];
  record(&log, "a");
  record(&log, "b");
  return log.length;
}
```

Un nombre, un booléen, une chaîne ou une option est copié quand on le passe : un `inout` ne pourrait pas atteindre son appelant, et il est refusé, comme `sink`, qui donne une valeur, et `parallel for`, qui partage la mémoire entre fils ([E0243](../errors/E0243.md)) :

```apeiron error E0243
export fn bump(n: inout i32): void {
  n += 1;
}
```

La possession reviendra dans les modules managed pour les **ressources** seulement : écouteurs, minuteurs, sockets, libérés avec leur propriétaire.

## Les nombres gardent leur sens

`number` est le nom managed de `f64`. Les autres types de nombres calculent exactement comme dans un module owned, pour qu’une fonction passée d’un mode à l’autre donne les mêmes résultats :

| Type                      | En JavaScript                                                            |
| ------------------------- | ------------------------------------------------------------------------ |
| `i32`, `u32`, plus petits | un `number`, replié après chaque opération : `(a + b) \| 0`, `Math.imul` |
| `i64`, `u64`              | un `bigint`, replié sur 64 bits                                          |
| `f32`                     | un `number`, arrondi en `f32` après chaque opération par `Math.fround`   |
| `f64`, `number`           | un `number`                                                              |

Un entier divisé par zéro lève la `WebAssembly.RuntimeError` d’un module owned ; un indice hors limites, la même `RangeError`, avec le même texte. Les conversions arrondissent vers zéro et saturent, comme dans un module owned.

```apeiron build
export fn checksum(xs: Array<i32>): i32 {
  let h = 17;
  for (const x of xs) {
    h = h * 31 + x;
  }
  return h;
}
```

`h * 31` se replie sur 32 bits, comme en WebAssembly.

## Les chaînes sont celles de JavaScript

Un module managed garde ses chaînes telles que JavaScript les a : le DOM et `fetch` parlent UTF-16, et rien n’est converti. `length`, `slice` et `indexOf` y comptent donc des unités UTF-16, là où un module owned compte des octets d’UTF-8 : `"é".length` vaut 1 dans un module managed, 2 dans un module owned. Le texte est le même ; seuls ses indices diffèrent. Les autres méthodes donnent les mêmes résultats dans les deux modes : `toUpperCase` ne change que les lettres ASCII, `trim` retire espaces, tabulations et sauts de ligne, `split("")` donne les caractères.

## Attendre

Une fonction `async` rend une `Promise<T>` ; son corps rend un `T`. `await` attend une promesse, dans une fonction `async` seulement ([E0242](../errors/E0242.md)) :

```apeiron error E0242
async fn load(): Promise<number> {
  return 1.0;
}

export fn now(): number {
  return await load();
}
```

Un module owned n’attend jamais : il calcule et rend, et `async`, `await` et `Promise` y sont refusés.

## Importer des modules owned

Un module managed importe les fonctions exportées de modules owned, par leur chemin relatif, dans le même dossier ou en dessous :

```apeiron build
import { mean } from "./stats.peras";

export fn centered(xs: Array<number>): Array<number> {
  const m = mean(xs);
  return xs.map((x) => x - m);
}
```

Chaque appel est vérifié d’après la signature de la fonction, lue dans le module owned, et converti par son chargeur comme pour tout appelant JavaScript ([les types à la frontière](javascript.md#les-types-à-la-frontière)). Les nombres, les booléens, les chaînes, et les tableaux, options, résultats, tables et ensembles de ceux-ci passent. Une structure ou une énumération est le type propre de l’autre module et ne passe pas encore ([E0244](../errors/E0244.md)) : donnez à la fonction des nombres ou des tableaux, et construisez la structure de chaque côté.

Un module owned ne peut pas importer un module managed : le calcul n’appelle pas l’interface.

## Compiler

`perasc build app.apeiron` écrit `app.js` et `app.d.ts`, et compile chaque module owned qu’il importe au même chemin relatif : `./lib/stats.peras` devient `lib/stats.wasm`, `lib/stats.js` et `lib/stats.d.ts`, que `app.js` importe. `--imports source` garde les imports de fichiers `.peras` tels quels, pour un bundler qui les compile lui-même.

Avec Vite, le plugin compile les deux genres : importez `./app.apeiron` depuis TypeScript comme tout module. Il écrit `app.d.apeiron.ts` à côté, pour TypeScript.

```ts
import { report } from "./app.apeiron";

document.body.textContent = await report(10);
```

## Pas encore

- **Le navigateur, typé** : le DOM, les évènements, `fetch`, les minuteurs et le stockage, avec leurs types, viennent à l’étape suivante. D’ici là, un module managed reçoit et rend des valeurs, et le code TypeScript appelle le navigateur.
- **Les ressources** : écouteurs et minuteurs possédés et libérés, avec un détecteur de fuites en debug.
- **Les fonctions comme valeurs**, hors des fonctions fléchées données aux méthodes des tableaux.
- **Les structures et les énumérations à travers un import.**
- **Les options d’options**, refusées par [E0900](../errors/E0900.md) : en JavaScript, `Some(None)` et `None` valent tous deux `undefined`.
