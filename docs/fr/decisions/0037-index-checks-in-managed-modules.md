# 0037 : Comment un module managed vérifie ses indices

_Version anglaise : [../../en/decisions/0037-index-checks-in-managed-modules.md](../../en/decisions/0037-index-checks-in-managed-modules.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F221, que le profil de la [décision 0029](0029-pond-of-lanterns.md) a expliqué : le module managed du bassin, construit pour la production, prend 2,62 fois le temps du TypeScript dans Node, et 1,09 une fois ses vérifications d’indice écrites comme des accès directs. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait aujourd’hui le générateur JavaScript, lu avant d’écrire ceci (constat M020) :

- **Chaque indice non prouvé dans les bornes passe par l’une de deux fonctions** du module (`crates/perasc-codegen-js/src/runtime.rs`) : `$at(xs, i)` lit, `$put(xs, i, v)` écrit ; hors des bornes, toutes deux lèvent la `RangeError` du module owned. Un indice que le générateur prouve dans les bornes (`proven`, une boucle comptée sur la longueur du tableau) est lu directement.
- **Les deux mêmes fonctions servent chaque tableau du module** : tableaux de nombres, de chaînes, de structures, tableaux typés venus de JavaScript (un paramètre `inout Array<f64>` peut être un `Float64Array`).
- **La vérification est la promesse du langage** : un indice hors des bornes arrête le programme dans les deux modes, ne donne jamais `undefined`.

## Options

- **o0. Comme aujourd’hui** : `$at` et `$put`.
- **o1. La vérification écrite à chaque accès** : `(i >>> 0 < xs.length ? xs[i] : $bounds(i, xs.length))` pour une lecture ; pour une écriture, la valeur calculée d’abord, comme `$put` la calcule, puis le même test avant l’affectation. Une partie qui n’est pas une variable locale est gardée dans une temporaire, pour que chacune soit évaluée une fois et dans l’ordre.
- **o2. Une paire de fonctions par sorte d’élément**, selon le type que le vérificateur donne au tableau : nombres, entiers, booléens, chaînes, autres valeurs ; le même code, une fonction chacune.
- **o3. Aucune vérification dans un build de production** : `xs[i]` et `xs[i] = v`. Construite pour mesurer ce que coûte la vérification, non proposée : un indice hors des bornes donne `undefined`, contre la promesse du langage.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_INDEX` ; le build sans elle garde le comportement d’aujourd’hui, o0. Chaque module mesuré est un build de production, nommé (`--allocator tlsf`, constat M029).

1. **Justesse** : chaque script `--check` du projet et `pnpm run e2e` sous chaque option ; un test de bout en bout qui lit et écrit hors des bornes, en lecture, en écriture, en affectation composée (`xs[i] += v`) et en pas (`xs[i]++`), dont l’indice et la valeur ont des effets dont l’ordre compte : o0, o1 et o2 lèvent la même erreur après les mêmes effets ; ce que fait o3 est consigné.
2. **Vitesse et charge du processeur**, dans Node, le TypeScript ou le JavaScript écrit à la main et le module de chaque option dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports tour par tour et leur étendue (constat M017), le temps CPU sur le temps réel :
   - **W1**, le bassin, image 600 en 1024 × 768 avec 1 000 lanternes, face à `pond.ts` ;
   - **W2**, le jardin de la décision 0013 en module managed, une image en 1024 × 1024, face à sa version TypeScript ;
   - **W3**, chaque noyau de `bench/managed/kernels.apeiron`, face au JavaScript écrit à la main de `scripts/bench-managed.mjs`.
3. **Octets** : chaque module managed du dépôt, brut et en gzip, par `scripts/check-regression.mjs` sous chaque option, qui donne aussi le temps de compilation et la taille du compilateur.
4. **Facilité d’écriture** : rien à mesurer ; le même programme pour chaque option. Dit tel quel.
5. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-index.mjs` lance les charges des points 2 et 3 ; `--check` vérifie que les modules de chaque option donnent les résultats de la référence, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
