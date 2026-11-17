# 0037 : Comment un module managed vérifie ses indices

_Version anglaise : [../../en/decisions/0037-index-checks-in-managed-modules.md](../../en/decisions/0037-index-checks-in-managed-modules.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `338eceb` ; proposition : o1. Il répond au constat F221, que le profil de la [décision 0029](0029-pond-of-lanterns.md) a expliqué : le module managed du bassin, construit pour la production, prend 2,62 fois le temps du TypeScript dans Node, et 1,09 une fois ses vérifications d’indice écrites comme des accès directs. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

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

### Justesse (2026-11-17)

- **Un test lit et écrit hors des bornes** (`an_index_out_of_bounds_throws_after_the_same_effects`), en lecture, en écriture, en affectation composée et en pas, son indice et sa valeur avec des effets : o0, o1 et o2 lèvent la même `RangeError` après les mêmes effets (une lecture après son indice, une écriture après son indice et sa valeur, une affectation composée après son indice, sa lecture échouant d’abord). Une mutation de o1 qui calcule la valeur après le test le fait échouer.
- **o3, comme prévu, rompt la promesse du langage** : la lecture donne 0 (`undefined + 1`, rebouclé), l’écriture agrandit le tableau à six éléments, les deux en silence ; elle échoue à ce test et au plus ancien `out_of_bounds_throws`.
- **Chaque script `--check`, les tests de bout en bout et `cargo test --workspace`** passent sous o1 et o2. Sous o3, les deux tests ci-dessus échouent, comme prévu. Une exécution de `bench-workers.mjs --check` sous o3 a échoué sans message ; il ne construit aucun module managed, donc aucune option de cette décision ne l’atteint, et neuf exécutions après elle, trois sous o3 et six sans option, sont passées. Consigné ici, non expliqué.

### La mesure (2026-11-17)

Deux exécutions au commit 338eceb, l’une après l’autre, rien d’autre ne tournant, chaque module un build de production ; chaque tableau dans [`bench/index/RESULTS.md`](../../../bench/index/RESULTS.md), avec la machine. Rapports à la référence écrite à la main, médiane de 21 tours, exécution 1 puis 2 :

| Charge                                    | o0, aujourd’hui | o1             | o2             | o3, sans vérification |
| ----------------------------------------- | --------------- | -------------- | -------------- | --------------------- |
| W1, le bassin, face à `pond.ts`           | 2,402, 2,598    | 1,186, 1,144   | 2,203, 1,987   | 0,978, 0,922          |
| W2, le jardin, face à `garden.ts`         | 1,272, 1,316    | 1,308, 1,306   | 1,285, 1,306   | 1,287, 1,326          |
| W3, les quatre noyaux, face au JavaScript | 0,787 à 1,076   | 0,995 à 1,037  | 0,792 à 1,092  | 0,929 à 1,000         |
| Le module du bassin, octets (gzip)        | 14 569 (3 674)  | 18 017 (4 149) | 14 875 (3 694) | 13 889 (3 513)        |

- **o1 divise par deux le temps du bassin**, de 2,40 à 1,19 fois celui du TypeScript et de 2,60 à 1,14, la vérification gardée. Ce qui reste face au TypeScript, environ 0,2, est ce que coûte la vérification : o3, qui ne vérifie rien, dessine à 0,98 et 0,92.
- **o2 non** : une paire de fonctions par sorte d’élément laisse le bassin à 2,20 et 1,99. Pourquoi une vérification par accès aide là où une fonction par sorte n’aide pas n’est pas mesuré ici ; que le moteur optimise chaque site à part est une cause possible, non montrée (constat M001).
- **Le jardin est à 1,27 à 1,33 fois le temps du TypeScript sous chaque option** : son seul accès vérifié n’est pas son écart, qui est ailleurs (constat F224).
- **W3 est dans le bruit de la méthode** : `words` ne lit aucun indice, son code est le même octet pour octet sous o0, o1 et o2, et donne pourtant de 0,79 à 1,03.
- **Octets** : par `scripts/check-regression.mjs 5ba74b7`, o1 n’agrandit aucun module du dépôt et en réduit deux (`bench/managed/kernels.apeiron`, de 2 756 à 2 725 octets ; `bench/web-bugs/w11-off-by-one.apeiron`, de 977 à 948) ; o2 agrandit ces deux-là ; o0 et o3 n’en changent aucun, le script construisant les modules managed en débogage, où o3 vérifie encore. Le bassin n’est pas un module managed du dépôt, et le script ne le voit pas : o1 l’agrandit de 24 %, 13 % en gzip, chaque accès vérifié écrit en entier. Temps de compilation de 0,973 à 1,004 ; le compilateur grossit de 4 736 octets avec les quatre construites.

### Proposition

**o1** : le module managed du bassin divise son temps par deux et garde sa vérification, à 1,14 à 1,19 fois le temps du TypeScript, chaque résultat et chaque erreur identiques à aujourd’hui ; aucun module du dépôt ne grossit, deux maigrissent. Son coût, 475 octets de plus en gzip dans un module qui vérifie 64 accès, est celui du bassin ; la règle du propriétaire, telle que `check-regression.mjs` la tient, est respectée. Le propriétaire décide.
