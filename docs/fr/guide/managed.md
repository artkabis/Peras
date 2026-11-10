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

## Le navigateur

Un module managed utilise le navigateur directement : `document`, les éléments, les évènements, un canvas, les minuteurs, `fetch` et `storage`, chacun typé ([référence du navigateur](../reference/browser.md), [décision 0009](../decisions/0009-browser-and-resources.md)). Là où le DOM donne `null`, Peras donne `None` : un élément absent est donc un cas que le compilateur vous fait traiter.

```apeiron build
export fn greet(name: String): void {
  match (document.getElementById("greeting")) {
    Some(el) => {
      el.textContent = `Bonjour, ${name}`;
    }
    None => log("pas de #greeting dans la page"),
  }
}
```

Un gestionnaire d’évènement est une fonction fléchée, qui appelle en général une fonction du module : `button.on("click", (e) => clicked(e))`.

## Les ressources

Un écouteur, un minuteur et un groupe sont des **ressources** : chacun appartient à une variable, et le compilateur le libère quand cette variable sort de sa portée, quelle que soit la façon dont le bloc se termine. Un écouteur est retiré, un minuteur arrêté, un groupe libère tout ce qu’on lui a ajouté. Rien ne continue d’écouter ou de battre une fois son propriétaire parti, ce qui est la façon dont fuient la plupart des interfaces.

```apeiron build
fn tick(): void {
  log("tick");
}

export fn start(root: Element): Group {
  let group = Group.new();
  const timer = setInterval(1000, () => tick());
  group.add(timer);
  const listener = root.on("click", (e) => e.stopPropagation());
  group.add(listener);
  return group;
}
```

`start` donne son groupe à son appelant, qui le libère quand la vue disparaît : `group.release()` en TypeScript, ou `using group = start(root)`. Une ressource peut être donnée une fois : rendue, ajoutée à un groupe, ou libérée par `release()`. Le compilateur refuse une ressource que rien ne garde, utilisée après avoir été donnée, ou gardée par un tableau ou une fonction fléchée, là où rien ne la libérerait ([E0245](../errors/E0245.md)).

En debug, le défaut de `perasc build` et du serveur de développement de Vite, une ressource ramassée sans avoir été libérée est signalée dans la console avec le fichier et la ligne qui l’ont créée.

## Pas encore

- **Davantage du navigateur** : les formulaires, `requestAnimationFrame`, les WebSockets et les workers, ajoutés quand des programmes en ont besoin.
- **Les tableaux de ressources**, et une requête comme ressource annulée avec son propriétaire.
- **Les fonctions comme valeurs**, hors des fonctions fléchées données aux méthodes des tableaux et à `on`, `setTimeout` et `setInterval` ; les fonctions fléchées à instructions.
- **Les structures et les énumérations à travers un import.**
- **Les options d’options**, refusées par [E0900](../errors/E0900.md) : en JavaScript, `Some(None)` et `None` valent tous deux `undefined`.
