# 0034 : La division entière, telle qu’un utilisateur venu de JavaScript la rencontre

_Version anglaise : [../../en/decisions/0034-integer-division.md](../../en/decisions/0034-integer-division.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées à `4dc37e9` ; option d1 acceptée par le propriétaire le 2026-11-17. Il répond au constat F113, que la [décision 0030](0030-numeric-literals.md) a trouvé : `const t = i / (count - 1); total += t;` somme 1 là où JavaScript somme 2,5. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **`/` sur deux entiers est une division entière**, tronquée vers zéro, comme en Rust, Go, C et Swift (`BinOp::Div`, `crates/perasc-ir/src/lower.rs` ; `$div` dans un module managed). JavaScript n’a pas d’entier : son `/` donne une fraction, et une division entière s’écrit `Math.floor(a / b)` ou `Math.trunc`.
- **Une variable n’est jamais convertie** (décision 0030) : `total += t`, `t` un `i32` et `total` un `f64`, rencontre E0210 à la somme. Sa correction, `t.toF64()`, convertit le quotient déjà tronqué : le programme se construit et donne 1 là où JavaScript donne 2,5. Suivie telle qu’offerte, la correction du compilateur donne un nombre faux, en silence.
- **Chaque diagnostic bloque la construction** : il n’y a pas d’avertissement (`crates/perasc-diag/src/lib.rs`). Une option peut dire, ou refuser ; elle ne peut pas seulement conseiller.
- **Les programmes du dépôt divisent des entiers exprès** : un milieu `(lo + hi) / 2`, une ligne `p / width`. 280 lignes des bancs, des exemples et de la documentation contiennent `/` ; combien sont des divisions entières, et ce qu’une option qui change `/` casserait, les prototypes le comptent.

## Options

- **d0. Comme aujourd’hui.**
- **d1. L’erreur dit où.** Quand E0210 rencontre un entier là où un flottant est attendu, et que cet entier est une division d’entiers, écrite là ou tenue par un `const` qu’elle a initialisé, le message montre la division, dit qu’elle tronque, et sa correction est à la division : `i.toF64() / (count - 1).toF64()`. Rien d’autre ne change : `/` garde son sens.
- **d2. `/` comme celui de JavaScript.** `/` sur deux entiers donne un `f64` ; une division entière s’écrit `a.div(b)`.
- **d3. Ni l’un ni l’autre, refusé.** `/` sur deux entiers est une erreur qui offre les deux : `a.div(b)` pour le quotient entier, `a.toF64() / b.toF64()` pour la fraction.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_DIV` ; le build sans elle garde le comportement d’aujourd’hui, d0.

1. **Facilité d’écriture, et les résultats que donnent les corrections** : cinq programmes, écrits comme un utilisateur les écrit à partir de JavaScript, puis corrigés en appliquant la correction qu’offre chaque diagnostic, telle que donnée, jusqu’à ce qu’ils se construisent :
   - Q1, les pas du dégradé sommés, `const t = i / (count - 1); total += t;` (`bench/numbers/ease/p5t.peras`), une fraction voulue ;
   - Q2, une moyenne rendue en `f64`, `return sum / count;`, une fraction voulue ;
   - Q3, le milieu d’une recherche dichotomique, `Math.floor((lo + hi) / 2)` en JavaScript, un entier voulu ;
   - Q4, la ligne et la colonne d’un pixel, `Math.floor(p / width)` et `p % width`, un entier voulu ;
   - Q5, un pourcentage affiché en nombre entier, `Math.round(done * 100 / total)`, une fraction arrondie.

   Pour chacun : les erreurs rencontrées, avec leurs codes ; les lignes changées ; et **si le programme ainsi corrigé donne le résultat de JavaScript**. Un résultat faux atteint en suivant la correction du compilateur lui-même compte contre l’option, quoi qu’elle gagne par ailleurs.

2. **Non-régression** (la règle du propriétaire du 2026-11-17) : chaque programme des bancs et des exemples construit sous chaque option contre le compilateur d’aujourd’hui, `scripts/check-regression.mjs` : combien ne se construisent plus, combien changent. Une option sous laquelle un programme du dépôt ne se construit plus, ou calcule autre chose, régresse : elle est écartée, sauf si la décision migre chacun de ces programmes, la migration comptée en lignes.
3. **Vitesse et charge du processeur** : deux boucles de 10 000 000 de tours, la version de chaque option telle que corrigée au point 1, modules owned sur TLSF dans Node, dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports à d0 tour par tour et leur étendue (constat M017), le temps CPU sur le temps réel : W1, une ligne et une colonne par pixel, un entier voulu ; W2, la somme du dégradé, une fraction voulue.
4. **Octets** : le module de chaque version, brut et en gzip.
5. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-division.mjs` les lance ; `--check` vérifie que chaque programme se construit comme consigné et donne ce qu’il a donné, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._

### Une précision du protocole (2026-11-17)

Le point 1 corrige un programme par les modifications que ses diagnostics offrent, telles que données. La première version a rencontré deux diagnostics qui n’en offrent aucune, dans chaque option : `Math` n’est pas défini dans un module owned (E0200, constat F116), et un entier n’a pas de `floor` (E0215, constat F117). Avant toute mesure, deux règles qu’un utilisateur suit ont été ajoutées au script, les mêmes pour chaque option : `Math.f(x)` s’écrit `(x).f()`, à la manière du guide ; `.floor()`, `.round()`, `.ceil()` ou `.trunc()` sur un entier s’enlève, l’entier étant déjà entier. Sans elles, Q3 à Q5 restaient bloqués sous d0, d1 et d3, et seule d2 les construisait.

### La mesure (2026-11-17)

Exécution 1, au commit 4dc37e9, rien d’autre en cours ; chaque tableau dans [`bench/division/RESULTS.md`](../../../bench/division/RESULTS.md), avec la machine ; la non-régression par `node scripts/check-regression.mjs 4dc37e9 PERAS_PROTO_DIV=<option>`.

| Quantité                                                                                                 | d0          | d1                    | d2                    | d3                    |
| -------------------------------------------------------------------------------------------------------- | ----------- | --------------------- | --------------------- | --------------------- |
| Programmes donnant le résultat de JavaScript une fois corrigés par les corrections du compilateur, sur 5 | 2           | **4**                 | 5                     | 2                     |
| Q1, le dégradé (JavaScript : 2,5)                                                                        | 1           | 2,5                   | 2,5                   | 1                     |
| Q2, une moyenne (7 / 3)                                                                                  | 2           | 2,333…                | 2,333…                | 2                     |
| Q5, un pourcentage arrondi (67 %)                                                                        | 66 %        | 66 %                  | 67 %                  | 66 %                  |
| Programmes du dépôt qui ne se construisent plus                                                          | -           | **0**                 | 13                    | 17                    |
| Programmes qui se construisent encore et calculent autre chose                                           | -           | **0**                 | 4                     | 0                     |
| Temps de compilation, / d0                                                                               | 1           | 1,003                 | 0,970                 | 0,908                 |
| W1, un entier voulu, / d0                                                                                | 1           | 1,000 (0,944 à 1,032) | 0,858 (0,794 à 3,501) | 1,001 (0,909 à 1,156) |
| W2, une fraction voulue, / d0                                                                            | 1 (et faux) | 0,668 (0,452 à 0,923) | 0,666 (0,456 à 0,720) | 0,996 (et faux)       |
| Octets de W1 (gzip)                                                                                      | 135 (123)   | 135 (123)             | 148 (140)             | 135 (123)             |

Ce que cela dit :

- **d0 mène un utilisateur à un nombre faux** : en suivant sa propre correction, `t.toF64()`, le dégradé somme 1 et la moyenne donne 2, en silence.
- **d1 corrige cela là où cela commence** : la correction est à la division, et les deux programmes donnent le résultat de JavaScript ; rien d’autre ne change, pas un octet d’aucun module du dépôt. Celui qu’elle manque, Q5, arrondit une fraction qu’aucun diagnostic ne rencontre : `done * 100 / total` est un entier dès le départ, utilisé comme tel.
- **d2 est le sens de JavaScript, au prix de chaque programme qui divise des entiers** : 13 du dépôt ne se construisent plus, et 4 se construisent et calculent autre chose, le pire cas, puisque rien ne le dit. Sous la règle du propriétaire, elle régresse, sauf si la décision migre chacun de ces programmes ; et les programmes d’un utilisateur, hors du dépôt, changeraient sans un mot.
- **d3 refuse ce que d2 change** : 17 programmes ne se construisent plus, et sa correction, `a.div(b)` d’abord, mène Q1 et Q2 aux mêmes nombres faux que d0.
- **W2 est plus rapide quand il est juste** : une division flottante coûte moins qu’une division entière dont le diviseur est vérifié ; d1 et d2 calculent la fraction en deux tiers du temps de d0, d0 et d3 la calculant faux.

### Proposition

**d1** : la seule option qui soit une amélioration sans régression, sous la règle du propriétaire du 2026-11-17. Elle change deux des trois résultats faux en justes, ne change aucun module, et ne coûte rien en temps ni en octets. Ce qu’elle laisse, Q5, et les deux frictions rencontrées en route, F116 (`Math`) et F117 (`floor` sur un entier), sont chacun une étape à part, mesurée comme celle-ci. Le propriétaire décide.
