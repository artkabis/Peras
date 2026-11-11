# 0015 : Le flux de capteurs, mesuré

_English version: [../../en/decisions/0015-stream-measured.md](../../en/decisions/0015-stream-measured.md)._

_Statut : protocole enregistré le 2026-11-11, avant toute mesure. Les résultats seront ajoutés en dessous, qu’ils confirment les hypothèses ou non ; le protocole ne changera pas après la première mesure, et tout écart sera listé comme tel._

## Contexte

L’[application de flux](../stream.md) montre, en direct, à quelle vitesse Peras traite un flux de mesures de capteurs à côté du même calcul en TypeScript. Ce qu’elle montre sert à observer, pas à affirmer : une machine, un moment, une seule exécution, une moyenne de ce qui s’est passé, et seulement le TypeScript écrit simplement. Cette étude mesure le même travail comme le ferait un ingénieur avant de décider : des répétitions indépendantes, un concurrent écrit pour la vitesse, des distributions plutôt que des moyennes, des intervalles plutôt que des nombres seuls, et des seuils fixés avant la première mesure.

Elle pose quatre questions :

1. Combien de lignes par seconde Peras traite-t-il, face au TypeScript écrit simplement et au TypeScript écrit pour la vitesse ?
2. La vitesse tient-elle pour de petits lots, où chaque appel vers WebAssembly coûte son prix fixe ?
3. Quelle part du temps de Peras est la frontière, le texte copié dans la mémoire du module, plutôt que le calcul ?
4. Son temps est-il régulier d’un appel à l’autre, ou y a-t-il des appels lents ?

## Le travail

Le calcul de l’[application de flux](../stream.md), inchangé : un lot de lignes `ts,sensor,zone,temp,humidity,watts` en texte CSV, généré à partir d’une graine (`examples/stream/src/generate.ts`) ; environ une ligne sur 400 cassée exprès, de cinq façons. Pour chaque lot : les lignes lues, refusées et gardées ; pour chaque zone, les mesures, les alertes, les sommes, le minimum et le maximum de température, l’humidité et l’énergie ; un histogramme des températures ; la mesure la plus chaude. Son texte canonique (`canonical` dans `examples/stream/src/digest.ts`) doit être le même dans chaque version, et les lignes refusées doivent être exactement celles que le générateur a cassées.

## Les versions

- **A. TypeScript idiomatique** : `examples/stream/src/digest.ts`, celui de l’application : `split`, expressions régulières, `Number`, `BigInt` pour l’intervalle d’un entier, une `Map` d’objets.
- **B. TypeScript optimisé à la main** : `bench/stream/digest-optimized.ts`. Le texte est parcouru une fois par codes de caractères, sans `split`, sans expression régulière ni tableau par ligne ; le texte d’un nombre est vérifié à la main et lu par `Number` seulement une fois qu’on sait que c’en est un.
- **C. Peras** : `examples/stream/src/telemetry.peras`, le module de l’application, compilé avec `perasc build --allocator tlsf`, le build de production.
- **X. La frontière seule** : `bench/stream/boundary.peras`, une fonction qui prend le texte et rend sa longueur, compilée de la même façon. Son temps est le coût du passage du texte dans un module Peras, mesuré seul, pour séparer le temps de C entre la frontière et le calcul.

A, B et C donnent le même résultat ; c’est vérifié sur chaque lot de chaque réserve avant de prendre le moindre temps.

## Plan

- **Tailles** : des lots de 1 000, 10 000 et 100 000 lignes, environ 38 Ko, 380 Ko et 3,8 Mo de texte.
- **Réserve** : 64, 32 et 12 lots distincts par taille, lus à tour de rôle, pour qu’aucune version ne voie toujours le même texte.
- **Processus** : 10 processus de navigateur indépendants, chacun un nouveau Chromium lancé par Playwright, l’un après l’autre. Le processus est l’unité de répétition : ce qu’un processus partage (son JIT, son tas, l’état de la machine) n’est pas compté comme indépendant.
- **Tours** : dans chaque processus et à chaque taille, 200, 40 et 10 tours de chauffe, écartés, puis 800, 200 et 60 tours mesurés. Dans un tour, A, B, C et X traitent chacun le même lot, dans un ordre qui tourne selon un carré latin de quatre ordres : chaque version passe première, deuxième, troisième et quatrième aussi souvent.
- **Horloge** : `performance.now()` dans une page servie avec `Cross-Origin-Opener-Policy: same-origin` et `Cross-Origin-Embedder-Policy: require-corp`, isolée entre origines, dont l’horloge avance par pas de 5 µs au lieu de 100 ; le pas est mesuré et consigné. La page exécute les versions sur son fil principal ; rien d’autre n’y tourne.
- **Non contrôlé** : le ramasse-miettes, qui passe quand il passe, dans les appels où il tombe ; les niveaux du JIT après la chauffe ; la charge du reste de la machine.

## Mesures

- **Temps par lot** : la durée de chaque appel en millisecondes. Par version, taille et processus, sa médiane ; par version et taille, la médiane des 10 médianes de processus, leur étendue et leur coefficient de variation.
- **Lignes par seconde** : la taille du lot divisée par le temps médian.
- **Centiles** : les 50e, 95e et 99e de tous les appels de tous les processus réunis.
- **Rapport de vitesse** : combien de fois C est plus rapide que A (ou B) : par processus, la médiane de A sur celle de C ; sur les processus, leur moyenne géométrique.
- **Part de la frontière** : par processus, la médiane de X sur celle de C ; sur les processus, leur moyenne.
- **Petit sur grand** : les lignes par seconde de C à 1 000 lignes sur ses lignes par seconde à 100 000 lignes.
- **Queue** : le 99e centile de C sur sa médiane, à 100 000 lignes, tous appels réunis.

**Intervalles** : à 95 %, par un bootstrap hiérarchique : 10 000 rééchantillonnages, graine 15 ; dans chacun, les processus sont tirés avec remise, puis les appels de chaque processus tiré ; la statistique est calculée sur le rééchantillon ; l’intervalle va du 2,5e au 97,5e centile des 10 000 valeurs. Pour petit sur grand, les processus de chaque taille sont tirés indépendamment.

## Hypothèses

Chacune avec son seuil. **Règle de décision** : une hypothèse _tient_ quand tout son intervalle à 95 % est du bon côté de son seuil ; elle est _réfutée_ quand tout son intervalle est du mauvais côté ; sinon elle est _indéterminée_, et rapportée comme telle, jamais comme tenant. Si une version donne un résultat différent sur un seul lot, toutes les hypothèses sont indéterminées.

- **H1, Peras bat TypeScript tel qu’on l’écrit.** C est au moins 1,5 fois plus rapide que A, à 10 000 et à 100 000 lignes. Les deux doivent tenir.
- **H2, Peras s’approche de TypeScript écrit pour la vitesse.** C est au moins 0,8 fois aussi rapide que B, à 10 000 et à 100 000 lignes. Les deux doivent tenir.
- **H3, les petits lots coûtent peu de plus.** Les lignes par seconde de C à 1 000 lignes valent au moins 0,75 fois ses lignes par seconde à 100 000 lignes.
- **H4, la frontière coûte peu.** X fait au plus 15 % du temps de C à 100 000 lignes.
- **H5, régulier.** Le 99e centile de C vaut au plus 1,5 fois sa médiane, à 100 000 lignes.

Les seuils ont été écrits dans le script avant sa première exécution. Un essai à blanc avec 2 processus et peu de tours a ensuite été fait pour tester le script et les définitions, comme l’a appris la décision 0014 (constat M003) ; ses nombres ont été vus. Ce ne sont pas des résultats, ils ne sont pas gardés, et les seuils n’ont pas été changés après eux : c’est une menace à la validité, listée plus bas.

## Figures

Écrites par le script à partir de `results.json`, jamais dessinées à la main ; chacune dit sous elle ce qu’elle montre, avec le nombre de processus, le commit et la machine.

- **Lignes par seconde, par taille de lot.** Une barre par version et taille à la médiane sur les processus ; un point pour la médiane de chaque processus, pour voir la dispersion. Plus haut est plus rapide.
- **Combien de fois C est plus rapide.** Pour C sur A et C sur B à chaque taille : un point au rapport, un trait sur son intervalle à 95 %, des tirets au seuil enregistré, et une ligne à 1×, où les deux vont aussi vite. Un intervalle entièrement à droite de ses tirets est une hypothèse qui tient.
- **La distribution du temps par ligne**, à 100 000 lignes : pour chaque version, la part des appels (verticalement) terminés en deçà d’un temps par ligne (horizontalement). Une courbe plus à gauche est plus rapide ; une courbe raide est régulière ; un long plat vers la droite est une queue d’appels lents.
- **Où va le temps de Peras.** Le temps médian par ligne de C à chaque taille, séparé entre X, la frontière, et le reste, le calcul.

## Menaces à la validité

- **Une machine**, un conteneur partagé dans le cloud avec 4 cœurs ; la charge de ses voisins n’est pas contrôlée. Les 10 processus et les intervalles mesurent la variation qu’elle cause ; ils ne font pas de ces nombres ceux d’une autre machine.
- **Un moteur** : V8, celui de Chromium. Les moteurs WebKit et Gecko ne sont pas mesurés ici.
- **Les auteurs ont écrit chaque version**, B compris. Un autre ingénieur pourrait écrire un B plus rapide, ou un C plus rapide.
- **Des données synthétiques**, générées ; de vraies mesures peuvent être plus courtes, plus longues, ou cassées autrement.
- **Le fil principal** : l’application exécute son calcul dans un worker. L’étude tourne sur le fil principal de la page, même moteur et même compilateur, sans les messages du worker, que les chiffres de l’application incluent.
- **Les nombres de l’essai à blanc ont été vus** avant cet enregistrement.
- **Le ramasse-miettes** tombe dans certains appels et pas dans d’autres ; les centiles l’incluent, comme un utilisateur le verrait.

## Reproduire

```sh
cargo build --release -p perasc
node scripts/bench-stream.mjs --check   # les versions concordent, dans Node
node scripts/bench-stream.mjs           # la mesure : environ 5 minutes
```

Le script écrit `bench/stream/RESULTS.md`, `bench/stream/results.json`, avec le temps de chaque appel, et les quatre figures dans `docs/assets/stream-*.svg`.
