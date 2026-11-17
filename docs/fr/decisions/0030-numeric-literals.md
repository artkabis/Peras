# 0030 : Des nombres écrits comme JavaScript les écrit

_Version anglaise : [../../en/decisions/0030-numeric-literals.md](../../en/decisions/0030-numeric-literals.md)._

_État : protocole enregistré le 2026-11-17 (commit `f73b8f6`), avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées à `bb91877` ; option b acceptée par le propriétaire le 2026-11-17. Il répond au constat F112, la première des corrections que le propriétaire a ordonnées après la [décision 0029](0029-pond-of-lanterns.md)._

## Contexte

Ce que fait le vérificateur aujourd’hui, lu avant d’écrire ceci (constat M020) : un littéral entier a le type `{integer}`, qui devient tout type entier que son contexte demande, et jamais un flottant (`E0211`, `crates/perasc-check/src/expr.rs`, `coerce`). La page d’`E0211` en donne la raison : « le point décimal rend l’intention visible ». Un tableau littéral prend le type de son premier élément, puis refuse les autres (`E0210`).

La décision 0029 a mesuré ce que cela coûte : le bassin, écrit à partir de son TypeScript, a rencontré 45 erreurs avant de se construire, 35 `E0211` (`2 * x`, `if (c < 0)`, `v = 1;` sur des flottants) et 10 `E0210` (`[1, s, 0, -s]` avec `s` un flottant, et les lignes qui suivaient). Aucun programme écrit pour le projet n’en a rencontré plus.

Ce que font les autres langages de `2 * x`, `x` un flottant : TypeScript n’a qu’un type de nombre ; Rust le refuse, comme Peras ; Swift et Go l’acceptent, un littéral prenant le type auquel il sert (`ExpressibleByIntegerLiteral` de Swift, les constantes sans type de Go) ; aucun des quatre ne convertit en silence une _variable_ entière en flottant, sauf JavaScript, où chaque nombre en est un.

## Options

- **a. Comme aujourd’hui** : `E0211`, avec la correction `2.0`.
- **b. Un littéral prend le type auquel il sert**, comme en Swift et en Go : un littéral entier là où un flottant est attendu est ce flottant, `2 * x`, `c < 0`, `v = 1` ; un tableau littéral de nombres prend le type de ses éléments ensemble, un flottant si l’un est un flottant, `[1, s, 0]`. Une variable n’est jamais convertie : `n * x`, `n` un `i32`, reste refusé, avec la correction `n.toF64()`.
- **c. b, et une variable entière devient un flottant là où un flottant est attendu**, comme le fait JavaScript : `n * x` et `sum / xs.length` convertissent l’entier. Un `i64` au-delà de 2⁵³ perd des chiffres sans un mot.

Une quatrième a été envisagée et n’est pas une option ici : une variable déclarée à partir d’un littéral entier, `let sum = 0;`, typée par un usage ultérieur comme flottant, `sum += x`. Ni Swift ni Go ne le font, et le vérificateur type une déclaration quand il la lit ; ce serait une décision sur l’inférence, pas sur les littéraux.

## Protocole, enregistré avant tout prototype

Quatre quantités, comme pour les décisions 0027 et 0028.

1. **Facilité d’écriture** : cinq programmes, écrits comme un utilisateur les écrit à partir de JavaScript, puis corrigés jusqu’à ce qu’ils se construisent, pour chaque option :
   - P1, le bassin de la décision 0029 tel qu’écrit d’abord (`bench/lanterns/as-written/pond.peras`) ;
   - P2, la moyenne et l’écart type d’un `Array<f64>`, `sum / xs.length` ;
   - P3, un pas de particules : positions et vitesses ralenties, rebondissant sur les murs à 0 et 1 ;
   - P4, des prix en centimes, un `i64`, avec une taxe de 20 % et un arrondi ;
   - P5, une couleur mêlée de trois composantes et un dégradé, `[0, 0.5, 1]`.

   Pour chacun : les erreurs rencontrées avant qu’il se construise, avec leurs codes, les lignes changées par rapport au JavaScript, les notions que JavaScript ne demande pas.

2. **Ce que chaque option laisse passer** : pour chaque programme, un résultat qui diffère de ce que donne la même source en module managed, qui est du JavaScript, compte comme un changement silencieux. P4 est écrit avec une valeur au-delà de 2⁵³ pour cela.
3. **Vitesse, charge du processeur, mémoire et octets** : le bassin tel qu’écrit, construit par b et c, contre le bassin corrigé, construit par a : les octets de chaque module, bruts et gzip, égaux ou non ; le temps de l’image 600 en 1024 × 768 dans un même processus, les versions alternant, 15 tours après 2, le rapport tour par tour (constat M017), et le temps CPU sur le temps réel.
4. **Les modes s’accordent** : chaque programme corrigé donne, en module owned sur les deux allocateurs, ce qu’il donne en module managed.
5. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-numbers.mjs` les lance ; `--check` vérifie que chaque programme se construit et s’accorde, et tourne avec chaque vérification du projet.

## Décision

**Option b : un littéral prend le type auquel il sert**, comme en Swift et en Go.

1. Un littéral entier là où un flottant est attendu est ce flottant : `2 * x`, `c < 0`, `v = 1;`, `return 0;`, un argument, `-1`. Un composé de littéraux est calculé dans ce flottant, `1 / 2` là où un `f64` est attendu valant 0,5, comme en Swift et en JavaScript.
2. Un tableau littéral de nombres prend le type de ses éléments ensemble : un flottant si l’un d’eux en est un, `[1, s, 0]`.
3. Une variable n’est jamais convertie : `n * x`, `n` un `i32`, reste refusé (`E0210`), avec la correction `n.toF64()`. Là où une valeur change de type, la source le dit.
4. `E0211` est retiré : sa page reste, marquée comme telle. Les options a et c restent constructibles derrière `PERAS_PROTO_NUM`, pour le script de mesure seulement.

## Critères de l’étape, enregistrés avant son code

1. **Tests UI** : une page de littéraux utilisés comme flottants se vérifie sans erreur, `f64` et `f32`, négatifs, dans une opération, une comparaison, une affectation, un retour, un argument et un tableau littéral ; `n * x` avec `n` un `i32` reste refusé, avec la correction `n.toF64()` ; le test d’`E0211` montre qu’il se construit.
2. **De bout en bout, sur les deux allocateurs**, contre la même source construite en module managed : la même page calculée, `1 / 2` là où un `f64` est attendu donnant 0,5 dans les deux modes, un littéral `f32` arrondi en `f32`. Les mêmes résultats, rien de vivant.
3. **Le bassin tel qu’écrit d’abord se construit** et dessine les pixels du bassin : `node scripts/bench-numbers.mjs --check` ; les scripts qui comptent sur les refus d’aujourd’hui, `bench-lanterns.mjs` pour sa version gardée, demandent l’option a explicitement (constat M007).
4. **La règle cassée exprès est attrapée** : un littéral typé flottant abaissé en constante entière, dans l’abaissement, est refusé par le vérificateur de libération ou fait échouer les tests de bout en bout.
5. **Mesures**, rapportées, pas des conditions : `node scripts/bench-numbers.mjs` au commit de l’étape ; l’A/B du tableau de bord contre le commit d’avant (`ab-data.mjs`).
6. **Docs**, en anglais et en français : la page du guide sur les nombres, la référence des types, la page d’`E0211`, l’état, le constat F112, le journal des modifications, `CLAUDE.md`.

## Registre

_Chaque partie est consignée ici à son arrivée._

- **2026-11-17, les options mesurées** (`node scripts/bench-numbers.mjs`, [résultats](../../../bench/numbers/RESULTS.md) ; prototypes à `bb91877`, derrière `PERAS_PROTO_NUM`). Un Intel Xeon à 2,30 GHz, 4 cœurs, Node 24.
  - **Facilité d’écriture**, les cinq programmes et P5′ :

    |       | Programmes qui se construisent tels qu’écrits | Erreurs rencontrées | Lignes changées | Résultats qui diffèrent du JavaScript tel qu’écrit |
    | ----- | --------------------------------------------: | ------------------: | --------------: | -------------------------------------------------: |
    | a     |                                       0 sur 6 |                  68 |             108 |                                                  2 |
    | **b** |                                       2 sur 6 |                   7 |              10 |                                                  2 |
    | c     |                                       6 sur 6 |                   0 |               0 |                                                  2 |

    Le bassin tel qu’écrit d’abord, 45 erreurs aujourd’hui, se construit avec b et avec c. Ce que b refuse encore, c’est une _variable_ entière utilisée comme flottant : `sum / xs.length` (P2), `cents * 1.2` (P4), le `t` de `i / (steps - 1)` (P5, P5′), chacune avec la correction `.toF64()`.

  - **Ce que chaque option laisse passer** : les deux mêmes résultats diffèrent de JavaScript dans chaque option, et aucun n’est le fait d’une option. Le premier prix de P4, 9 007 199 254 740 993, dépasse ce que tient un nombre JavaScript : JavaScript perd son dernier chiffre, l’`i64` de Peras le garde. Le `i / (count - 1)` de P5′ est une division entière en Peras, une fraction en JavaScript : 1 contre 2,5, quelle que soit l’option ; a et b refusent `total += t` et leur correction, `t.toF64()`, convertit l’entier déjà tronqué (constat F113). Chaque programme donne le même résultat en owned sur les deux allocateurs et en managed.
  - **Vitesse, charge et octets** : le bassin tel qu’écrit, construit par b et par c, contre le bassin corrigé, construit par a, image 600, 15 tours : 1,033 (0,937 à 1,108) et 1,009 (0,886 à 1,095), la charge du processeur 1,00 ; les modules à 5 octets l’un de l’autre (14 647 et 14 652, 5 591 à 5 592 en gzip), les mêmes pixels. Aucune option ne change le programme qu’elle compile ; seulement ce qu’elle accepte.
- **Écarts au protocole.**
  - **P5′ a été ajouté après l’enregistrement** : le total de P5 ne dépend pas de `t`, il ne pouvait donc pas montrer une division entière.
  - **Les résultats sont comparés aux programmes écrits en JavaScript**, dans `scripts/bench-numbers.mjs`, en plus du module managed : un module managed suit les mêmes règles que l’owned, il ne peut donc pas montrer ce qu’une option change face à JavaScript.
  - **P4 ne teste pas la perte de chiffres de c** : sa taxe s’applique à un prix sous 2⁵³ ; c convertit un `i64` en flottant sans marque dans la source, ce qu’une mesure des résultats ne peut pas montrer, JavaScript perdant les mêmes chiffres.
- **Ce que disent les mesures** : b retire 61 des 68 erreurs (90 %) sans conversion que la source ne montre pas, sans coût en vitesse ni en octets ; c retire les 7 dernières en convertissant en silence les variables entières, comme JavaScript le fait et comme Swift, Go et Rust ne le font pas. Ni l’une ni l’autre n’attrape la division entière de P5′, une question à part (F113). b est l’option à proposer au propriétaire, qui choisit.
- **2026-11-17, l’étape : l’option b par défaut** (`fdc341c` ; `PERAS_PROTO_NUM` nomme une autre option pour le script de mesure seulement, ou `before`, le compilateur d’avant cette décision). Ses critères :
  1. **Tests UI** : `tests/ui/check/literals-as-floats.peras` se vérifie sans erreur ; `E0210-integer-variable-as-float.peras` reste refusé avec la correction `.toF64()` ; `E0211-integer-for-float.peras` se construit. Atteint.
  2. **De bout en bout, sur les deux allocateurs** (`packages/vite-plugin-peras/e2e/numbers.test.ts`), contre la même source construite en module managed : les mêmes résultats, `1 / 2` donnant 0,5, une somme `f32` de `1 / 3` arrondie en `f32` dans les deux modes, rien de vivant. Atteint.
  3. **Le bassin tel qu’écrit d’abord se construit** et dessine les pixels du bassin (`node scripts/bench-numbers.mjs --check`) ; `bench-lanterns.mjs` compte les 45 erreurs de sa version gardée avec l’option a. Atteint.
  4. **La règle cassée exprès est attrapée** : le littéral flottant de l’abaissement redevenu constante entière, le vérificateur de libération refuse le module de bout en bout. Atteint.
  5. **Mesures**, à `fdc341c` : `node scripts/bench-numbers.mjs` redonne 68, 7 et 0 erreurs ; le bassin tel qu’écrit par b, 1,039 (0,847 à 1,307) du bassin corrigé par a, les mêmes pixels ; l’A/B du tableau de bord contre `00d73af`, 20 tours, 0,998 (0,842 à 1,106) : aucun changement.
  6. **Docs** : la page du guide sur les nombres, la référence des types, la spécification, la page d’`E0211` et l’index des codes, l’état, le constat F112 fermé et F113 ouvert, le journal des modifications, `CLAUDE.md`. Atteint.
- **Écarts aux critères** : le test des exemples documentés comptait un exemple par code ; la page d’un code retiré montre des lignes qui se construisent, donc les codes retirés en sont exclus, comme ils l’étaient déjà du test qui veut un test UI par code.
