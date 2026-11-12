# 0018 : Les grandes tables, d’après un profil du tableau de bord à 4 000 000 de lignes

_English version: [../../en/decisions/0018-large-maps.md](../../en/decisions/0018-large-maps.md)._

_Statut : proposée le 2026-11-12. Chaque changement est décidé par un A/B de deux compilateurs dans un même processus à 4 000 000 de lignes (constat M017), puis mesuré par une nouvelle exécution du protocole de la [décision 0014](0014-data-at-scale.md), inchangé._

## Contexte

L’exécution 4 de la décision 0014 a laissé une hypothèse aller dans le mauvais sens. H6 demande que le temps de Peras à 4 000 000 de lignes soit au plus 4,4 fois son temps à 1 000 000. Il valait 3,80 à l’exécution 2, 4,42 à l’exécution 3 et 4,88 à l’exécution 4. La décision 0017 a retiré 11 % du temps à 1 000 000 de lignes mais seulement 2 % à 4 000 000. Quelque chose croît plus vite que les lignes, et ce n’est pas le travail par ligne que la 0017 a retiré.

**Le profil.** `node scripts/profile-data.mjs --rows N --only …` chronomètre les mêmes variantes de `bench/data/dashboard.peras` à 1 000 000 et à 4 000 000 de lignes, dans une même session, au commit `8417667`. Les chiffres sont des médianes de 9 tours, dans Node.

| Ce qui est retiré                                 | 1 000 000 de lignes | 4 000 000 de lignes | 4M / 1M |
| ------------------------------------------------- | ------------------- | ------------------- | ------- |
| Rien : le tableau de bord tel qu’écrit            | 278 ms              | 1 265 ms            | 4,55    |
| Tout sauf le parcours des lignes                  | 21 ms en tout       | 85 ms en tout       | 4,05    |
| Tout sauf la lecture et le filtre                 | 137 ms en tout      | 521 ms en tout      | 3,80    |
| La table des paires                               | 54 ms               | 245 ms              | 4,5     |
| La table des produits                             | 15 ms               | 65 ms               | 4,3     |
| L’`add` d’un utilisateur à l’ensemble de son pays | 69 ms               | 420 ms              | 6,1     |
| Le chiffre d’affaires par jour                    | dans le bruit       | dans le bruit       |         |

Tout croît avec les lignes, sauf une chose : ajouter un utilisateur à l’ensemble de son pays. Chaque ligne gardée coûte environ 140 ns à 1 000 000 de lignes et environ 214 ns à 4 000 000. Un coût par opération qui monte avec la taille des données est la marque d’une mémoire qui n’est plus dans le cache. Les ensembles tiennent plus d’utilisateurs distincts quand le fichier grandit : jusqu’à 200 000 utilisateurs et 20 pays, donc jusqu’à des millions de paires à 4 000 000 de lignes. Le constat F215 a déjà écarté le hachage.

**Où sont les défauts de cache.** Une table ou un ensemble de Peras garde l’ordre d’insertion, comme les dicts de Python depuis 3.6 : des entrées dans l’ordre, et un index de numéros d’entrée. Une recherche lit une case de l’index, puis l’entrée que cette case désigne, pour comparer sa clé. Dans un grand ensemble, les deux sont hors du cache, donc chaque recherche coûte deux défauts. La plupart des `add` du tableau de bord trouvent leur utilisateur déjà là, et un tel `add` ne change rien : il paie les deux défauts seulement pour l’apprendre.

## La technique

**La clé dans l’index, pour les clés entières de 32 bits.** Kulukundis, _Designing a Fast, Efficient, Cache-friendly Hash Table, Step by Step_, CppCon 2017 (les tables suisses d’Abseil), garde quelques bits de chaque hachage à côté de sa case, pour qu’un sondage lise rarement une entrée qui n’est pas celle cherchée. Pour une clé de 32 bits, on peut garder davantage : la clé elle-même.

- Une case de l’index de 8 octets tient le numéro de l’entrée et la clé.
- Une recherche qui trouve sa clé lit une case, et aucune entrée.
- Une recherche qui rencontre une autre clé passe son chemin sans lire l’entrée de cette clé non plus.

L’ordre d’insertion est gardé tel quel, dans les entrées. Le parcours ne change pas.

**Où elle s’applique.** Le compilateur connaît le type des clés d’une table. `Set<i32>` et `Map<i32, …>` recevraient le nouvel index ; les tables à clés de 64 bits et à clés textuelles gardent le leur. Pour les clés textuelles, la même idée avec quelques bits du hachage s’appliquerait. C’est une seconde étape, décidée par sa propre mesure.

**Ce qu’elle coûte.** L’index prend deux fois plus de mémoire : 8 octets par case au lieu de 4, au plus un tiers vide. Pour un ensemble de 1 000 000 d’utilisateurs, cela fait environ 4 Mo de plus. Une mesure de la mémoire fait partie de la décision : H3 de la décision 0014 et le pic à l’exécution 5.

**Attendu.** Les `add` qui trouvent leur utilisateur déjà là paient un défaut au lieu de deux. Si les défauts sont le coût, cela retire l’essentiel de ce qui passe de 140 à 214 ns par ligne, et une part des 140. C’est une estimation : un profil ne peut pas retirer un défaut de cache. Seul l’A/B le dira.

## Ce qui ne change pas

- L’ordre de parcours, qui est l’ordre d’insertion, et chaque résultat. Les tests de modèle des tables et des ensembles du runtime comparent les deux avec un modèle en Rust, sur les deux allocateurs, et gagnent des ensembles de clés `i32`.
- SipHash, la protection contre des clés choisies pour entrer en collision (F215).
- Le protocole de la 0014, et `dashboard.peras`, qui n’est pas réécrit.
- Le test différentiel (M015) : 2 000 graines, et une fois avec le changement cassé exprès.

## Critères, enregistrés avant tout code

Enregistrés le 2026-11-12, avant que le changement soit écrit. Le nouvel index n’est gardé que si les trois tiennent :

1. **4 000 000 de lignes.** `node scripts/ab-data.mjs` à 4 000 000 de lignes donne une médiane avant / après d’au moins 1,10 à chacune de trois exécutions.
2. **1 000 000 de lignes.** Le même A/B à 1 000 000 de lignes donne une médiane d’au moins 0,98 à chacune de trois exécutions : pas de perte de plus de 2 %.
3. **Mémoire.** Une nouvelle exécution de la décision 0014 donne H3, le pic de mémoire de C sur celui de A à 4 000 000 de lignes, au plus 0,90. Il valait 0,78 aux exécutions 3 et 4.

Si l’un échoue, le changement est retiré : le commit annulé, les mesures gardées, le constat écrit.
