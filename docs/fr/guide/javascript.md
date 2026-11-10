# Peras et JavaScript

Un module Peras s’utilise depuis JavaScript ou TypeScript comme n’importe quel module ES.

## Ce qu’écrit `perasc build`

Pour `geometry.peras` :

| Fichier         | Contenu                                                               |
| --------------- | --------------------------------------------------------------------- |
| `geometry.wasm` | le module WebAssembly                                                 |
| `geometry.js`   | un module ES qui le charge et exporte les fonctions marquées `export` |
| `geometry.d.ts` | les déclarations TypeScript                                           |

## Les types à la frontière

| Peras                 | JavaScript                                                                                                                                 |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `i32`, `f32`, `f64`   | `number`                                                                                                                                   |
| `i64`                 | `bigint`                                                                                                                                   |
| `bool`                | `boolean`                                                                                                                                  |
| une structure         | un simple objet, déclaré comme `interface`                                                                                                 |
| `Option<T>`           | la valeur, ou `undefined`                                                                                                                  |
| `Array<T>`            | un tableau, d’objets pour des structures, copié à l’entrée et à la sortie                                                                  |
| `String`              | une chaîne, copiée en UTF-8                                                                                                                |
| `Map<K, V>`, `Set<K>` | une `Map` ou un `Set`, copiés dans l’ordre d’insertion, à l’entrée et à la sortie ; un `inout` reçoit en retour les entrées de la fonction |

```peras build
export struct Rect {
  width: f64;
  height: f64;
}

export fn area(r: Rect): f64 {
  return r.width * r.height;
}

export fn big(r: Rect): bool {
  return area(r) > 100.0;
}
```

```ts
import { area, big } from "./shapes.peras";

area({ width: 3, height: 4 }); // 12
big({ width: 30, height: 4 }); // true
```

Le chargeur convertit à la frontière : un objet passé en argument est copié champ par champ, si bien que Peras n’en garde jamais de référence, et que les modifications faites par Peras ne l’atteignent jamais. Un tableau est copié de la même façon, élément par élément ; seul un paramètre `inout` est recopié dans le tableau passé ([collections](collections.md#en-javascript)).

## Des tableaux sans copie

Un tableau de nombres passé à une fonction exportée est copié dans la mémoire du module, et recopié en retour pour un `inout` : O(n) à chaque appel. Pour les grands tableaux, un `PerasArray` reste dans la mémoire du module et se passe sans copie. <span class="status compiled">compilé</span>

```js
import { scale, sum, __perasHeap } from "./points.peras";

const xs = __perasHeap.array("f32", 1_000_000); // des zéros ; ou depuis un tableau de nombres
xs.view().set(samples); // un Float32Array sur la mémoire du module
scale(xs, 2); // inout : modifié sur place, rien n'est recopié
const total = sum(xs); // prêté : rien n'est copié
xs.release(); // rend sa mémoire
```

- `__perasHeap.array(kind, init)` en crée un : `kind` vaut `"i32"`, `"i64"`, `"f32"`, `"f64"`, `"bool"` ou `"u8"`, et `init` est une longueur (des zéros) ou un tableau de valeurs.
- Toute fonction exportée qui prend un tableau de nombres l'accepte : prêté à un paramètre simple, modifié sur place par un `inout`, donné à un `sink`, après quoi on ne peut plus s'en servir.
- `view()` est un tableau typé sur ses éléments, sans copie. Il reste valable jusqu'au prochain appel au module, qui peut agrandir sa mémoire ou déplacer les éléments : prenez une nouvelle vue après chaque appel.
- `toArray()` le copie dans un tableau JavaScript ; `length` et `kind` le décrivent.
- `release()` rend aussitôt sa mémoire. Un tableau jamais libéré est rendu quand le ramasse-miettes le collecte.

Sur un million de `f32`, `sum` passe de 5,2 à 1,5 ms, et `scale`, qui modifie son tableau, de 11,5 à 0,8 ms : les copies faisaient l'essentiel du temps.

## Les boucles vectorielles

Une boucle dont les tours sont indépendants calcule quatre `i32` ou `f32`, ou deux `f64`, à la fois, avec les vecteurs de 128 bits de WebAssembly ; la boucle telle qu’écrite termine les derniers tours. Le résultat est exactement celui de la boucle telle qu’écrite : Peras ne vectorise que ce que chaque voie calcule de la même façon. Un `if` qui compare des nombres est vectorisé aussi : les deux branches s’exécutent dans chaque voie, et chaque voie garde le résultat de la branche qu’elle aurait prise. Dans `bench/browser`, un filtre de gris sur 4 millions de pixels passe d’environ 10 à 5 ms, deux fois plus vite que le même JavaScript.

```peras build
export fn gain(samples: inout Array<f32>, g: f32): void {
  for (let i = 0; i < samples.length; i++) {
    samples[i] = samples[i] * g;
  }
}
```

Le runtime s’en sert aussi, pour chercher un octet seize à la fois : `split` sur un million de caractères prend 35 % de temps en moins.

Le SIMD de WebAssembly est activé par défaut. Un module qui s’en sert, dans ses boucles ou son runtime, demande Chrome et Edge 91, Firefox 89, Safari 16.4, Node 16.4 ou suivants ; ailleurs, son chargeur lève une erreur qui le dit avant de charger quoi que ce soit. `perasc build --simd off`, ou `simd: false` pour Vite, compile sans lui, pour les moteurs qui n’en ont pas, comme Safari avant 16.4 sur les iPhone bloqués à iOS 15.

## Avec Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "chemin/vers/perasc", lang: "fr" })],
});
```

| Option      | Par défaut                                                         | Sens                                                                                              |
| ----------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| `perasc`    | `PERASC`, puis `perasc` dans le `PATH`                             | le compilateur à lancer                                                                           |
| `lang`      | celle du système                                                   | `"en"` ou `"fr"` pour les diagnostics                                                             |
| `dts`       | `true`                                                             | écrire `nom.d.peras.ts` à côté de chaque `nom.peras`                                              |
| `allocator` | `debug` pour le serveur de développement, `tlsf` pour `vite build` | l’allocateur des modules qui utilisent le tas                                                     |
| `simd`      | `true`                                                             | le SIMD de WebAssembly dans les boucles et le runtime ; `false` pour les moteurs qui n’en ont pas |

En développement, le WebAssembly est intégré au module ; `vite build` l’émet comme un fichier séparé, compilé pendant son téléchargement. Une erreur dans un fichier `.peras` s’affiche dans l’overlay de Vite avec son extrait, sa cause et sa correction, et fait échouer `vite build`.

## Les erreurs mémoire à l’exécution

Dans un module qui utilise le tas, l’allocateur de débogage lève une `PerasMemoryError` sur un bug mémoire, avec les sites concernés dans son message, et dans `allocSite` et `releaseSite`. Un tel module exporte aussi `__perasHeap` :

```ts
import { __perasHeap } from "./parser.peras";

__perasHeap.live(); // blocs encore vivants
__perasHeap.checkLeaks(); // lève une erreur s’il en reste
```

Un indice hors limites lève une `RangeError`, dans la langue choisie à la construction du module :

```text
RangeError: l’indice 3 est hors limites : la longueur est 3
```

C’est la fin du guide. La [référence](../reference/status.md) liste tout ce que le langage offre aujourd’hui.
