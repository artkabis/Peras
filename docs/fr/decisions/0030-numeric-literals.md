# 0030 : Des nombres écrits comme JavaScript les écrit

_Version anglaise : [../../en/decisions/0030-numeric-literals.md](../../en/decisions/0030-numeric-literals.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il répond au constat F112, la première des corrections que le propriétaire a ordonnées après la [décision 0029](0029-pond-of-lanterns.md)._

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

## Registre

_Chaque partie est consignée ici à son arrivée._
