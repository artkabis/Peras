# Peras

**Un langage web avec la syntaxe de TypeScript et la rigueur de Rust.** Peras compile les modules `.peras` en WebAssembly avec une mémoire exacte, sans ramasse-miettes, et refuse de compiler tant que quelque chose ne va pas, en expliquant la cause et la correction en français ou en anglais.

```peras build
export struct Vec2 {
  x: f64;
  y: f64;
}

export fn length(v: Vec2): f64 {
  return (v.x * v.x + v.y * v.y).sqrt();
}
```

```ts
import { length } from "./geometry.peras";

length({ x: 3, y: 4 }); // 5, calculé par WebAssembly
```

## Par où commencer

- [Peras pour les décideurs](guide/overview.md) : ce qu’il apporte, mesuré, où il convient, sa maturité, et comment l’essayer sans risque.
- [Introduction](guide/introduction.md) : à quoi sert Peras, ses deux modes, ce qui le distingue.
- [Premiers pas](guide/getting-started.md) : compiler le compilateur, écrire un module, l’exécuter dans le navigateur avec Vite.
- [Le langage, pas à pas](guide/basics.md) : valeurs, fonctions, contrôle du flux, structures, collections, mémoire.
- [Ce qui fonctionne aujourd’hui](reference/status.md) : chaque fonctionnalité, compilée, vérifiée seulement, ou prévue. Peras est jeune : cette page dit exactement à quel point.

## Référence

- [Types](reference/types.md), [opérateurs](reference/operators.md), [méthodes intégrées](reference/methods.md), [mots-clés](reference/keywords.md).
- [La commande `perasc`](reference/cli.md).
- [Codes d’erreur](errors/index.md) : une page par erreur, aussi affichée par `perasc explain`.
- [Vers un langage complet](reference/roadmap.md) : ce qui vient ensuite, et dans quel ordre.

## Le projet

- [Spécification du langage](spec.md) : le langage tel qu’il est conçu, y compris ce qui n’est pas encore construit.
- [Plan de démarrage](kickoff-plan.md) et [décisions](decisions/0001-webassembly-backend.md) : comment le compilateur est construit, et pourquoi.

Chaque bloc de code de cette documentation est compilé par `perasc` dans les tests du projet : celui qui dit compiler compile, et celui qui dit être refusé l’est, avec le code indiqué.
