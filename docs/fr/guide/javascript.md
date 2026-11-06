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

| Peras               | JavaScript                                 |
| ------------------- | ------------------------------------------ |
| `i32`, `f32`, `f64` | `number`                                   |
| `i64`               | `bigint`                                   |
| `bool`              | `boolean`                                  |
| une structure       | un simple objet, déclaré comme `interface` |

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

Le chargeur convertit à la frontière : un objet passé en argument est copié champ par champ, si bien que Peras n’en garde jamais de référence, et que les modifications faites par Peras ne l’atteignent jamais.

## Avec Vite

```ts
// vite.config.ts
import { defineConfig } from "vite";
import peras from "vite-plugin-peras";

export default defineConfig({
  plugins: [peras({ perasc: "chemin/vers/perasc", lang: "fr" })],
});
```

| Option   | Par défaut                             | Sens                                                 |
| -------- | -------------------------------------- | ---------------------------------------------------- |
| `perasc` | `PERASC`, puis `perasc` dans le `PATH` | le compilateur à lancer                              |
| `lang`   | celle du système                       | `"en"` ou `"fr"` pour les diagnostics                |
| `dts`    | `true`                                 | écrire `nom.d.peras.ts` à côté de chaque `nom.peras` |

En développement, le WebAssembly est intégré au module ; `vite build` l’émet comme un fichier séparé, compilé pendant son téléchargement. Une erreur dans un fichier `.peras` s’affiche dans l’overlay de Vite avec son extrait, sa cause et sa correction, et fait échouer `vite build`.

## Les erreurs mémoire à l’exécution

Dans un module qui utilise le tas, l’allocateur de débogage lève une `PerasMemoryError` sur un bug mémoire, avec les sites concernés dans son message, et dans `allocSite` et `releaseSite`. Un tel module exporte aussi `__perasHeap` :

```ts
import { __perasHeap } from "./parser.peras";

__perasHeap.live(); // blocs encore vivants
__perasHeap.checkLeaks(); // lève une erreur s’il en reste
```

C’est la fin du guide. La [référence](../reference/status.md) liste tout ce que le langage offre aujourd’hui.
