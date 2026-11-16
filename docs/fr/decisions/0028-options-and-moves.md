# 0028 : Prendre une valeur ou partir, et quand un déplacement a lieu

_Version anglaise, qui fait foi : [../../en/decisions/0028-options-and-moves.md](../../en/decisions/0028-options-and-moves.md)._

_État : proposée le 2026-11-16, avec ses options et son protocole enregistrés avant qu’aucun prototype ne soit construit ni mesuré, comme la règle du projet le demande ([la décision 0027](0027-closures-over-the-heap.md) a été la première). Elle répond aux constats F101, F103, F104 et F105, et au bogue F012 trouvé en lisant le code pour elle._

## Contexte

Ce que fait le code aujourd’hui, lu et exécuté avant d’écrire ceci (constat M020).

- **F103, un déplacement a lieu là où son argument est écrit.** `m.set(key, m.get(key).unwrapOr(0) + 1)` est refusé (`E0204`) : `key` se déplace dans `set` par le premier argument, et le second le lit ensuite. La lecture du second argument se termine avant que `set` soit appelée ; JavaScript accepte la ligne, et c’est la ligne la plus courante d’un comptage.
- **F101, une branche qui part ne donne pas de type au `match`.** Dans `const row = match (parse(line)) { Some(r) => r, None => { continue; } };`, la branche `None` est un bloc qui ne finit jamais normalement, mais elle est typée `void`, si bien que le `match` est `void` et son usage refusé avec `` `void` has no field or method `qty` `` (`E0215`), qui ne dit pas pourquoi.
- **F105, aucun moyen de prendre une valeur ou de partir.** Il n’y a ni `unwrap` ni `let … else` : chaque valeur optionnelle demande un test, `isNone()`, puis `unwrapOr` avec une valeur de remplacement jamais utilisée.
- **F104, le repli de `unwrapOr` est évalué quand il n’est pas nécessaire**, dans un module owned : un repli qui fabrique une chaîne alloue à chaque appel. La décision 0016 a rendu paresseux un repli tranquille (F209), pas un repli qui a un effet.
- **F012, un bogue : les deux modes ne s’accordent pas sur `unwrapOr`.** Un module managed le compile en `x !== undefined ? x : repli`, qui n’évalue le repli que si nécessaire ; un module owned l’évalue toujours. Avec un repli qui change une variable de module, le même source donne `501, 202` dans un module owned et `500, 101` dans un module managed.

## Options

Trois questions, chacune tranchée par une de ses options. Chaque option est construite en prototype quand c’est possible, et mesurée selon le protocole ci-dessous avant qu’une soit choisie.

**Q1, quand un déplacement a lieu (F103).**

- **1a.** Comme aujourd’hui : refusé, avec la correction `key.copy()`.
- **1b. Un déplacement a lieu à l’appel**, une fois chaque argument évalué : un argument suivant peut lire la valeur, tant que ce qu’il lit n’est pas prêté au même appel. `m.set(key, m.get(key).unwrapOr(0) + 1)` se construit ; `f(key, key)` avec le second prêté reste refusé.
- **1c. Une copie faite pour le programme** : le compilateur insère `key.copy()` là où un argument suivant lit ce qui se déplace.

**Q2, prendre une valeur ou partir (F101, F105).**

- **2a.** Comme aujourd’hui : `isNone()`, une valeur de remplacement, `unwrapOr`.
- **2b. Une branche qui ne finit jamais normalement n’a pas de type** à elle (`never`) : `match (parse(line)) { Some(r) => r, None => { continue; } }` a le type de `r`. Quand chaque branche part, le message le dit.
- **2c. 2b, et `const … else`** : `const row = parse(line) else { continue; };` prend la valeur de `Some` ou `Ok`, ou exécute le bloc, qui doit partir.
- **2d. `unwrap()`**, qui arrête le programme avec une erreur quand il n’y a pas de valeur.

**Q3, quand un repli est évalué (F104, F012).**

- **3a. Toujours, dans les deux modes** : le module managed l’évalue d’abord lui aussi.
- **3b. Seulement si nécessaire, dans les deux modes**, comme le font `??` et `? :` en JavaScript : le module owned évalue le repli dans la branche où il n’y a pas de valeur.
- **3c. Toujours, et `unwrapOrElse(() => …)`** pour un repli évalué seulement si nécessaire.

## Protocole, enregistré avant tout prototype

Quatre grandeurs, comme pour la décision 0027 : la vitesse, la charge du processeur, la mémoire et les octets, la facilité d’écriture.

1. **Facilité d’écriture** : six programmes, écrits comme un utilisateur les écrirait depuis JavaScript, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option de la question qu’ils touchent :
   - P1, des mots comptés dans une table, `m.set(w, m.get(w).unwrapOr(0) + 1)` ;
   - P2, des lignes lues en rangées, une mauvaise ligne sautée ;
   - P3, une fonction qui rend tôt quand une recherche ne trouve rien ;
   - P4, une recherche avec une chaîne comme repli ;
   - P5, une chaîne de trois étapes optionnelles, chacune dépendant de la précédente ;
   - P6, des rangées regroupées par une clé dans une table de structures.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au JavaScript, les notions que JavaScript ne demande pas. Chaque programme corrigé donne ce que donne le même source construit comme module managed.

2. **Charges**, modules owned sur TLSF dans Node, le module de chaque option dans un même processus, l’ordre alterné, 21 tours, la médiane des rapports à l’option qui garde le comportement d’aujourd’hui (1a, 2a, 3a), et leur étendue (constat M017) :
   - **W1**, P1 sur 1 000 000 de mots tirés parmi 1 000 : 1b contre 1c, qui copie la clé à chaque mot ;
   - **W2**, P2 sur 1 000 000 de lignes, une sur dix mauvaise : 2a, 2b, 2c et 2d (2d sur des lignes toutes bonnes) ;
   - **W3**, P4 sur 1 000 000 de recherches qui trouvent leur clé neuf fois sur dix, le repli une chaîne fabriquée à chaque appel : 3a, 3b et 3c.
3. **Charge du processeur** : le temps CPU sur le temps écoulé des tours de chaque module.
4. **Mémoire et octets** : la mémoire du module après chaque charge ; ses octets, bruts et compressés.
5. **Les modes s’accordent** : pour Q3, un repli qui a un effet donne le même résultat dans les deux modes pour chaque option gardée ; une option qui ne peut pas les accorder est écartée.
6. **Une option non construite** est rapportée comme telle, avec la raison, jamais classée sur une supposition (constat M001).

## Critères de l’étape qui suit le choix

Fixés une fois les options choisies, avec le propriétaire, comme pour la décision 0027.

## Registre

_Chaque partie est consignée ici à son arrivée._
