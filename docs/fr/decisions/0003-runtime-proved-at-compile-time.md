# 0003 : Un runtime prouvé à la compilation, pas un micro-OS

_Version anglaise, qui fait foi : [../../en/decisions/0003-runtime-proved-at-compile-time.md](../../en/decisions/0003-runtime-proved-at-compile-time.md)._

## Contexte

L'[architecture technique](../architecture.md) propose d'exécuter les modules possédés comme un « micro-OS virtuel » dans la mémoire WebAssembly :

- une MMU logicielle, avec des pages de 4 Ko et des handles de 32 bits ;
- un allocateur fait de slabs et d'un allocateur buddy ;
- des pages de garde ;
- un ordonnanceur de fibres ;
- de la copie sur écriture entre threads.

L'objectif est juste : une mémoire gérée de façon déterministe, sans ramasse-miettes, par le module lui-même.

Les mécanismes, eux, viennent d'une autre situation. Un système d'exploitation ne sait rien des programmes qu'il exécute : il doit traduire, intercepter et vérifier à l'exécution. Peras est dans le cas inverse. Avant que le moindre code s'exécute, son compilateur a prouvé :

- qui possède chaque valeur ;
- qu'aucune valeur n'a d'alias pendant qu'elle change (`inout` est exclusif) ;
- qu'aucune référence n'est rangée dans une donnée ;
- où chaque tampon est libéré, exactement une fois sur chaque chemin.

Des mécanismes qui vérifient à l'exécution paieraient, à chaque accès mémoire, pour ce qui est déjà connu.

## Décision

1. **Peras est un runtime prouvé à la compilation.** La documentation et les présentations du projet le décrivent ainsi, pas comme un micro-OS.
2. **Ce qui est retenu de la proposition, tel qu'il a été proposé :**
   - l'allocateur de production, TLSF avec `i32.clz` ([décision 0002](0002-tlsf-allocator.md)) ;
   - les tableaux en colonnes (déjà faits), le SIMD (phase 6) et la fusion de boucles ;
   - la règle `@frame`, la preuve que le code de rendu n'alloue pas : la forme que prend le « linter zéro GC » ;
   - les tampons circulaires sans verrou sur `Atomics`, entre workers ;
   - l'alignement à 256 octets des tampons partagés avec WebGPU (phase 6) ;
   - un inspecteur mémoire : une carte du tas et l'histoire de chaque valeur, construits sur ce que l'allocateur de débogage enregistre déjà.
3. **Ce qui est retenu sous une autre forme :**
   - **Le compactage plutôt qu'une table de pages.** Chaque tableau a un en-tête de 16 octets qui garde l'adresse de ses éléments, et un seul propriétaire détient cet en-tête. Un compacteur peut déplacer les éléments et mettre à jour un mot, sans changer aucune adresse détenue par le programme, et sans traduction à chaque accès.
   - **`sink` entre threads plutôt que la copie sur écriture.** Une valeur passée en `sink` change de propriétaire : l'envoyer à un worker est un déplacement sans copie. Le compilateur prouve que l'expéditeur n'y touche plus, ce qu'aucune protection de page ne pourrait faire.
   - **Des vérifications logicielles plutôt que des pages de garde.** La mémoire WebAssembly n'a pas de protection par page. Chaque index est déjà vérifié, et l'allocateur de débogage place une zone rouge après chaque bloc. Un module en faute redémarre à l'échelle de son Web Worker.
   - **Le réordonnancement des champs**, seulement quand des enregistrements auront une disposition en mémoire (pixels, tampons GPU), avec un attribut qui garde l'ordre écrit pour l'interopérabilité.
   - **Les courbes de remplissage**, en type de bibliothèque choisi exprès, comme une `Grid2D` en ordre de Morton, pas en transformation automatique.
4. **Ce qui n'est pas retenu :**
   - les fibres, tant que WebAssembly ne sait pas changer de pile (JSPI ou la proposition _stack switching_) ; les Web Workers donnent le parallélisme dès maintenant ;
   - les compteurs d'énergie (RAPL) dans le navigateur, qui n'en expose aucun ; l'énergie se mesure sur des bancs natifs ;
   - l'interdiction de `push`, qui ne sollicite jamais le ramasse-miettes du navigateur ;
   - les chiffres annoncés avant d'être mesurés.

## Conséquences

La feuille de route garde son ordre. Trois chantiers système s'y ajoutent :

1. **Le compactage du tas** derrière les en-têtes de tableaux, après la bibliothèque standard : la suite de TLSF.
2. **La concurrence** : Web Workers, valeurs déplacées vers eux avec `sink`, tampons circulaires. Elle demande le mode géré (phase 4) côté navigateur ; sa propre décision écrite fixera la frontière.
3. **L'inspecteur mémoire**, avec l'extension d'éditeur, en phase 7.

Toute annonce de performance vient avec sa mesure, comme la décision 0002 le fait pour TLSF.
