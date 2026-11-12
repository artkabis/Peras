# 0017 : Agréger sans copies, d’après un profil du tableau de bord

_English version: [../../en/decisions/0017-aggregating-without-copies.md](../../en/decisions/0017-aggregating-without-copies.md)._

_Statut : proposée le 2026-11-12, close le même jour (voir le [Bilan](#bilan)). L’étape 1 est arrivée à `037bb74`, l’étape 2 à `02ff9c2`, l’étape 3 à `a497971` ; l’étape 4 examinée et pas faite (voir [Étape 4, examinée](#étape-4-examinée--pas-faite) et [Mesuré après l’étape 1](#mesuré-après-létape-1), [après l’étape 2](#mesuré-après-létape-2) et [après l’étape 3](#mesuré-après-létape-3)). Chaque changement est mesuré quand il arrive : d’abord par le profil ci-dessous, dans Node, puis par une nouvelle exécution du protocole de la [décision 0014](0014-data-at-scale.md), inchangé. Un changement qui n’y montre pas son gain est rapporté comme tel._

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

## Mesuré après l’étape 2

L’étape 2 est arrivée au commit `02ff9c2`, en deux parties, chacune un petit changement à part.

- **`m.get(k).unwrapOr(repli)` abaissé d’un seul tenant**, quand l’option n’est qu’une étape. Une recherche ; chaque scalaire de la valeur lu seulement si l’entrée est là ; un tampon copié une fois ; le repli construit seulement si la clé manque. Avant, l’option se voyait construire un tampon vide pour une valeur absente, et sa chaîne était copiée deux fois. Un repli qui n’est pas calme s’exécute toujours d’abord, après la lecture de l’entrée, comme écrit.
- **Un `set` d’une clé que la passe de recherche connaît reçoit l’entrée.** `map_put_at_i`, `map_put_at_s` et `map_put_at_s_lent` libèrent l’ancienne valeur et la clé sans chercher la clé à nouveau. La passe n’en donne une que là où la table et la clé n’ont pas pu changer depuis la recherche : la même règle que dans la décision 0016. Une clé entière élargie depuis une variable locale `i32` lui est désormais connue aussi.

**Ce qui la garde.** Chaque garde doit échouer si l’étape est fausse :

- Le runtime vérifie que l’entrée qu’on lui donne tient la clé. Si ce n’est pas le cas, il cherche la clé comme `set` l’a toujours fait : une entrée fausse coûte du temps, jamais une table fausse. Les tests du runtime lui donnent exprès des entrées fausses, sur les deux allocateurs.
- L’interpréteur de l’IR s’arrête sur une entrée fausse, donc une preuve fausse de la passe fait échouer le test différentiel.
- Le test différentiel gagne des tables de structures sorties, changées et remises, un repli qui n’est pas calme, et une clé ou une table changée entre la recherche et le `set`. 2 000 graines passent. Trois mutations le font chacune échouer :
  - la passe qui n’oublie pas les changements d’une table ;
  - toute entrée connue de la table donnée à un `set`, quelle que soit sa clé ;
  - la valeur de l’entrée lue avant le choix, alors qu’elle peut manquer.
- Un test de bout en bout change sur place des entrées de structures à travers la frontière JavaScript, sur les deux allocateurs, y compris une clé supprimée entre les deux. Les tests de bout en bout passent, 118 sur 118.

**Elle est prise.** En comptant les appels au runtime (`node scripts/profile-data.mjs --calls`), chaque ligne gardée :

- libère un bloc 10 fois au lieu de 12 ;
- copie une chaîne 6 fois au lieu de 7 ;
- construit un tampon vide de moins ;
- cherche chaque clé des paires et des produits une seule fois, avec `map_put_at_s` et `map_put_at_i` à la place de `map_put_s` et `map_put_i`.

**Le gain : 8,5 à 13,5 %.** Deux profils lancés à une heure d’écart ne pouvaient pas le montrer. `full` est passé de 304 à 310 ms, mais les variantes que cette étape ne touche pas ont ralenti autant : le seul parcours des lignes est passé de 21 à 24 ms. La machine avait ralenti entre les deux (constat M017). Les deux compilateurs ont donc été comparés dans un même processus avec `node scripts/ab-data.mjs 037bb74 02ff9c2` : le même texte résumé par chacun à tour de rôle, l’ordre alterné, et le rapport pris tour par tour. Sur quatre telles exécutions, avant / après valait 1,085, 1,097, 1,104 et 1,135 en médiane. Les résumés étaient identiques.

## Mesuré après l’étape 3

L’étape 3 est arrivée au commit `a497971`, sous une autre forme que prévu. Avant de construire la clé par morceaux (technique 2), un profil par retrait a demandé ce que coûte la clé. `` `${country}|` `` à la place de `` `${country}|${category}` `` faisait gagner 25 ms sur 32. Cette variante désignait l’entier : `${category}` devenait une chaîne à part, copiée dans la clé, puis libérée.

**Le changement.** Un entier d’un gabarit est évalué à sa place :

- `fmt_int_len` compte ses octets, pour que la chaîne du gabarit soit dimensionnée une fois ;
- `fmt_int_append` écrit ses chiffres dans cette chaîne.

Chaque gabarit fait donc une allocation et une libération de moins pour chacun de ses entiers. C’est général, pas seulement pour les clés, et plus petit que la technique 2.

**Ce qui la garde.**

- Un test de bout en bout compare des gabarits d’`i32` et d’`i64` à leurs bornes avec les gabarits de JavaScript lui-même, sur les deux allocateurs.
- Le test différentiel gagne des gabarits d’entiers négatifs, nuls et larges. 2 000 graines passent.
- Deux mutations le font chacune échouer : un chiffre mal compté, et le signe mal écrit. Le test de bout en bout attrape aussi le signe.

**Elle est prise.** Dans `--calls`, `fmt_int` a disparu, et chaque ligne gardée libère un bloc 9 fois au lieu de 10.

**Le gain : 1,3 à 7,4 %, moins que ce qu’annonçait le profil.** `node scripts/ab-data.mjs f989a03 a497971` a donné une médiane avant / après de 1,074, 1,034 et 1,013 sur trois exécutions, les mêmes résumés.

La variante avait changé deux choses, ce qui est encore le constat M013. Sans la catégorie, la table tenait une clé par pays au lieu de jusqu’à douze : une table plus petite, plus souvent en cache. Les 25 ms étaient le coût de l’entier et celui de la table plus petite ensemble, et seul le premier est ce que retire cette étape. La technique 2, la clé pas construite du tout, n’est pas poursuivie maintenant. La clé est construite une fois par ligne gardée, et la construire ne fait plus qu’une allocation.

## Étape 4, examinée : pas faite

L’étape 4 reposait sur une affirmation que cette décision a faite sans lire le runtime : « `parseI32` demande au runtime si le texte est un entier, puis lui demande l’entier : deux passes sur les mêmes octets. » Ce n’est pas ce que fait le runtime. Depuis le constat F201, `str_is_int` lit la valeur dans le passage même qui la vérifie, et la garde dans une globale. `str_parse_int` ne fait que rendre cette globale, un appel que le moteur intègre maintenant que le runtime est lié dans le module. L’étape n’aurait donc presque rien retiré, et elle n’a pas été construite (constat M019).

Ce que coûte la lecture a été mesuré à la place, par retrait. `node scripts/profile-data.mjs` gagne des variantes construites sur `parse-only`, où rien ne s’exécute après le filtre : une variante qui change les lignes gardées ne change donc rien d’autre. Les chiffres valent pour 1 000 000 de lignes, en une exécution, au commit `ad8803f`.

| La lecture, une chose retirée                             | Temps  | Ce qu’elle coûte |
| --------------------------------------------------------- | ------ | ---------------- |
| Lues et filtrées, rien d’agrégé                           | 135 ms |                  |
| Les cinq entiers remplacés par leur longueur, pas lus     | 102 ms | environ 33 ms    |
| Sans le contrôle des lettres du pays                      | 114 ms | environ 21 ms    |
| Pas de split : une ligne faite de la longueur de la ligne | 47 ms  | environ 88 ms    |
| Le seul parcours des lignes                               | 21 ms  |                  |

Les cinq entiers coûtent environ 6,6 ns chacun. Le découpage en vues et la copie du pays coûtent environ 34 ms, le contrôle des lettres environ 21. Aucune de ces parts n’est grande seule. L’étape qui les retirerait ensemble est celle à laquelle a mené le profil du TypeScript optimisé : lire les champs d’une ligne directement dans ses octets, sans ses vues, comme le fait B (Langdale et Lemire, 2019).

## Bilan

Close le 2026-11-12. Les étapes 1 à 3 sont arrivées ; l’étape 4 a été examinée et pas construite (M019) ; l’étape 5, l’index qui garde des bits du hachage, n’a pas été essayée. Son gain était une supposition, et l’exécution 4 de la décision 0014 désigne d’abord autre chose : ce qui croît plus vite que les lignes.

| Étape | Quoi                                                             | Mesuré, un A/B dans un même processus  | Commit    |
| ----- | ---------------------------------------------------------------- | -------------------------------------- | --------- |
| 1     | Un repli qui copie, construit seulement s’il sert                | 322 à 304 ms, 8 à 28 ms                | `037bb74` |
| 2     | Une entrée de table sortie, changée et remise, cherchée une fois | 8,5 à 13,5 % plus rapide               | `02ff9c2` |
| 3     | Les entiers d’un gabarit écrits dans sa chaîne                   | 1,3 à 7,4 % plus rapide                | `a497971` |
| 4     | Un appel par entier                                              | pas construite : pas de second passage | (M019)    |

Sous le protocole de la décision 0014, exécution 4 contre exécution 3, même processeur :

- Peras écrit simplement passe de 2,06–2,36 à 2,39–2,63 fois plus rapide que TypeScript simple ;
- face au TypeScript optimisé à la main, il passe de 0,25–0,28 à 0,26–0,31 ;
- en millisecondes, 11 % de moins à 1 000 000 de lignes, 2 % de moins à 4 000 000.

L’espoir écrit plus haut était d’environ 200 ms pour 1 000 000 de lignes dans Node et 0,45 de B. Il n’est pas atteint : environ 280 ms, et 0,26 à 0,31 de B. L’étape 4 reposait sur un coût qui n’existait pas, et le profil de l’étape 3 comptait deux changements à la fois (M013).

**Ce que cette décision a appris du TypeScript optimisé.** Environ 130 ms de son avance viennent de ce qu’il sait des données : un bitmap d’utilisateurs, des tableaux denses de codes de pays et de produits. Un compilateur ne peut pas le déduire de code écrit simplement. Environ 70 ms viennent de la lecture des octets en une passe, ce qui revient au compilateur.

**La suite.**

- **Le tableau de bord à 4 000 000 de lignes, profilé.** Le gain à 4 000 000 de lignes est un cinquième de celui à 1 000 000, et H6 s’éloigne de sa borne.
- **Puis la lecture.** Les champs d’une ligne lus directement dans ses octets, sans vues : le découpage, la copie du pays et les entiers coûtent environ 88 ms pour 1 000 000 de lignes.
- **Un moyen pour un programme de dire ce qu’il sait de ses données** serait un changement du langage. Il se pèse à part.

Chacune de ces pistes est une décision à part.
