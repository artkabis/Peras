# Architecture technique : choix systèmes et valeur ajoutée

_Version anglaise, qui fait foi : [../en/architecture.md](../en/architecture.md)._

Ce document de référence justifie les décisions d’ingénierie logicielle et matérielle du projet Peras. Il sert de cahier des charges aux développeurs, de grille de validation aux auditeurs techniques et de présentation de la rupture technologique visée auprès des investisseurs et des organismes publics.

Chaque choix porte un **statut** :

- <span class="status compiled">fait</span> : dans le compilateur aujourd’hui, testé ;
- <span class="status checked">en cours</span> : une partie existe ;
- <span class="status planned">prévu</span> : dans la [feuille de route](reference/roadmap.md) ou la [spec](spec.md), pas encore dans le langage ;
- **à décider** : proposé ici, pas encore dans la feuille de route ; une décision écrite (comme [0001](decisions/0001-webassembly-backend.md)) le tranchera.

Les **notes d’ingénierie** disent ce que la plateforme web permet réellement, pour que chaque promesse faite à un client ou un financeur soit tenable. Les gains chiffrés sont des objectifs, à mesurer avec `bench/backends` avant d’être annoncés comme des résultats.

La décision [0003](decisions/0003-runtime-proved-at-compile-time.md) tranche l’usage de ce document : Peras est un runtime prouvé à la compilation, pas un micro-OS. Elle liste ce qui est retenu, ce qui l’est sous une autre forme, et ce qui ne l’est pas.

## I. Compilation et analyse statique (`perasc`)

### 1. Un compilateur en Rust sûr

**Détail technique.** Tout le pipeline de compilation (analyse lexicale, analyse syntaxique, vérification des types et de la possession, représentation intermédiaire, génération de WebAssembly) est écrit en Rust, sans aucun bloc `unsafe`.

**Valeur ajoutée.** Pas de corruption mémoire possible dans l’outil lui-même. Le binaire `perasc` tourne à vitesse native et pourra être compilé en WebAssembly pour s’exécuter dans un navigateur ou sur des serveurs Edge.

**Statut.** <span class="status compiled">fait</span> : `unsafe_code = "forbid"` pour tout l’espace de travail, avec zéro avertissement toléré. Le compilateur dans le navigateur est <span class="status planned">prévu</span> (phase 7, le bac à sable de la documentation).

### 2. Deux cibles de compilation : `.peras` et `.apeiron`

**Détail technique.** À partir d’une syntaxe unique inspirée de TypeScript, le compilateur produit soit du WebAssembly (`.peras`), soit du JavaScript (`.apeiron`).

**Valeur ajoutée.** On choisit la cible selon la nature du traitement, sans réécrire le code métier.

**Statut.** `.peras` <span class="status compiled">fait</span>. `.apeiron` <span class="status planned">prévu</span>, phase 4.

**Note d’ingénierie.** La [spec](spec.md#mode-managed) fait du mode géré la cible du DOM, d’`async` et des API du navigateur, en JavaScript ordinaire. Un JavaScript « manipulé exclusivement par indexation sur des tableaux typés » est un autre mode : **à décider**. Les deux peuvent coexister, le second comme repli de `.peras` là où WebAssembly n’est pas disponible.

### 3. Types affines et vérification de la possession (`perasc-check`)

**Détail technique.** À la compilation, chaque valeur a un seul propriétaire ; elle se déplace à l’affectation, au `return` et avec `sink`, et elle est empruntée sinon. Un usage après déplacement bloque la compilation ([E0204](errors/E0204.md)), comme deux arguments qui atteignent une même variable que l’un modifie ([E0207](errors/E0207.md)).

**Valeur ajoutée.** Pas d’usage après libération, de double libération ni de fuite : ni ramasse-miettes ni compteur de références à l’exécution.

**Statut.** <span class="status compiled">fait</span>. La descente insère chaque libération, et un vérificateur de libérations prouve que chaque valeur possédée est libérée exactement une fois sur chaque chemin.

**Note d’ingénierie.** Peras suit la sémantique de valeurs mutables (Swift, Hylo), pas les références de Rust. Aucune référence n’est stockée dans une donnée : il n’y a donc **pas de durée de vie à écrire, ni à inférer**. Le développeur n’écrit que `inout`, `sink` et `&` à l’appel. Le chiffre de « 80 % d’inférence » n’a donc pas d’objet : ce qui se mesure, c’est la part de programmes qui compilent sans aucune annotation.

### 4. Réordonnancement des champs et bourrage automatique

**Détail technique.** Le compilateur trie les champs du plus large au plus étroit (`v128`, `f64`, `u32`, `u8`) et aligne chacun selon $A_{align} = (A_{raw} + (N - 1)) \ \& \ \sim(N - 1)$, en insérant les octets de bourrage nécessaires.

**Valeur ajoutée.** Pas de pénalité d’accès non aligné, et des enregistrements plus compacts.

**Statut.** **À décider.**

**Note d’ingénierie.** Aujourd’hui, aucune structure n’a de disposition en mémoire. Une structure est remplacée par ses scalaires (des variables locales, des résultats multiples), et un tableau de structures est stocké en colonnes (voir 5) : il n’y a ni bourrage ni accès non aligné. Le réordonnancement servira le jour où des enregistrements seront placés en mémoire : pixels d’un `Framebuffer`, tampons partagés avec WebGPU, `sizeOf<T>()`. La spec promet alors une **disposition visible et fixe**. Pour l’interopérabilité, il faudra donc un attribut qui fige l’ordre écrit (comme `#[repr(C)]` en Rust). Le gain de mémoire face aux objets de V8 est à mesurer, pas à annoncer.

### 5. Structure de tableaux (SoA) et vectorisation SIMD

**Détail technique.** Les tableaux d’objets sont rangés en colonnes contiguës, une par propriété, et les boucles sur ces colonnes sont compilées en instructions vectorielles WebAssembly `v128`.

**Valeur ajoutée.** Des lignes de cache pleines de données utiles, et 2 à 16 valeurs traitées par instruction selon leur taille.

**Statut.** Colonnes <span class="status compiled">fait</span> : `Array<Point>` garde un tampon par champ, et `points[i].x` ou une boucle sur un champ ne touche qu’un tampon. SIMD <span class="status planned">prévu</span>, phase 6.

**Note d’ingénierie.** WebAssembly SIMD a des registres de 128 bits : 4 `f32`, 2 `f64` ou 16 `u8` par instruction. Le facteur de 4 à 8 est atteignable sur des noyaux de calcul en `f32` ou sur des entiers étroits, pas en général. Le trait « 256 bits » de la RFC n’existe pas en WebAssembly.

### 6. Fusion de boucles

**Détail technique.** Des opérations chaînées sur une collection (`.map().filter().reduce()`) sont combinées en une seule boucle.

**Valeur ajoutée.** Moins de lectures et d’écritures en mémoire, et pas de tableau intermédiaire alloué.

**Statut.** <span class="status planned">prévu</span> : `map`, `filter` et `reduce` arrivent avec la bibliothèque standard (phase 3). La fusion se fera dans l’IR, ou en compilant ces méthodes comme des itérateurs paresseux.

### 7. Indexation spatiale par courbes de remplissage

**Détail technique.** Les index des grilles en 2D ou en 3D sont entrelacés bit à bit (ordre de Morton, ou Z-order), pour que des cases voisines dans l’espace soient voisines en mémoire.

**Valeur ajoutée.** Bien plus de succès de cache lors des parcours de voisinage.

**Statut.** **À décider.**

**Note d’ingénierie.** Morton et Hilbert sont deux courbes différentes. Morton se calcule en quelques instructions, par entrelacement de bits. Hilbert garde une meilleure localité, mais coûte plus cher à calculer. Changer l’ordre des index change le sens de `grid[i]`, et un parcours ligne par ligne devient alors plus lent : ce sera un type de la bibliothèque (`Grid2D` en ordre de Morton), choisi explicitement, pas une transformation automatique. Le taux de cache proche de 100 % dépend du parcours.

## II. Noyau virtuel et gestion de la mémoire (`peras-kernel`)

### 8. MMU logicielle et pages de 4 Ko

**Détail technique.** La mémoire est découpée en pages virtuelles de 4 Ko. Le code manipule des handles de 32 bits (20 bits de page, 12 bits de décalage), traduits en temps constant par une table de pages.

**Valeur ajoutée.** Déplacer et compacter les blocs en arrière-plan sans invalider les références du développeur.

**Statut.** **À décider.**

**Note d’ingénierie.**

- La traduction ajoute une lecture de table et un calcul à **chaque** accès mémoire, boucles chaudes comprises.
- Peras a une meilleure prise sur ce problème : comme aucune référence n’est stockée dans une donnée, chaque tampon n’a qu’un propriétaire, et donc un seul endroit à mettre à jour s’il se déplace.
- Les tableaux ont déjà un en-tête de 16 octets qui garde l’adresse de leurs éléments : les éléments peuvent bouger, l’en-tête reste. Un compacteur peut ainsi déplacer les données sans table de pages, avec une seule indirection, que l’IR sait sortir des boucles.
- Compacter pendant les temps morts du navigateur est faisable entre deux appels exportés, quand aucun tampon n’est prêté.

### 9. Allocateur déterministe

**Détail technique.** Des slabs pour les petits objets de taille fixe, un index TLSF (_Two-Level Segregated Fit_) qui trouve un bloc libre grâce à `i32.clz`, et un allocateur buddy (blocs en puissances de 2) pour les grands tableaux.

**Valeur ajoutée.** Allocation et libération en temps borné, et une fragmentation maîtrisée.

**Statut.** TLSF <span class="status compiled">fait</span> ([décision 0002](decisions/0002-tlsf-allocator.md)) : temps borné, blocs fusionnés aussitôt, lié par `vite build` et `--allocator tlsf`. L’allocateur de débogage reste pour le développement : mémoire empoisonnée, zones rouges, quarantaine, fuites comptées, et chaque erreur nomme son site d’allocation et de libération.

**Note d’ingénierie.**

- TLSF sert à lui seul les tailles variables en O(1) ; le combiner avec un buddy est redondant, et il faudra en choisir un par classe de tailles.
- Aucun allocateur ne garantit **0 % de fragmentation**. Le buddy arrondit chaque bloc à une puissance de 2 (jusqu’à près de 50 % de perte interne). TLSF borne la fragmentation sans l’annuler. Seuls les slabs n’ont pas de fragmentation externe, et seulement au sein d’un slab.
- La promesse tenable : temps borné, fragmentation bornée, et compactage pour la reprendre (voir 8).

### 10. Pages de garde

**Détail technique.** Des pages inaccessibles entre les zones de mémoire interceptent les débordements.

**Valeur ajoutée.** Un débordement est arrêté avant de corrompre les données voisines.

**Statut.** Équivalent logiciel <span class="status compiled">fait</span>.

**Note d’ingénierie.**

- La mémoire linéaire de WebAssembly n’a **aucune protection par page** : un accès dans ses limites n’est jamais intercepté par le matériel. Une page de garde ne peut donc être qu’une vérification logicielle.
- C’est ce que Peras fait déjà : chaque index est vérifié avant tout accès (`RangeError`), et en développement, chaque bloc a une zone rouge de 16 octets vérifiée à la libération.
- Le langage interdit l’arithmétique de pointeurs : un débordement ne peut venir que d’un bug du compilateur, que le vérificateur de libérations et le fuzzing traquent.
- Redémarrer un module défaillant est possible au niveau d’un Web Worker.

### 11. Ordonnanceur coopératif (fibres)

**Détail technique.** Des fibres légères dans le runtime découpent les tâches longues en tranches de temps.

**Valeur ajoutée.** Une interface fluide sans bloquer le fil principal du navigateur.

**Statut.** **À décider.**

**Note d’ingénierie.** WebAssembly ne sait pas encore changer de pile. Trois voies sont possibles : JSPI (_JavaScript Promise Integration_, en cours de standardisation), la proposition _stack switching_, ou une transformation des fonctions par le compilateur (machine à états, comme `async` en Rust). La dernière marche partout, mais n’autorise une pause qu’aux points marqués. Pour du calcul lourd, des Web Workers donnent un vrai parallélisme sans fibres.

## III. Interopérabilité, réseau et matériel

### 12. Transfert entre fils sans copie

**Détail technique.** Des pages en lecture seule partagées dans un `SharedArrayBuffer`, dupliquées seulement à la première écriture (copie sur écriture).

**Valeur ajoutée.** Remplacer la sérialisation de `postMessage`, en O(n), par un transfert en O(1).

**Statut.** **À décider.**

**Note d’ingénierie.**

- Ni JavaScript ni WebAssembly ne détectent une écriture dans une page : la copie sur écriture demanderait une vérification compilée avant chaque écriture.
- Le modèle de Peras offre mieux. Une valeur passée en `sink` change de propriétaire : l’envoyer à un worker est un **déplacement sans copie**, sûr, puisque l’expéditeur ne peut plus la lire. Une valeur seulement lue peut être partagée sans copie, puisque personne ne l’écrit.
- Les `ArrayBuffer` transférables de `postMessage` sont déjà en O(1).
- Un `SharedArrayBuffer` exige l’isolation inter-origines (en-têtes COOP et COEP) : une contrainte de déploiement à documenter.

### 13. Tampons circulaires sans verrou

**Détail technique.** Des files FIFO de taille puissance de 2 (masque `& (n - 1)` au lieu de `%`), synchronisées par `Atomics.load` et `Atomics.store`.

**Valeur ajoutée.** Des flux réseau et audio à haute fréquence, sans allocation ni verrou.

**Statut.** **À décider**, faisable tel quel.

**Note d’ingénierie.** C’est le schéma standard à un producteur et un consommateur, celui des `AudioWorklet`. Avec plusieurs producteurs, il faut une compare-and-swap (`Atomics.compareExchange`). `Atomics` opère sur des tableaux typés entiers (`Int32Array`), pas sur l’`ArrayBuffer` lui-même.

### 14. Pont vers WebGPU

**Détail technique.** Des tampons alignés sur 256 octets, l’alignement que WebGPU exige pour les décalages des tampons _uniform_ et _storage_.

**Valeur ajoutée.** Des données prêtes pour le GPU, sans réorganisation.

**Statut.** <span class="status planned">prévu</span>, phase 6.

**Note d’ingénierie.** WebGPU ne lit pas directement la mémoire de JavaScript ni celle de WebAssembly : les données passent par `writeBuffer` ou par un tampon mappé, donc **au moins une copie**, même sur une puce à mémoire unifiée, où le navigateur garde la main. L’alignement de 256 octets supprime la réorganisation, pas la copie. Promesse tenable : une seule copie, sans conversion.

## IV. Outillage, sécurité et télémétrie

### 15. Linter d’invariants

**Détail technique.** Une analyse au build qui refuse les motifs qui allouent et les mutations partagées non atomiques.

**Valeur ajoutée.** La preuve, avant déploiement, qu’une partie du code n’alloue pas.

**Statut.** <span class="status planned">prévu</span>, par la règle `@frame` de la spec : une fonction `@frame` ne peut ni allouer, ni appeler une fonction qui alloue, ce que le compilateur prouve, ou il refuse le build.

**Note d’ingénierie.**

- Dans un module `.peras`, aucun objet JavaScript n’existe : `push` agrandit un tampon dans la mémoire de WebAssembly, sans jamais solliciter le ramasse-miettes du navigateur. Interdire `push` partout retirerait les tableaux du langage. La règle juste porte sur les fonctions `@frame`, et sur le code JavaScript de liaison, qui crée aujourd’hui des objets aux frontières.
- Les mutations partagées non atomiques sont déjà impossibles : `inout` est exclusif ([E0207](errors/E0207.md)).
- ISO 27001 et DORA certifient l’organisation d’une entreprise, pas un programme. Ces preuves peuvent alimenter ses audits, sans les remplacer.

### 16. Inspecteur mémoire et profilage énergétique

**Détail technique.** Une carte thermique de la mémoire, une frise qui rejoue la vie des valeurs (allocation, prêt, déplacement, libération), et la lecture des compteurs d’énergie du processeur (RAPL).

**Valeur ajoutée.** Un débogage bas niveau bien plus rapide, et des rapports d’économie d’énergie.

**Statut.** Inspecteur **à décider**. Ses données existent déjà : l’allocateur de débogage connaît chaque bloc vivant et son site d’allocation, et E0204 retrace l’histoire d’une valeur.

**Note d’ingénierie.** Aucun navigateur n’expose RAPL. Même nativement, sous Linux, ses compteurs demandent des droits élevés, et ils mesurent tout le processeur, pas un onglet. Le profilage énergétique est donc faisable sur un banc de mesure natif ou serveur, pas chez l’utilisateur. Les rapports RSE devront s’appuyer sur ces bancs, avec leur méthode publiée.

## Tableau récapitulatif

| Choix technique         | Mécanisme clé                                    | Impact matériel                    | Valeur économique              | Statut                                    |
| ----------------------- | ------------------------------------------------ | ---------------------------------- | ------------------------------ | ----------------------------------------- |
| Possession vérifiée     | Types affines, libérations insérées et vérifiées | Pas de ramasse-miettes             | Pas de pause, pas de fuite     | <span class="status compiled">fait</span> |
| Tableaux en colonnes    | Un tampon par champ                              | Caches pleins de données utiles    | Calculs plus rapides           | <span class="status compiled">fait</span> |
| SIMD                    | Instructions `v128`                              | 2 à 16 valeurs par instruction     | Calculs plus rapides           | <span class="status planned">prévu</span> |
| Allocateur déterministe | TLSF, `i32.clz`                                  | Temps borné, fragmentation bornée  | Applications lourdes stables   | <span class="status compiled">fait</span> |
| Compactage              | Déplacement des éléments derrière leur en-tête   | Mémoire reprise à chaud            | Pas de saturation sur la durée | **à décider**                             |
| Transfert entre fils    | `sink` : déplacement sans copie                  | O(1) au lieu de O(n)               | Traitement en arrière-plan     | **à décider**                             |
| Alignement WebGPU       | Tampons alignés sur 256 octets                   | Une copie, sans conversion         | 3D et IA locales               | <span class="status planned">prévu</span> |
| `@frame`                | Preuve d’absence d’allocation                    | Pas de croissance mémoire au rendu | Images régulières              | <span class="status planned">prévu</span> |
| Profilage énergétique   | RAPL sur banc natif                              | Mesure reproductible               | Rapports GreenTech             | **à décider**                             |

Ce que coûtent ces choix, et dans quel ordre ils arrivent : la [feuille de route](reference/roadmap.md). Chaque choix **à décider** passera par une décision écrite, avec mesures, comme [0001](decisions/0001-webassembly-backend.md).
