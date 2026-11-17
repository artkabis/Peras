# 0036 : Les petites fonctions insérées, les invariants sortis des branches

_Version anglaise : [../../en/decisions/0036-inlining-and-invariants.md](../../en/decisions/0036-inlining-and-invariants.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F222, que le profil de la [décision 0029](0029-pond-of-lanterns.md) a trouvé : dans le bassin, le travail qui ne dépend que de la ligne est fait à chaque pixel, et les petites fonctions sont appelées là où LLVM les insère ; fait à la main, chacun gagne environ 3,5 % de l’image. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Aucune fonction n’est insérée**, sauf une fonction récursive en elle-même, sur trois niveaux (`crates/perasc-ir/src/inline.rs`). `tri`, `unit` et `byte` du bassin, quelques opérations chacune, sont des appels à chaque pixel.
- **Le déplacement des invariants de boucle** (`crates/perasc-ir/src/licm.rs`) sort `l = e` d’une boucle quand `e` est simple (`is_simple`, `crates/perasc-ir/src/opt.rs`) : sans appel, et sans aucune division, même celle d’un flottant, qui ne peut pas arrêter le programme. Une affectation dans un `if` de la boucle ne sort pas.
- **Une fonction qui ne change rien est connue** (`pure_functions`, décision 0031), pour `parallel for` ; rien d’autre ne s’en sert.
- **Le moteur insère de lui-même** aussi, selon son propre budget (constat de `inline.rs`) : un gain ici doit être mesuré, pas supposé.

## Options

- **i0. Comme aujourd’hui.**
- **i1. Les petites fonctions insérées** : une fonction du module dont le corps est un seul `return e`, `e` d’au plus 40 nœuds, sans boucle, sans appel à elle-même ni valeur du tas, est remplacée à chaque appel par `e`, ses paramètres liés aux arguments, chacun évalué une fois et dans l’ordre.
- **i2. Des invariants plus larges** : une division de flottants compte comme simple ; de même un appel à une fonction qui ne change rien et ne peut pas arrêter le programme ; et une affectation invariante dans un `if` de la boucle sort de la boucle quand son expression est simple, la calculer une fois étant alors sûr quelle que soit la branche.
- **i3. Les deux**, i1 puis i2.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_INLINE` ; le build sans elle garde le comportement d’aujourd’hui, i0.

1. **Justesse** : le test différentiel (`crates/perasc-driver/tests/differential.rs`) avec `PERAS_DIFFERENTIAL_SEEDS=2000` sous chaque option, son générateur étendu à ce que touchent les options (petites fonctions appelées dans des boucles, divisions, branches) ; puis une fois avec chaque option cassée exprès, pour voir le test échouer (constat M015). Chaque script `--check` du projet sous chaque option.
2. **Vitesse et charge du processeur**, modules owned sur TLSF dans Node, le module de chaque option et celui de i0 dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports à i0 tour par tour et leur étendue (constat M017), le temps CPU sur le temps réel :
   - **W1**, le bassin, image 600 en 1024 × 768 avec 1 000 lanternes (`bench/lanterns/pond.peras`) ;
   - **W2**, le jardin de la décision 0013 (`bench/garden/garden.peras`), une image ;
   - **W3**, chaque noyau de `bench/browser/kernels.peras`.
3. **Octets** : chaque module du dépôt, brut et en gzip, par `scripts/check-regression.mjs` sous chaque option, qui donne aussi le temps de compilation et la taille du compilateur. Une optimisation change des modules par nature : chaque module qui change est l’un de W1 à W3, ou est mesuré avec eux, et aucun ne doit être plus lent (la règle du propriétaire).
4. **Facilité d’écriture** : rien à mesurer ; le même programme est écrit pour chaque option. Dit tel quel.
5. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-inline.mjs` lance les points 2 et 3 ; `--check` vérifie que les modules de chaque option donnent les résultats de i0, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
