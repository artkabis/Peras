# 0036 : Les petites fonctions insérées, les invariants sortis des branches

_Version anglaise : [../../en/decisions/0036-inlining-and-invariants.md](../../en/decisions/0036-inlining-and-invariants.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `1f4b070` ; proposition : aucune, i0 gardée. Il répond au constat F222, que le profil de la [décision 0029](0029-pond-of-lanterns.md) a trouvé : dans le bassin, le travail qui ne dépend que de la ligne est fait à chaque pixel, et les petites fonctions sont appelées là où LLVM les insère ; fait à la main, chacun gagne environ 3,5 % de l’image. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

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

### Une précision du protocole (2026-11-17), avant tout prototype

Lu avant le premier prototype (constat M020) : `optimize` (`crates/perasc-ir/src/opt.rs`) ne lance aucune de ses passes, le déplacement des invariants compris, sur une fonction qui utilise le tas (`Function::uses_heap`), aucune passe n’étant prouvée garder les libérations exactes. `render` du bassin prend ses tableaux en paramètres : aujourd’hui rien ne sort de ses boucles, pas même un invariant simple. Chaque fonction lourde de W1 à W3 qui prend un tableau est dans ce cas.

Il en va de même pour une fonction qui en appelle une donnant plusieurs résultats, une structure (`Function::uses_many`) : `render` appelle `lantern()`, qui donne un `Mesh`.

Donc i2, et i3 avec elle, lance aussi le déplacement des invariants sur une fonction qui utilise le tas ou en appelle une donnant plusieurs résultats, les cibles d’un tel appel comptées comme des affectations, en ne déplaçant qu’une affectation à une variable locale qui n’est pas une poignée, d’une expression simple, qui ne lit aucun bloc (`is_simple` refuse `Buf`, `Move`, `Borrow` et `Alloc`) : ce qu’elle déplace ne peut ni allouer, ni libérer, ni lire la mémoire. i1 insère aussi dans une fonction qui utilise le tas, une petite fonction qui ne prend et ne donne que des nombres ou des booléens. Les autres passes d’`optimize` restent telles quelles ; le déplacement sur une fonction qui utilise le tas est mesuré au sein de i2, pas comme une cinquième option, puisque sans lui i2 n’atteindrait pas le bassin.

### Justesse (2026-11-17)

- **Le test différentiel**, son générateur étendu (quatre petites fonctions ; des petites fonctions appelées dans des boucles, l’une avec un argument lu deux fois ; une division de flottants et un appel qui ne changent pas dans une boucle, l’un dans une branche ; des arguments à effet dont l’ordre compte ; un appel à droite de `&&` ; un dans la condition d’une boucle ; une variable de module lue avant un appel qui la change) : chaque option le passe avec 2 000 graines. Trois mutations faites exprès le font échouer : i1 qui substitue chaque argument directement ; i1 qui calcule un argument dans une variable là où ce qui précède l’appel n’est pas simple ; i2 qui sort d’une branche une affectation qui n’est pas invariante.
- **Chaque script `--check`** du projet passe sous i1, i2 et i3. `cargo test --workspace` passe sous chacune, sauf deux tests d’`opt_tests`, qui vérifient la forme de i0 : `g(n)`, qui rend `n`, sorti d’une boucle par i2, ce qui est permis puisque `g` ne peut pas arrêter le programme ; `g(n) = 10 / n` inséré par i1 là où il était appelé, dans le même ordre.

### La mesure (2026-11-17)

Deux exécutions au commit 1f4b070, l’une après l’autre, rien d’autre ne tournant ; chaque tableau dans [`bench/inline/RESULTS.md`](../../../bench/inline/RESULTS.md), avec la machine. Rapports à i0, médiane de 21 tours :

| Charge                             | i1            | i2            | i3            |
| ---------------------------------- | ------------- | ------------- | ------------- |
| W1, le bassin, exécution 1, puis 2 | 0,991, 1,014  | 1,060, 0,994  | 0,915, 0,964  |
| W2, le jardin                      | 1,046, 1,049  | 1,021, 1,009  | 1,053, 1,056  |
| W3, chaque noyau                   | 0,966 à 1,107 | 0,925 à 1,018 | 0,761 à 1,006 |
| Modules plus grands, sur 173       | 19            | 0             | 19            |
| Modules changés, de même taille    | 2             | 26            | 9             |

- **W3 est le bruit de la méthode** : le module des noyaux est le même, octet pour octet, sous les quatre options, et ses rapports vont pourtant de 0,761 à 1,107 en médiane. Une différence de quelques centièmes sur W1 est dans ce bruit. Le temps CPU sur le temps réel est de 0,98 à 1,03 partout : un seul fil, comme avant.
- **L’insertion ralentit le jardin**, dans les deux exécutions, i1 comme i3, à chaque tour de la seconde exécution de i3 : 5 %. Pourquoi n’est pas mesuré. Une cause est possible, non montrée : `noise` grossit de quatre copies de `unit`, et le moteur, qui dimensionne sa propre insertion d’après les corps du module ([`inline.rs`](../../../crates/perasc-ir/src/inline.rs)), pourrait cesser de l’insérer dans `fbm`. Un verdict demanderait une mesure, pas ce raisonnement (constat M001).
- **i1 et i3 agrandissent 19 modules**, de 1 à 3 %, ce que la règle du propriétaire ne permet pas.
- **i2 sort de la boucle des pixels du bassin ce que F222 avait trouvé**, les divisions de la ligne et l’argument de `tri` (lu dans l’IR), et ne change aucune taille ; son gain sur W1, s’il existe, est sous le bruit de la méthode. Les versions écrites à la main de la décision 0029 gagnaient 3,5 % chacune dans leur propre profil ; ici, le moteur semble déjà en faire une partie, ce que cette exécution ne peut pas séparer.
- **Le temps de compilation**, par `scripts/check-regression.mjs 1494bac` : de 1,004 à 1,009 pour chaque option ; le compilateur grossit de 24 640 octets avec les quatre construites.

### Proposition

**Aucune des options, i0 gardée.** i1 et i3 agrandissent des modules et ralentissent le jardin : une régression, sous la règle du propriétaire. i2 n’est pas une régression, mais n’apporte aucun gain que la méthode puisse distinguer de son bruit : une option qui coûte du code et n’apporte rien de mesuré n’est pas une amélioration. Une fois que le propriétaire aura décidé, les prototypes quittent le compilateur, leurs chiffres gardés ici et leur code dans git au commit 1f4b070 ; le test différentiel garde son générateur élargi. F222 reste ouvert, avec ce que cette décision a appris : une insertion pour le bassin devrait laisser telle quelle l’insertion propre au moteur, et son gain être montré au-dessus du bruit, par plus de tours ou sur une charge où il est plus grand. Le propriétaire décide.
