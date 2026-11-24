# 0044 : Les pools, beaucoup de valeurs d’un type tenues par des poignées

_Version anglaise : [../../en/decisions/0044-pools.md](../../en/decisions/0044-pools.md)._

_État : protocole enregistré le 2026-11-24, avant tout prototype et toute mesure, comme le demande la règle du projet. C’est le pas de la phase 6 de la [feuille de route](../reference/roadmap.md) qui suit les arènes ([décision 0043](0043-arenas.md)), « l’allocateur `Pool<T>` », et il vient avant `@frame` et l’étude de la mémoire chargée selon la proximité, qui a besoin d’un budget fixe de places. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression ; si aucune ne l’est, la décision le dit et rien n’est installé._

## Contexte

Ce que fait le compilateur aujourd’hui, lu et essayé avant d’écrire ceci (constat M020) :

- **Un tableau de structures est rangé en colonnes**, un buffer par champ : `Array<Spark>` garde son `x`, son `y` et son `parent` chacun dans un buffer à lui, et une boucle sur un champ ne touche qu’un buffer.
- **Un programme qui a beaucoup de valeurs qui vont et viennent, et qui pointent les unes vers les autres, écrit des indices à la main** : des tableaux, et un nombre pour chaque lien (`bench/shared/workloads/w2-index.peras`). La décision 0032 l’a mesuré 6,8 fois plus rapide qu’un arbre de `Rc`, avec 4,75 fois moins de mémoire. Ce qu’il doit écrire lui-même : la liste des places libres, pour en réutiliser une ; et rien n’attrape un indice gardé après que sa place a été libérée et réutilisée, qui lit alors une autre valeur, en silence.
- **Les structures et les fonctions génériques se compilent** : `struct Slots<T> { values: Array<T>; live: Array<bool>; }`, rempli par `fn put<T>(s: inout Slots<T>, v: sink T)`, se construit et tourne (essayé le 2026-11-24 au commit `658321d`) ; une valeur déplacée dans une fonction générique est déclarée `sink`, sinon le vérificateur la refuse (`E0209`).
- **La bibliothèque standard n’est pas écrite en Peras** : ses méthodes sont intégrées au vérificateur et au runtime (`crates/perasc-check/src/stdlib.rs`) ; il n’y a pas de source Peras que le compilateur livre et que chaque programme importe.
- **Ce qui vient ensuite a besoin d’un budget** : `@frame` interdit d’allouer pendant le rendu, donc ses valeurs vivent dans des places réservées d’avance ; l’étude de proximité garde un nombre fixe de blocs et en évince selon une politique.

Ce que font les autres : les indices à génération (`slotmap` en Rust, les systèmes d’entités des moteurs de jeu) : une poignée est une place et une génération ; libérer une place augmente sa génération, et une vieille poignée est refusée. Les pools d’objets de capacité fixe des moteurs de jeu et des systèmes embarqués. `std::pmr::unsynchronized_pool_resource` en C++.

## Options

- **p0. Comme aujourd’hui** : des tableaux et des indices, la liste des places libres écrite à la main. La référence.
- **p1. `Pool<T>` intégré au compilateur, poignées à génération** : `Pool.new()`, `pool.insert(v)` donne un `Handle<T>`, une place et une génération ; `pool.get(h)` une `Option<T>`, `None` pour une poignée dont la valeur a été retirée ; `pool.set(h, v)`, `pool.remove(h)`, `pool.size`, `for (const [h, v] of pool)`. Chaque accès compare la génération.
- **p2. La même, avec des poignées simples** : une poignée est une place ; une poignée gardée après que sa valeur a été retirée lit ce que la place contient maintenant.
- **p3. `Pool<T>` écrit en Peras**, une structure générique sur des tableaux (les valeurs, les générations, les places libres) avec les mêmes méthodes que p1, comme une source que le compilateur livre et qu’un programme importe (`import { Pool } from "peras:pool"`), aucun nouveau type dans le vérificateur. Elle a besoin de ce moyen de livrer une source Peras, qui n’existe pas : il est construit pour cette décision, et son coût compté.
- **Une capacité fixée à la création**, `Pool.withCapacity(n)`, `insert` donnant `None` quand le pool est plein et rien ne grandissant, est ce dont `@frame` aura besoin ; elle est mesurée comme une variante de l’option retenue, W3 ci-dessous, pas comme une option à part.

## Protocole, enregistré avant tout prototype

p1, p2 et p3 sont prototypées derrière `PERAS_PROTO_POOL=p1|p2|p3` ; sans elle, le compilateur est comme aujourd’hui, p0. Chaque module mesuré est construit pour la production, `--allocator tlsf` (constat M029), par une construction release de `perasc`.

### Avant toute mesure : les définitions testées

Chaque charge est d’abord écrite pour p0 et vérifiée donner ce que donne son module géré ; les opérations qu’offre chaque option sont confrontées à ce qu’utilisent les charges (constat M003). Une charge qu’une option ne peut pas exprimer, ou une définition trouvée fausse, est consignée et le protocole amendé avant toute mesure, l’amendement daté.

### Les charges

Des modules possédés sur TLSF dans Node, le module de chaque option et celui de p0 dans un seul processus, l’ordre alterné tour à tour, 21 tours, la médiane des rapports à p0 et leur étendue (constat M017). Chaque charge est aussi appelée deux fois sur une même instance, et sa seconde réponse vérifiée (constat M035).

- **W1, les étincelles** (`bench/pool/sparks.peras`) : à chaque image sur 600, mille étincelles naissent, chacune tenant la poignée de l’étincelle d’où elle vient, et celles plus vieilles que leur vie meurent ; les vivantes bougent, et lisent la place de leur parent par sa poignée quand il vit encore. La scène de la mesure : le même ciel pour chaque option.
- **W2, un graphe de scène avec des retraits** (`bench/pool/scene.peras`) : un arbre de 100 000 nœuds, chacun tenant les poignées de son parent et de ses enfants ; un tiers retiré, puis autant insérés, puis la profondeur de chacun sommée.
- **W3, un budget de blocs** (`bench/pool/chunks.peras`) : 256 places de 4 Kio chacune, des blocs demandés le long d’un trajet de 100 000 pas et le moins récemment utilisé évincé quand le budget est plein : la base de l’étude de proximité. Mesurée pour chaque option, et avec une capacité fixée à la création sous l’option retenue.
- **W4, sans pool** (`bench/pool/plain.peras`) : un programme de tableaux qui n’en utilise aucun ; avec chaque banc et chaque exemple, `check-regression` le garde identique.

### Les grandeurs

1. **Vitesse** : le temps de chaque charge, comme ci-dessus.
2. **Charge du processeur** : le temps pendant lequel un cœur est occupé, `process.cpuUsage`, sur les mêmes tours.
3. **Mémoire et octets** : la mémoire du module à la fin de chaque charge, et la taille de chaque module, brute et gzip ; `check-regression` contre le commit de ce protocole, et la taille du compilateur.
4. **Facilité d’écriture** : W1, W2 et W3 écrites pour chaque option comme un utilisateur le ferait depuis JavaScript, puis corrigées jusqu’à ce qu’elles se construisent : les erreurs rencontrées, avec leurs codes, les lignes, les notions que JavaScript ne demande pas.
5. **Sûreté** : `bench/pool/stale.peras`, un programme qui garde une poignée après avoir retiré sa valeur, puis lit à travers elle : sous chaque option, si la lecture périmée est attrapée (`None`, ou une faute) ou donne une autre valeur en silence.

### La justesse, avant que tout chiffre compte

Chaque option passe `cargo test --workspace` avec sa variable posée, les tests de bout en bout, et le test différentiel avec `PERAS_DIFFERENTIAL_SEEDS=2000`, son générateur étendu de pools dont les valeurs sont insérées, retirées et lues par des poignées gardées à travers les retraits ; puis une fois cassée exprès (p1 et p3 : la génération non comparée), pour voir un test échouer (constats M015, M034).

### Les critères, enregistrés maintenant

La mémoire est bornée en rapport et en octets, et un module identique à celui de p0 compte pour 1,000 (constat M037).

- Une option qui échoue à `check-regression` pour un programme qui ne l’utilise pas, ou à un contrôle de justesse ci-dessus, est écartée.
- Une option n’est une amélioration que si elle est **plus sûre que p0** (point 5 : la lecture périmée attrapée), son temps au plus **1,05** fois celui de p0 sur chaque charge, son étendue prise en compte, sa mémoire au plus **1,10 fois celle de p0 ou une page de 64 Kio de plus**, et W1 à W3 écrites en **pas plus de lignes** que p0, sans plus d’erreurs rencontrées.
- Parmi les améliorations, celle qui a le moins de notions et de lignes, puis la plus rapide par la médiane de ses rapports sur W1 à W3.
- Si aucune ne l’est, p0 reste, les prototypes sont retirés, leurs chiffres gardés ici, et la feuille de route dit pourquoi.

## Journal

### Les définitions testées, et un amendement (2026-11-24, avant tout prototype)

Les quatre charges et le programme du point 5 sont écrits pour p0 (`bench/pool/`), comme un utilisateur le ferait depuis JavaScript : W1 a rencontré six `E0210` (un entier divisé comme un flottant, `pop()` donnant une `Option<i32>`, un `i32` ajouté à un flottant), et chaque charge s’est arrêtée sur un dépassement dans une construction de développement, les nombres aléatoires `seed * 1103515245 + 12345` dépassant exprès, corrigés avec `wrappingMul` et `wrappingAdd` ; W2, W3 et le programme du point 5 ont été écrits après W1 et n’ont rencontré aucune erreur. Construite pour la production, chacune donne la même chose deux fois sur une même instance, la même chose construite pour le développement, et la même chose que son module géré (W1 433595.05078125, W2 487013, W3 12797022). Sous p0, le programme du point 5 lit 99, la valeur donnée à la place après que celle gardée a été retirée : en silence, comme attendu.

Une définition est amendée : W1 déplace chaque étincelle sur place (`sparks[i].x += sparks[i].vx`), et les opérations enregistrées pour p1 et p2 n’offrent que `set(h, v)`, une valeur entière. **p1 et p2 indexent aussi par une poignée, `pool[h]`, pour lire et écrire un champ** : sous p1, une poignée dont la valeur a été retirée arrête le programme, comme le fait un indice hors limites, `get(h)` restant la lecture qui donne `None` ; sous p2, elle lit la place telle qu’elle est. **p3**, une structure écrite en Peras, ne peut pas définir `[]` : elle offre `pool.slot(h)`, une `Option<i32>`, et le programme indexe `pool.values` avec, ce qui compte dans ses lignes.

_À venir : les prototypes, la mesure, la proposition._
