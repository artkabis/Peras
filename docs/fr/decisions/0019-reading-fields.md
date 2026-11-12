# 0019 : Lire plus vite les champs d’une ligne

_English version: [../../en/decisions/0019-reading-fields.md](../../en/decisions/0019-reading-fields.md)._

_Statut : proposée le 2026-11-12. Chaque étape est décidée par un A/B de deux compilateurs dans un même processus (`node scripts/ab-data.mjs`, constat M017), contre des critères enregistrés ici avant son code._

## Contexte

La décision 0017 a trouvé que le TypeScript optimisé doit environ 70 ms de son avance à la lecture des octets en une passe. La décision 0018 a laissé les grandes tables comme elles étaient. Cette décision prend la lecture.

**Le profil.** `node scripts/profile-data.mjs` donne des variantes construites sur `parse-only`, où rien ne s’exécute après le filtre. Les chiffres valent pour 1 000 000 de lignes, en une exécution, au commit `5d3c443`.

| La lecture, une chose retirée                             | Temps  | Ce qu’elle coûte         |
| --------------------------------------------------------- | ------ | ------------------------ |
| Lues et filtrées, rien d’agrégé                           | 139 ms |                          |
| Les cinq entiers remplacés par leur longueur, pas lus     | 100 ms | environ 39 ms            |
| Sans le contrôle des lettres du pays                      | 113 ms | environ 26 ms            |
| La ligne découpée et ses champs comptés, aucun lu         | 80 ms  |                          |
| Pas de split : une ligne faite de la longueur de la ligne | 48 ms  | le split : environ 32 ms |
| Le seul parcours des lignes                               | 23 ms  |                          |

Les parts sont de même taille : les entiers environ 39 ms, le découpage environ 32, le contrôle des lettres environ 26. Lire les champs directement dans les octets, sans leurs vues (Langdale et Lemire, 2019), retirerait au plus les 32 ms du découpage. C’est aussi un gros changement : un champ passé à une fonction, comme `isCountry(f[2])`, a toujours besoin d’une chaîne, donc d’une vue.

**Ce que montre le code des entiers.** `str_is_int` vérifie la plage d’un entier en le lisant, contre une limite et son dixième. Il les calcule à chaque appel : un `i64.div_u` et un `i64.rem_u`. Une division de 64 bits est parmi les instructions les plus lentes d’un processeur. Pour les 32 et 64 bits que Peras compile, les deux sont des constantes :

- pour 32 bits, le dixième vaut 214 748 364 et le dernier chiffre 7, ou 8 après `-` ;
- pour 64 bits, le dixième vaut 922 337 203 685 477 580 et le dernier chiffre 7, ou 8 après `-`.

## Étapes

1. **La plage d’un entier vérifiée contre des constantes.** Aucune division pour 32 et 64 bits ; les autres largeurs les calculent comme aujourd’hui. Petit, et cela ne change aucun résultat.
2. **Les champs lus dans les octets.** Là où le découpage d’une ligne n’est lu que par `f.length` et `f[i]` à `i` constant, les séparateurs sont trouvés une fois et chaque champ est lu sur place. Un champ passé à une fonction garde une vue. Cette étape est décidée après l’étape 1, avec ses propres critères.

## Critères de l’étape 1, enregistrés avant son code

Gardée seulement si les deux tiennent, sinon retirée :

1. **À 1 000 000 de lignes.** `node scripts/ab-data.mjs` donne une médiane avant / après d’au moins 1,02 à chacune de trois exécutions.
2. **À 4 000 000 de lignes.** La même, au moins 0,98 à chacune de trois exécutions : pas de perte.

## Ce qui ne change pas

- Chaque résultat : un entier est accepté ou refusé exactement comme avant. Les tests de bout en bout comparent `parseI32` et `parseI64` avec JavaScript à leurs bornes.
- Le test différentiel passe 2 000 graines, et échoue avec le changement cassé exprès (M015).
