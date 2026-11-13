# 0022 : Des modules owned qui s’importent

_Version anglaise, qui fait foi : [../../en/decisions/0022-owned-modules.md](../../en/decisions/0022-owned-modules.md)._

_État : proposée le 2026-11-12, avec ses critères enregistrés avant tout code ; acceptée par le propriétaire le même jour ; construite, ses critères tenus (voir [Registre](#registre)) ; étape 3 de la [décision 0021](0021-phases-4-and-5.md). Les génériques, étape 3b, sont à venir._

## Contexte

Un module owned (`.peras`) tient en un seul fichier. Chaque fonction, structure et énumération dont un programme a besoin est dans ce fichier. Un module managed (`.apeiron`) peut déjà importer depuis des modules owned ([décision 0008](0008-managed-mode.md), [décision 0021](0021-phases-4-and-5.md), étape 1). Un module owned ne peut rien importer : `import` dans un fichier `.peras` est refusé comme « réservé au mode managed ».

Une application d’une taille raisonnable ne tient pas dans un seul fichier. La phase 5 de la [feuille de route](../reference/roadmap.md) cite en premier `import` et `export` entre modules Peras. Une application de référence, écrite en Peras pour voir si une application assez standard peut être construite et déployée avec, en a besoin aussi.

## Ce que d’autres ont fait

- **Les modules ECMAScript.** `import { a } from "./b.js"` nomme un fichier par un chemin relatif et ne prend que ce que le fichier exporte. Un cycle est permis et résolu à l’exécution, ce qui laisse un module lire une liaison avant qu’elle soit posée (une `ReferenceError` dans la zone morte temporelle).
- **Rust.** Une crate est l’unité de compilation, et ses modules sont des fichiers en son sein. Les cycles entre modules sont permis, parce que la crate est vérifiée d’un bloc. Les cycles entre crates sont refusés.
- **Go.** Un paquet est un dossier. Les cycles d’import sont refusés par le compilateur, avec le chemin du cycle affiché. La règle est défendue comme ce qui garde les builds rapides et les dépendances lisibles.
- **Zig.** `@import("file.zig")` fait du fichier une structure de déclarations. Tout ce qu’un build atteint est compilé en une seule unité, et seul ce qui est atteint est analysé.

## Décision

1. **La syntaxe est celle que les modules managed utilisent déjà.** C’est `import { area, Size } from "./shapes.peras";` en tête de fichier. Le chemin est relatif au fichier qui importe et nomme un fichier `.peras`, extension comprise. Un module owned ne peut pas importer un fichier `.apeiron` : le code managed se compile en JavaScript, qu’un module owned ne peut pas appeler.
2. **`export` garde son sens et en gagne un.** Une fonction, une structure ou une énumération exportée peut être importée par un autre module owned. Ce qui atteint JavaScript est ce qu’exporte le fichier qu’on construit, comme aujourd’hui. Un fichier construit seul garde exactement l’interface qu’il a aujourd’hui, même si un autre fichier l’importe.
3. **Un build, un module WebAssembly.** `perasc build main.peras` suit les imports. Chaque fichier atteint est compilé dans le même module, avec une seule mémoire, un seul allocateur et un seul runtime. Une valeur passée d’un fichier à l’autre ne franchit aucune frontière et n’est pas copiée. Les déplacements et les emprunts sont vérifiés d’un fichier à l’autre comme au sein d’un seul.
4. **Chaque fichier est vérifié seul, contre les signatures qu’exportent les fichiers qu’il importe.**
   - Ses noms privés restent les siens : deux fichiers peuvent chacun avoir une fonction `helper`.
   - Les fichiers sont abaissés en un seul module avant que les optimisations des décisions 0016 à 0018 s’exécutent, donc ces optimisations voient les fonctions de chaque fichier. Le compilateur n’inline aucun appel dans une autre fonction, hormis une récursion qu’il déroule ; le moteur WebAssembly peut inliner un appel d’un fichier à l’autre, comme au sein d’un seul, puisqu’ils forment un seul module. (Corrigé à l’arrivée : le premier jet disait que les optimisations du compilateur comprenaient l’inlining.)
5. **Les cycles sont refusés** (nouvelle erreur `E0250`), avec le chemin du cycle : `a.peras → b.peras → a.peras`.
   - C’est la règle de Go, choisie pour les mêmes raisons. L’ordre de vérification est l’ordre du graphe, et un lecteur sait toujours quel fichier dépend de quel autre.
   - Un cycle dont une conception a besoin se casse en déplaçant ce que les deux fichiers partagent dans un troisième.
6. **Les noms ne sont pas renommés.** Un nom importé qui entre en conflit avec un nom que le fichier déclare, ou avec un nom importé d’un autre fichier, est refusé (`E0244`, `clash`), comme en mode managed. `import { a as b }` ne fait pas partie de cette décision.
7. **Ce qui passe dans cette première version :** les fonctions, structures et énumérations non génériques.
   - Les fonctions et structures génériques, et les constantes de module, viennent dans une seconde étape, 3b. Un générique est refait pour chaque type avec lequel il sert, depuis sa source, donc le fichier qui importe a besoin de cette source et pas seulement d’une signature.
   - Jusqu’à l’étape 3b, l’import d’un générique ou d’une constante est refusé (`E0244`) avec cette raison.
8. **L’outillage suit le graphe.**
   - `perasc build --json` liste les fichiers dont le module est fait, et le plugin Vite surveille chacun d’eux. Changer un fichier importé reconstruit ce qui l’importe.
   - Un diagnostic dans un fichier importé nomme ce fichier et montre sa source.
   - Un module managed qui importe un module owned qui en importe d’autres n’a besoin de rien de plus : le fichier owned est construit comme racine de son propre graphe.

## Non retenu

- **Permettre les cycles.** Rust le peut parce qu’une crate est vérifiée d’un bloc. ECMAScript le peut parce qu’il résout les cycles à l’exécution, au prix de lire une valeur avant qu’elle soit posée. Peras vérifie un fichier à la fois, contre des signatures, et n’a pas d’exécution dans laquelle résoudre quoi que ce soit. Refuser les cycles peut s’assouplir plus tard ; les permettre ne pourrait pas se reprendre.
- **Un module WebAssembly par fichier, liés au chargement.** Chaque fichier aurait sa propre mémoire et son propre allocateur. Chaque structure ou chaîne passée d’un fichier à l’autre serait copiée à travers une frontière, ce que la décision 0020 a retiré pour le texte venu de JavaScript.
- **Renommer avec `as`.** C’est utile, mais pas nécessaire pour découper un programme. Sans lui, un conflit est une erreur, donc rien n’est perdu à l’ajouter plus tard.

## Conséquences

- Deux modules `.apeiron` qui importent deux racines owned partageant un fichier reçoivent chacun une copie du code de ce fichier, dans deux modules WebAssembly à deux mémoires. Les valeurs ne passent de l’un à l’autre que par JavaScript. Partager un module entre racines est hors du périmètre.
- **La taille du module.**
  - Les fichiers sont compilés en un seul module. Une fonction qu’aucun export n’atteint est gardée, comme aujourd’hui dans un seul fichier : aucune passe du compilateur ne retire encore de telles fonctions (corrigé avant tout code : le premier jet les disait retirées). Un fichier importé apporte toutes ses fonctions, utilisées ou non ; retirer celles qu’aucun export n’atteint est un autre chantier.
  - Un programme découpé en fichiers devrait donner le même module que le même programme en un seul fichier, à l’ordre des fonctions près.
  - C’est vérifié par les tests ci-dessous plutôt que supposé.

## Critères, enregistrés avant tout code

L’étape n’arrive qu’avec tous ceux-ci, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape) :

1. **Des tests UI pour chaque refus**, avec le diagnostic du fichier qui importe :
   - un fichier introuvable ;
   - un fichier qui ne se compile pas, signalé dans ce fichier ;
   - un nom non exporté ;
   - un conflit avec un nom local ou importé ;
   - un générique ou une constante (jusqu’à l’étape 3b) ;
   - un fichier `.apeiron` importé ;
   - un cycle de deux fichiers et un cycle de trois (`E0250`, avec son chemin).
2. **Égal à un seul fichier.**
   - Un programme découpé en trois fichiers donne les mêmes résultats que le même programme écrit en un seul, sur les deux allocateurs, de bout en bout dans Chromium, rien ne restant vivant.
   - La taille de son module est à 1 % près celle du module en un seul fichier.
3. **À travers la frontière entre fichiers :**
   - une structure qui tient des chaînes et des tableaux passée, déplacée et rendue ;
   - une énumération filtrée par `match` dans un autre fichier que le sien ;
   - le même nom privé dans deux fichiers ;
   - un fichier importé par deux autres (un losange), compilé une fois.
4. **Le test différentiel** est étendu à des programmes découpés en deux fichiers, lancé sur 2 000 graines. Casser l’édition de liens exprès doit le faire échouer : une fonction appelée à la place d’une autre du même nom dans un autre fichier.
5. **L’outillage.**
   - `perasc build --json` liste les fichiers.
   - Un test de bout en bout du plugin Vite change un fichier importé et voit le résultat reconstruit.
6. **Le guide gagne une page sur les modules**, en anglais et en français. L’état, la feuille de route, la référence de `E0244` et la nouvelle `E0250` sont mis à jour.

## Registre

_Chaque partie est consignée ici à son arrivée._

**Étape 3, les fonctions, structures et énumérations non génériques : arrivée.**

- **Ce qui a changé.**
  - Le pilote suit les imports depuis le fichier construit. Il lit, analyse et vérifie chaque fichier qu’il atteint, les fichiers qu’il importe d’abord, chacun contre ce que ceux-ci exportent. Un fichier atteint par deux chemins est lu une fois, donc ses structures et énumérations sont un seul type dans chaque fichier qui les importe.
  - Les fichiers sont abaissés en une seule passe dans un seul module de l’IR. Chaque fichier garde ses propres noms, et seuls les exports de la racine sont ceux de JavaScript. Les fonctions des autres fichiers reçoivent un nom à elles dans le module, `area__m1`.
  - Un cycle est refusé (`E0250`) dans le fichier dont l’import le ferme, avec le chemin depuis le dossier de la racine.
  - `E0244` gagne trois raisons : `generic`, `managed` (un module owned qui importe un fichier `.apeiron`) et `clash`, qui couvre maintenant les fonctions et deux imports d’un même nom.
  - `perasc build --json` liste les fichiers sous `inputs`, le plugin Vite les surveille, et `perasc check` sur un module owned le vérifie contre les fichiers qu’il importe.
- **Les tests, critère par critère.**
  1. `crates/perasc-driver/tests/modules.rs`, pour chaque refus : un fichier introuvable, un fichier qui ne se compile pas (affiché dans ce fichier), un nom non exporté, un conflit avec un nom local et avec un nom importé, une structure générique, un fichier `.apeiron`, et des cycles de deux et de trois fichiers avec leur chemin.
  2. Trois fichiers contre un seul, de bout en bout dans Chromium sur les deux allocateurs (`modules.test.ts`), et par l’interpréteur de l’IR, rien ne restant vivant. Leurs modules sont à 1 % près de la même taille.
  3. Les trois fichiers tiennent une structure faite d’une chaîne et d’un tableau de chaînes passée par `sink`, modifiée et rendue, une énumération filtrée dans un autre fichier que le sien, un `helper` privé dans chaque fichier, et un losange : `size.peras`, importé par les deux autres, compilé une fois.
  4. Le test différentiel découpe chaque programme généré en deux fichiers, chacun avec un `helper` privé qui calcule autre chose, et le compare au programme en un seul fichier sur 2 000 graines. Numéroter exprès les fonctions de chaque fichier à partir de zéro fait appeler à `helper` d’un fichier celui de l’autre, et le fait échouer dès la première graine.
  5. Un test du serveur de développement de Vite change un fichier que la racine importe et voit le résultat changer. Sans la surveillance, exprès, il garde l’ancien résultat et échoue.
  6. La page [modules](../guide/modules.md) du guide, dont le test de la documentation compile les blocs avec les fichiers qu’ils importent, et l’état, la feuille de route, `E0244` et `E0250`.
- **Trouvé en la construisant.**
  - La passe de propriété du vérificateur lisait les conventions de chaque fonction dans l’arbre syntaxique, où une fonction importée n’est pas. Un paramètre `sink` d’une fonction importée aurait été prêté, et le vérificateur de la release refusait le code. La passe lit maintenant les signatures du vérificateur, qui contiennent les fonctions importées.
  - Deux phrases de cette décision décrivaient le compilateur sans l’avoir lu (constat M020). L’une disait qu’une fonction qu’aucun export n’atteint est retirée ; aucune passe ne le fait. L’autre disait que les optimisations du compilateur comprennent l’inlining ; elles n’inlinent aucun appel. Les deux sont corrigées plus haut, là où elles étaient écrites.
  - Une fonction générique ne peut pas du tout être exportée d’un module owned (`E0230`), puisque ses exports sont ceux de JavaScript, et une constante de module ne peut pas être exportée (il n’y a pas de `export const`). L’étape 3b porte donc d’abord sur les structures et énumérations génériques, puis sur ces deux-là.
  - Les méthodes d’une structure importée ne sont pas importées avec elle (`E0215` sur l’appel) : le guide le dit.
