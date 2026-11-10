# 0007 : Des threads sur une mémoire partagée, découpée par possession

_Version anglaise, qui fait foi : [../../en/decisions/0007-threads-over-shared-memory.md](../../en/decisions/0007-threads-over-shared-memory.md)._

## Contexte

Une page web dispose de plusieurs cœurs, et un module Peras en utilise un. Le [benchmark des Web Workers](../../../bench/README.fr.md#web-workers) exécute aujourd’hui un noyau dans N workers, chacun avec sa propre instance du module et sa propre mémoire. Les données entrent et sortent par copie, et les copies décident du résultat :

- 200 pas sur un million de particules vont 2,7 à 3 fois plus vite sur 4 cœurs ;
- un passage d’un filtre sur 16 Mo de pixels va 3 fois moins vite : les copies coûtent environ 20 ms, le filtre 4 à 8.

Les threads WebAssembly suppriment les copies. Plusieurs workers instancient le même module sur une seule `WebAssembly.Memory({ shared: true })`, adossée à un `SharedArrayBuffer`, et lisent les mêmes tableaux. Avant de construire cela, cette décision mesure ce que cela peut apporter au mieux.

## Ce que d’autres ont fait

- **Rust et C++.** `wasm-bindgen-rayon` et les pthreads d’Emscripten instancient le module une fois par worker sur une seule mémoire partagée. Rayon découpe une tranche en parts disjointes que le vérificateur d’emprunts prouve indépendantes, et vole du travail entre threads.
- **Le coût pour une page.** Un `SharedArrayBuffer` n’est donné qu’à une page isolée entre origines : servie avec `Cross-Origin-Opener-Policy: same-origin` et `Cross-Origin-Embedder-Policy: require-corp`, depuis Spectre (2018). Chaque ressource d’une telle page doit l’autoriser ; certains scripts et intégrations tiers ne le font pas.
- **Ce que coûte la mémoire partagée.** Elle ne peut pas rétrécir. Son agrandissement doit être vu de tous les threads. Le fil principal ne peut pas se bloquer sur `Atomics.wait`. Un allocateur partagé entre threads demande des verrous ou une arène par thread.

## Mesure

`node scripts/bench-workers.mjs` ajoute une façon : _4 workers, données résidentes_. Chaque worker garde sa part dans la mémoire de son module, remise en l’état avant chaque exécution, hors chronomètre. Seuls l’exécution du noyau sur les 4 workers et la réponse de chacun sont chronométrées. C’est le temps de threads sur une seule mémoire partagée sans rien copier, le coût de l’allocateur partagé et de la synchronisation laissé de côté : un plafond. Chaque résultat est vérifié égal au résultat direct, élément par élément.

Sur 4 cœurs, Chromium 153, 3 exécutions de 15 médianes chacune :

| Noyau              | Direct     | 4 workers, copies | 4 workers, résidentes (plafond) |
| ------------------ | ---------- | ----------------: | ------------------------------: |
| particles, 10 pas  | 19–20 ms   |       0,80 – 0,83 |                 **2,08 – 2,22** |
| particles, 200 pas | 325–334 ms |       2,68 – 2,99 |                 **3,00 – 3,28** |
| grayscale          | 8,1–8,7 ms |       0,32 – 0,35 |                 **1,84 – 2,02** |

Accélérations face à l’appel direct sur le fil principal. Cette machine était plus lente ce jour-là que pour la première mesure des Workers (grayscale direct 8 ms au lieu de 4) ; les rapports tiennent.

Ce que cela dit :

- **Sans copies, les noyaux légers gagnent aussi.** Grayscale passe de 3 fois moins vite à 2 fois plus vite ; 10 pas de particules de 0,8 à 2,2 fois.
- **Les noyaux lourds gagnent peu de plus.** 200 pas payaient déjà leurs copies : 3 à 3,3 au lieu de 2,7 à 3.
- **4 cœurs ne donnent pas 4 fois.** Un filtre sur 16 Mo est limité par la bande passante de la mémoire, que les cœurs partagent, et réveiller 4 workers coûte une fraction de milliseconde.

## Décision

1. **Peras construit des threads sur une mémoire partagée, en option du backend.** `--threads` construit le module avec une mémoire partagée et les atomiques ; sans elle, rien ne change. Une page qui n’est pas isolée entre origines charge le module sans threads et tourne sur un cœur, avec les mêmes résultats.
2. **Les boucles parallèles sont découpées par possession, pas par verrous.** Un `parallel for` sur les indices de tableaux que la boucle emprunte les découpe en plages disjointes, une par worker. Le vérificateur ne l’accepte que si chaque tour n’écrit que les éléments de son propre indice, la règle qui permet déjà aux [boucles vectorielles](../../../CHANGELOG.fr.md) de faire 4 voies à la fois, et n’appelle rien qui alloue. Les résultats sont alors ceux de la boucle séquentielle, bit pour bit, comme pour les boucles vectorielles ; des tests comparent les deux.
3. **Une arène de l’allocateur par thread.** Chaque worker alloue dans sa propre arène TLSF de la mémoire partagée ; seul l’agrandissement de la mémoire prend un verrou. Un bloc libéré par un autre thread retourne à son arène par une liste que ce thread vide.
4. **Le chargeur tient le pool.** Il démarre `navigator.hardwareConcurrency − 1` workers une fois, sur la même mémoire, et distribue les plages ; le fil principal travaille aussi, et attend avec `Atomics.waitAsync`.

## Écarté pour l’instant

- **Le vol de travail.** Des plages égales suffisent pour des boucles dont les tours coûtent pareil ; le vol viendra avec la mesure d’une boucle qui en a besoin.
- **Déplacer des valeurs entre threads avec `sink`** ([décision 0003](0003-runtime-proved-at-compile-time.md)) : après les boucles parallèles, qui couvrent les noyaux mesurés.

## Statut

Mesuré, pas construit. Le plafond est versionné avec sa commande ; le backend vient après le parcours des `Map`.

## Conséquences

- Un module construit avec `--threads` demande une page isolée entre origines pour ses threads ; le plugin Vite posera les deux en-têtes en développement et le dira en production.
- Le texte qui entre dans le module coûte toujours une copie : les threads ne suppriment pas celle-là.
