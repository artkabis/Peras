# 0017 : Agréger sans copies, d’après un profil du tableau de bord

_English version: [../../en/decisions/0017-aggregating-without-copies.md](../../en/decisions/0017-aggregating-without-copies.md)._

_Statut : proposée le 2026-11-12. L’étape 1 est arrivée à `037bb74` (voir [Mesuré après l’étape 1](#mesuré-après-létape-1)). Chaque changement est mesuré quand il arrive : d’abord par le profil ci-dessous, dans Node, puis par une nouvelle exécution du protocole de la [décision 0014](0014-data-at-scale.md), inchangé. Un changement qui n’y montre pas son gain est rapporté comme tel._

## Contexte

À l’exécution 3 de la décision 0014, Peras écrit simplement calcule le tableau de bord commercial 2,06 à 2,36 fois plus vite que TypeScript simple. Le TypeScript optimisé à la main reste 3,6 à 4 fois plus rapide que lui. La décision 0016 a retiré les coûts que l’application de flux et le tableau de bord partagent : les littéraux, les appels au runtime, les petits blocs, une recherche faite deux fois, la recherche des séparateurs. Ce qui reste est surtout propre au tableau de bord : son agrégation.

**Le profil.** `node scripts/profile-data.mjs` chronomètre des variantes de `bench/data/dashboard.peras` qui retirent ou remplacent chacune une seule chose, au commit `989c3c4`. Les chiffres valent pour 1 000 000 de lignes, dont environ 490 000 gardées, médiane de 9 tours, les variantes alternées. Le tableau de bord tel qu’écrit prend 322 ms.

| Ce qui est retiré ou remplacé                                      | Gain         | Constat                |
| ------------------------------------------------------------------ | ------------ | ---------------------- |
| Toute l’agrégation : les lignes sont seulement lues et filtrées    | 190 ms, 59 % |                        |
| La table des paires, à clé `` `${country}\|${category}` ``         | 96 ms, 30 %  | [F216](../findings.md) |
| … dont la clé construite par un gabarit, le pays seul à la place   | 27 ms, 8 %   | [F202](../findings.md) |
| … dont la copie du pays dans le repli de `unwrapOr`                | 11 ms, 3 %   | [F209](../findings.md) |
| La table des pays et les ensembles d’utilisateurs                  | 76 ms, 24 %  |                        |
| … dont seulement l’`add` d’un utilisateur à l’ensemble de son pays | 64 ms, 20 %  | [F215](../findings.md) |
| La table des produits, `get` puis `set` d’une clé entière          | 22 ms, 7 %   | [F216](../findings.md) |
| Le contrôle des lettres du pays                                    | 23 ms, 7 %   | [F207](../findings.md) |
| Le chiffre d’affaires par jour, un tableau indexé par jour         | 9 ms, 3 %    |                        |
| Une seconde copie du pays, que la ligne pourrait prêter            | 7 ms, 2 %    | [F214](../findings.md) |
| Lire quoi que ce soit : le texte passé, un résumé rendu            | 8 ms en tout |                        |

Les lignes seules sont parcourues en 22 ms, lues et filtrées en 133.

**Ce que fait chaque ligne gardée.** Le même tableau de bord a été construit avec le runtime à côté du module (`--link off`), et chaque appel au runtime compté, sur 100 000 lignes. Chaque ligne gardée appelle le runtime environ 40 fois. Elle copie une chaîne 8 fois et libère un bloc 13 fois. Chaque entier est lu par deux appels : `str_is_int`, puis `str_parse_int`. La table des paires en est la cause principale. `let pair = pairs.get(key).unwrapOr(…)` copie hors de la table la `Pair` de l’entrée, son pays compris. Puis `pairs.set(key, pair)` cherche la clé à nouveau, libère la clé passée et la valeur qu’elle remplace, et range la copie.

**Ce qui ne l’expliquait pas.** L’`add` d’un utilisateur coûte 64 ns. SipHash n’en est pas la cause. Le runtime a été reconstruit avec une multiplication au lieu de SipHash pour les clés entières, et le tableau de bord a pris 342 ms au lieu de 322 : pas plus rapide (F215). Les ensembles sont grands, jusqu’à quelque 100 000 utilisateurs par pays. Une recherche lit l’index, puis l’entrée qu’il désigne, et les deux sont hors du cache. Le TypeScript optimisé garde à la place un bitmap de 32 Ko par pays. Il sait par les données que les utilisateurs sont sous 2^18, ce qu’un compilateur ne peut pas savoir.

## Les techniques

**1. Mettre à jour une entrée sur place** (F216). Lorenzen et Leijen, _Reference Counting with Frame Limited Reuse_, ICFP 2022, et Reinking, Xie, de Moura et Leijen, _Perceus_, PLDI 2021, compilent une valeur défaite puis refaite comme une mise à jour de la même mémoire, quand rien d’autre ne la tient. Morel et Renvoise, 1979, déjà appliqués dans la décision 0016, retirent la seconde recherche. Le motif ici est : `let v = m.get(k).unwrapOr(d)`, puis des changements de `v`, puis `m.set(k, v)`, sans autre usage de `m` entre les deux. Le changement : l’IR le reconnaît et change l’entrée là où elle est. Il fait une seule recherche, et ne met `d` que si `k` manque. Rien n’est copié hors de la table, et ni la clé ni l’ancienne valeur ne sont libérées. La bibliothèque standard de Rust offre la même chose en API, `HashMap::entry` ; ici, le compilateur la trouve dans du code écrit simplement. **Attendu** : l’essentiel des 96 ms des paires, moins les 27 de la clé, et une part des 22 des produits.

**2. Ne pas construire une clé seulement cherchée** (F202). Wadler, _Deforestation: Transforming Programs to Eliminate Trees_, Theoretical Computer Science 73(2), 1990, retire les structures intermédiaires qu’un programme construit seulement pour les consommer. Une clé faite d’un gabarit, `` `${country}|${category}` ``, en est une quand elle est trouvée. Le changement : ses morceaux sont hachés l’un après l’autre, comme le seraient les octets du tout. SipHash prend son entrée par morceaux (Aumasson et Bernstein, INDOCRYPT 2012). La clé est comparée à l’entrée morceau par morceau, et elle n’est construite que si elle est insérée. **Attendu** : l’essentiel des 27 ms.

**3. Un repli calme peut copier** (F209). La décision 0016 ne construit le repli de `unwrapOr` que si l’option est vide, quand le construire n’a aucun effet qu’un programme puisse voir. La copie d’une valeur n’en a aucun : seulement une allocation, que le programme ne peut pas observer. Le changement : `x.copy()` d’une valeur calme est calme. **Attendu** : les 11 ms, et un bloc par ligne.

**4. Lire un entier en un seul appel** (lecture). `parseI32` demande au runtime si le texte est un entier, puis lui demande l’entier : deux passes sur les mêmes octets. Le changement : un seul appel rend les deux, la valeur et si c’en est une. **Attendu** : une part des 111 ms de lecture ; petite, mesurée.

**5. Moins de défauts de cache dans les grands ensembles** (F215). Kulukundis, _Designing a Fast, Efficient, Cache-friendly Hash Table, Step by Step_, CppCon 2017 (les tables suisses d’Abseil), garde quelques bits de chaque hachage dans l’index, pour que la plupart des comparaisons ne lisent pas l’entrée. Celis, _Robin Hood Hashing_, thèse, Université de Waterloo, 1986, garde les sondages courts. Les tables de Peras gardent l’ordre d’insertion, comme celles de Python depuis 3.6 : des entrées dans l’ordre, et un index de numéros. Le changement : l’index garde les bits hauts du hachage à côté du numéro de l’entrée, pour qu’un sondage sur une autre clé ne lise pas son entrée. C’est une supposition, pas une mesure : une recherche qui trouve sa clé lit toujours son entrée. **Attendu** : incertain. C’est essayé en prototype, mesuré, et gardé seulement si le profil montre un gain.

## Ordre

D’après ce que montre le profil, et selon le risque, chacun mesuré avant le suivant :

1. Un repli calme qui copie (3) : petit, et le test différentiel le couvre.
2. La mise à jour sur place (1) : la plus grande.
3. Une clé non construite (2).
4. Un appel par entier (4).
5. L’index avec les bits du hachage (5), en prototype.

Si les étapes 1 à 4 donnent chacune l’essentiel de ce que montre le profil, le tableau de bord passe d’environ 322 à environ 200 ms pour 1 000 000 de lignes dans Node. Peras serait alors environ 0,45 fois aussi rapide que le TypeScript optimisé à la main, contre 0,25 à 0,28. C’est un espoir, pas un résultat : un profil par retrait donne le plus qu’un changement puisse faire gagner (constat M013). La parité n’est pas le but. Le TypeScript optimisé se sert de ce qu’il sait des données : 676 codes de pays, des utilisateurs sous 2^18. Un compilateur ne peut pas le savoir à partir de code écrit simplement.

## Ce qui ne change pas

- Le protocole de la 0014, ses versions A, B et C, ses données et ses seuils. `dashboard.peras` n’est pas réécrit pour gagner : le gain doit venir du compilateur.
- La sûreté. Chaque changement garde les vérifications de l’allocateur de debug, le vérificateur de release, les tests des deux allocateurs, et le test différentiel : 2 000 graines une fois, et une fois avec le changement cassé exprès (M015). Le générateur du test différentiel gagne des tables de structures mises à jour par `get` puis `set`, et des clés construites par gabarit.
- SipHash reste : il protège les tables contre des clés choisies pour entrer en collision, et ce n’est pas lui qui coûte ici (F215).

## Comment le TypeScript optimisé atteint sa vitesse

`node scripts/profile-optimized.mjs` profile la version B de la même façon : chaque variante de `bench/data/dashboard-optimized.ts` remplace une de ses techniques par ce que fait du code simple. Les chiffres valent pour 1 000 000 de lignes dans Node, médiane de 9 tours, au commit `037bb74`.

| B, avec une technique remplacée                                                     | Temps  | Ajoute |
| ----------------------------------------------------------------------------------- | ------ | ------ |
| B tel qu’écrit                                                                      | 106 ms |        |
| Chaque utilisateur dans un `Set` par pays, au lieu d’un bitmap                      | 186 ms | 80 ms  |
| Les paires dans une `Map` à clé chaîne construite par ligne                         | 133 ms | 26 ms  |
| Les paires dans une `Map` à clé numérique                                           | 109 ms | 3 ms   |
| Chaque produit dans une `Map`, au lieu d’un tableau dense                           | 111 ms | 5 ms   |
| Les trois : la lecture des octets de B, avec les tables et ensembles du code simple | 236 ms | 130 ms |
| Pour comparer, `TextDecoder` seul, les octets en la chaîne de C                     | 18 ms  |        |

**Ce que cela dit.** B est rapide pour deux raisons, de taille inégale.

- **L’essentiel de son avance est la connaissance des données, environ 130 ms sur 236.** B sait trois choses que le code ne dit pas :
  - les utilisateurs sont sous 2^18, donc un bitmap de 32 Ko par pays les contient ;
  - un pays est fait de deux majuscules, donc c’est un nombre sous 676 ;
  - les produits sont sous 8 192.

  Le bitmap vaut à lui seul 80 ms. Un compilateur ne peut pas trouver ces faits dans du code écrit simplement. Le même `Set` coûte d’ailleurs plus en JavaScript qu’en Peras : 80 ms de plus dans V8, contre 48 à 64 pour les ensembles de Peras (F215).

- **Le reste est la lecture des octets en une passe, environ 70 ms.** B avec des structures simples prend 236 ms ; Peras en prend 304 (304 à 322, plus haut). B parcourt les octets une seule fois : il lit chaque entier en rencontrant ses chiffres, et n’alloue rien par ligne. Peras :
  - découpe chaque ligne en vues ;
  - demande deux fois chaque entier au runtime ;
  - contrôle le pays lettre par lettre ;
  - et, dans la mesure de la décision 0014, reçoit une chaîne que `TextDecoder` a faite à partir des octets, 18 ms que B ne paie pas.

  Cette part est celle du compilateur. Les étapes 2 à 4 la visent, et une technique de plus s’y applique : lire les champs d’une ligne directement dans ses octets, sans ses vues (Langdale et Lemire, 2019, déjà cités dans la décision 0016).

Peras écrit simplement pourrait donc rattraper B écrit avec des structures simples. Il ne rattrapera pas le bitmap de B en compilant seulement. Y arriver demanderait un moyen, dans le langage, pour qu’un programme dise ce qu’il sait : par exemple un ensemble d’entiers sous une borne, gardé en bitmap. C’est un changement du langage, et il ne fait pas partie de cette décision. Il serait proposé à part, son coût pour le langage pesé, et mesuré comme le reste.

## Mesuré après l’étape 1

L’étape 1 est arrivée au commit `037bb74`. Un repli de `unwrapOr` qui copie une valeur lue est désormais calme, donc construit seulement si l’option est vide (F209). Elle est correcte et testée. Le test différentiel gagne une table de chaînes, des replis qui copient et un repli qui déplace une variable. 2 000 graines passent. Avec le repli paresseux d’un tampon cassé exprès, le test échoue dès la première graine. Les tests de bout en bout des deux allocateurs passent, 116 sur 116.

Elle est prise : en comptant les appels au runtime, chaque ligne gardée copie une chaîne 7 fois au lieu de 8 et libère un bloc 12 fois au lieu de 13. Dans le profil, la variante sans la copie ne fait plus rien gagner (−1 ms). Le tableau de bord est passé de 322 à 304 ms pour 1 000 000 de lignes dans Node. C’est plus que les 11 ms qu’annonçait le profil, mais les variantes varient d’environ 10 ms (l’une donne −10), donc le gain est entre 8 et 28 ms.
