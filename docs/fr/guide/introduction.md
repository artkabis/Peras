# Introduction

Peras est un langage pour construire des applications web dont les parties critiques doivent être rapides, prévisibles et justes : rendu, simulation, audio, traitement de données, jeux. Il se lit comme TypeScript, et vérifie comme Rust.

## Deux modes, un seul langage

Un module Peras est dans l’un de deux modes, fixé par l’extension de son fichier :

| Mode                | Extension  | Compilé en  | Mémoire                                                                                                             |
| ------------------- | ---------- | ----------- | ------------------------------------------------------------------------------------------------------------------- |
| **Possédé** (owned) | `.peras`   | WebAssembly | exacte : chaque valeur a un seul propriétaire, libérée à la fin de sa portée, sans ramasse-miettes                  |
| **Géré** (managed)  | `.apeiron` | JavaScript  | le ramasse-miettes du navigateur, avec la possession appliquée aux ressources comme les écouteurs et les minuteries |

Le mode possédé sert au code où chaque milliseconde et chaque octet comptent. Le mode géré, prévu, sert au reste de l’application : le DOM, les événements, les appels réseau. Les deux partagent la syntaxe, les types et les diagnostics.

> Aujourd’hui, le mode possédé compile les fonctions sur des nombres, des booléens et des structures. Le mode géré est conçu mais pas encore construit. La [page d’état](../reference/status.md) liste chaque fonctionnalité.

## Ce qui distingue Peras

**Rien d’implicite.** Peras ne convertit jamais le type d’une valeur de lui-même : `1 + 2.5` est une erreur, et l’on écrit `1.0 + 2.5` ou `n.toF64() + 2.5`. Il n’y a ni `undefined`, ni `null`, ni valeur « truthy » : une condition est un `bool`.

**Chaque erreur bloque la compilation.** Il n’y a pas d’avertissements. Une variable inutilisée, un nom masqué, un `return` manquant : chacun est une erreur avec un code stable, un extrait de votre source, la cause, et la correction sous forme de diff.

**Des erreurs qui expliquent.** Chaque diagnostic dit pourquoi la règle existe, en français ou en anglais. `perasc explain E0006` affiche la page complète.

**Rapide là où ça compte.** Les modules possédés sont compilés en WebAssembly par l’optimiseur propre à Peras, mesuré face à Rust sur les mêmes programmes. Le [banc d’essai](https://github.com/artkabis/Peras/blob/main/bench/backends/RESULTS.md) est dans le dépôt.

**Aucune chaîne d’outils à installer.** `perasc` est un seul programme. Il écrit le module `.wasm`, un chargeur JavaScript et les déclarations TypeScript ; un plugin Vite importe les fichiers `.peras` comme n’importe quel module.

## Un premier aperçu

```peras build
// Un point est-il dans l’ensemble de Mandelbrot ?
export fn inside(cx: f64, cy: f64): bool {
  let x = 0.0;
  let y = 0.0;
  for (let step = 0; step < 64; step++) {
    if (x * x + y * y > 4.0) {
      return false;
    }
    const next = x * x - y * y + cx;
    y = 2.0 * x * y + cy;
    x = next;
  }
  return true;
}
```

Ensuite : [premiers pas](getting-started.md).
