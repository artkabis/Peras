# 0014 : La donnée à l’échelle, agrégée dans le navigateur

_Version anglaise : [../../en/decisions/0014-data-at-scale.md](../../en/decisions/0014-data-at-scale.md)._

_Statut : protocole enregistré le 2026-11-11 (commit `96e6c43`), amendé le même jour avant toute mesure : DuckDB-wasm retiré (voir [Amendement](#amendement-avant-toute-mesure)). Exécution 1, au commit `b06da25` : cinq hypothèses réfutées, deux tiennent. Exécution 2, au commit `6ef6283`, après trois changements du compilateur : les mêmes verdicts, et Peras simple désormais 1,23 à 1,26 fois plus rapide que TypeScript simple, là où il était plus lent. Exécution 3, au commit `df51da3`, après la décision 0016 : H1 tient, Peras simple 2,06 à 2,36 fois plus rapide que TypeScript simple ; H6 réfutée de 0,02 ; sur une autre machine (un écart, les rapports comparés). Exécution 4, au commit `d9fbe35`, après les étapes 1 à 3 de la décision 0017 : Peras simple 2,39 à 2,63 fois plus rapide que TypeScript simple, 0,26 à 0,31 fois aussi rapide que TypeScript optimisé ; H6 réfutée plus nettement, 4,88. Exécution 5, au commit `8f98a87`, la version C recevant les octets (décision 0020) : H3 tient pour la première fois, le pic de mémoire de C à 0,23 fois celui de A, et Peras simple 2,52 à 3,00 fois plus rapide que TypeScript simple. Résultats et journal ci-dessous._

_Statut du protocole : enregistré avant toute mesure. Les résultats seront ajoutés sous lui, qu’ils confirment les hypothèses ou non ; le protocole lui-même ne changera pas après la première mesure, et tout écart sera signalé comme tel._

## Contexte

Le jardin d’Apeiron (décision 0013) a montré qu’un programme peut être plus petit que son image ; il n’a pas montré que Peras aide une entreprise. Son propriétaire a dit ce que cela veut dire : absorber beaucoup de données et en faire quelque chose sur la page web, sans friction. C’est une autre affirmation, sur l’ingestion, pas la génération, et elle a un solide concurrent : TypeScript, que chaque équipe web écrit déjà, simplement ou optimisé à la main.

Cette étude demande, avant d’en écrire une ligne, si Peras fait le travail d’un tableau de bord commercial plus vite, dans moins de mémoire et avec moins de friction que TypeScript, de cent mille à quatre millions de lignes. Elle est enregistrée pour que sa réponse ne puisse pas être pliée vers une démo : les données, la requête, les versions, les mesures et les seuils sont fixés ici.

Les risques sont ceux que la décision 0013 a nommés, et un de plus :

- **Une démo choisie pour gagner.** Les seuils sont fixés ici ; une hypothèse réfutée est annoncée réfutée, dans le README aussi.
- **Des versions qui ne font pas le même travail.** Chaque version doit donner le même résumé, octet pour octet, vérifié par une empreinte.
- **Un homme de paille.** Le TypeScript idiomatique est ce qu’un développeur web écrit sans optimiser ; un TypeScript optimisé à la main est mesuré à côté, pour que Peras ne soit pas jugé face au plus faible seulement.
- **Des données propres.** Les vrais exports portent des lignes cassées ; une ligne sur mille est cassée ici, et chaque version doit compter et écarter les mêmes.

## Les données

Des lignes d’événements de vente, comme un export CSV, produites par `scripts/bench-data.mjs` à partir d’une graine fixe avec l’empreinte entière de la décision 0013 (`lowbias32`). La même graine donne le même fichier ; son SHA-256 est consigné avec les résultats.

- **Tailles** : _N_ = 100 000, 1 000 000 et 4 000 000 lignes, environ 3,5 Mo, 35 Mo et 140 Mo.
- **En-tête**, puis une ligne par enregistrement, terminée par `\n` : `ts,user,country,product,amount,qty`.
- **`ts`** : secondes Unix en 2026, de 1 767 225 600 à 1 798 761 599, uniformes.
- **`user`** : 1 à 200 000, biaisé pour que certains clients achètent souvent : `1 + floor(u² × 200 000)`, _u_ uniforme dans [0, 1).
- **`country`** : un de 20 codes ISO 3166, deux lettres capitales, les premiers plus fréquents : indice `floor(u^1,5 × 20)`.
- **`product`** : 1 à 5 000, quelques meilleures ventes : `1 + floor(u³ × 5 000)`. Sa catégorie est `product % 12`, dérivée, pas une colonne.
- **`amount`** : le prix unitaire en centimes, 100 à 99 999.
- **`qty`** : 0 à 5 ; 0, une commande annulée, dans environ 2 % des lignes.
- **Lignes cassées** : environ une sur mille, de trois sortes à tour de rôle : un champ manquant ; un montant qui n’est pas un entier (`12a`) ; une ligne vide.

Une ligne est **bien formée** quand elle a exactement six champs ; que `ts`, `user`, `product`, `amount` et `qty` sont des entiers stricts (un signe facultatif, puis des chiffres ASCII, rien d’autre, dans la plage d’un entier signé de 32 bits) ; et que `country` est deux lettres de `A` à `Z`. Toute autre ligne non vide après l’en-tête est **cassée**. Une ligne est **retenue** quand elle est bien formée, que son `qty` est supérieur à 0, et que son `ts` est dans la seconde moitié de 2026, de 1 782 864 000 (1er juillet) à 1 798 761 600 (1er janvier 2027) exclu. Son **chiffre d’affaires** est `amount × qty`, en centimes, un entier exact.

## La requête

Ce que montre un tableau de bord commercial, calculé à partir des octets du fichier :

1. **Les comptes** : les lignes lues, les lignes cassées, les lignes retenues.
2. **Par pays et catégorie** : pour chaque paire présente parmi les lignes retenues, leur nombre et leur chiffre d’affaires.
3. **Les clients** : pour chaque pays, le nombre d’utilisateurs distincts parmi ses lignes retenues.
4. **Chaque jour** : le chiffre d’affaires de chacun des 184 jours du 1er juillet au 31 décembre, par `floor((ts − 1 782 864 000) / 86 400)`.
5. **Les meilleures ventes** : les 10 produits au plus fort chiffre d’affaires, à égalité le plus petit numéro de produit d’abord.

Le **résumé** est ces cinq parties, écrites en JSON sous une forme canonique : les clés dans l’ordre ci-dessus, les paires triées par pays puis catégorie, les pays triés, les entiers en décimal. Son SHA-256 doit être le même pour chaque version et chaque taille.

Puis la page **dessine un graphique** à partir du résumé : le chiffre d’affaires de chaque jour en courbe, celui de chaque pays en barres, sur un canvas de 1200 × 600 pixels. Le graphique est dessiné par une seule fonction JavaScript, partagée par toutes les versions : ce qui est comparé, c’est comment le résumé est obtenu, pas comment il est peint.

## Les versions

- **A. TypeScript, idiomatique** : le texte de `response.text()`, `split("\n")`, `split(",")`, les nombres vérifiés par une expression régulière et lus par `Number`, des `Map` et `Set` à clés textuelles, `Array.prototype.sort`. Écrit comme un développeur web l’écrit sans optimiser.
- **B. TypeScript, optimisé** : les octets du `Uint8Array`, lus sans créer de chaînes ; les entiers lus à la main ; les clés tassées en entiers ; les comptes dans des tableaux typés ; les utilisateurs distincts dans une table de bits.
- **C. Peras** : un module owned compilé en WebAssembly, écrit aussi simplement que A avec ce que le langage offre : `split`, `parseI32`, `Map`, `Set`, `sortByDescending`. Il reçoit le texte ou ses octets, ce que le langage passe avec le moins de copie, et ce choix est consigné.
- **D. Peras, en parallèle** : C, ses lignes découpées en autant de morceaux que de threads aux frontières de lignes, chacun agrégé dans un `parallel for` (décision 0007), puis fusionnés ; construit avec `--threads`, mesuré dans une page isolée cross-origin. Si `parallel for` ne peut pas l’exprimer, D est déclaré non exprimable, et c’est un résultat.

Chaque version est écrite d’après la spécification ci-dessus par les auteurs de Peras ; c’est une menace pour la validité, listée plus bas.

## Hypothèses

Chacune avec ce qui la réfute. La « vitesse » est l’inverse du temps médian jusqu’au résumé, à chaud ; « au moins _k_ fois plus rapide » compare des vitesses.

- **H1, Peras simple bat TypeScript simple.** C est au moins 2 fois plus rapide que A à 1 000 000 et à 4 000 000 lignes. _Réfutée_ si l’un des rapports est sous 2.
- **H2, Peras simple approche le TypeScript optimisé.** C est au moins 0,8 fois aussi rapide que B à 1 000 000 et à 4 000 000 lignes. _Réfutée_ si l’un des rapports est sous 0,8. Elle teste l’affirmation d’absence de friction : la vitesse sans écrire la version optimisée.
- **H3, moins de mémoire.** La mémoire résidente maximale de C vaut au plus la moitié de celle de A à 4 000 000 lignes. _Réfutée_ au-delà de la moitié.
- **H4, un petit programme.** C livre au plus 3 fois les octets de A. _Réfutée_ au-delà.
- **H5, les cœurs aident.** D est au moins 2 fois plus rapide que C à 4 000 000 lignes, sur les 4 cœurs de la machine. _Réfutée_ sous 2, ou si D ne peut pas être écrit.
- **H6, linéaire.** Le temps de C à 4 000 000 lignes vaut au plus 4,4 fois son temps à 1 000 000. _Réfutée_ au-delà.
- **H7, pas plus de code.** C compte au plus 1,25 fois les lignes de A. _Réfutée_ au-delà.

Le temps jusqu’au premier graphique, à froid, est mesuré et rapporté pour chaque version, et testé contre aucune hypothèse.

## Mesures

- **Même résumé** : SHA-256 du résumé canonique de chaque version, à chaque taille, tous égaux. Une version qui diffère est exclue des hypothèses qui la concernent, et cela est rapporté.
- **Temps jusqu’au résumé, à chaud** : des octets du fichier en mémoire, un `Uint8Array`, au résumé en objet JavaScript, décodage et copie éventuelle dans un module compris ; médiane de 15 exécutions après 2 d’échauffement, les versions en alternance, dans Chromium, à chaque taille.
- **Temps jusqu’au premier graphique, à froid** : un nouveau contexte de navigateur, rien en cache, le programme et les données servis par un serveur local ; du début de la navigation au graphique peint, l’image qui suit son tracé ; médiane de 7 chargements, à chaque taille.
- **Mémoire** : la mémoire résidente maximale du processus de rendu de la page, `VmHWM` dans `/proc/<pid>/status` de Linux, un nouveau processus par exécution, moins le maximum du même processus avec la version chargée et les données en mémoire avant que la requête tourne ; médiane de 5 exécutions, à chaque taille. Elle compte tout ce que la page tient : le tas JavaScript, les mémoires WebAssembly, les tampons.
- **Octets livrés** : le JavaScript et le WebAssembly que chaque version charge, minifiés par Vite là où cela s’applique, puis gzip.
- **Énergie** : celle du processeur, lue par RAPL là où la machine la donne ; déclarée non mesurée ailleurs, jamais estimée (décision 0012).
- **Code** : les lignes sans commentaires ni lignes vides, du code qui calcule le résumé à partir des octets ; le graphique et la page partagés ne sont pas comptés.

## Menaces pour la validité

- Les données sont générées ; les vrais exports ont d’autres distributions, d’autres largeurs, des champs entre guillemets. Le CSV ici n’a pas de guillemets, ce qui rend le travail de chaque analyseur plus simple que celui d’un vrai export.
- Les auteurs ont écrit chaque version ; un expert d’un langage peut l’écrire plus rapide.
- Les moteurs analytiques compilés en WebAssembly, comme DuckDB-wasm, ne sont pas mesurés : le projet ne prend aucune dépendance hors de Rust, pas même dans un banc (voir l’amendement). Toute comparaison avec eux citera leurs chiffres publiés, avec leur source et leurs conditions, et ne sera pas présentée comme une mesure de cette étude.
- Une machine, sans écran, 4 cœurs ; un téléphone peut ordonner les versions autrement. Le serveur local cache le réseau : les temps à froid seront aussi mesurés sous un réseau bridé, comme chiffre exploratoire.
- `VmHWM` compte ce qu’un processus a touché, pas ce dont il avait besoin ; une version qui rend sa mémoire plus tôt n’en est pas récompensée.

## Conséquences

- `bench/data`, `node scripts/bench-data.mjs`, et ses résultats sous ce protocole ; une vérification en CI que les versions donnent le même résumé à 100 000 lignes.
- Si les hypothèses tiennent, le README gagne une troisième démo : un tableau de bord commercial de millions de lignes, construit dans le navigateur, avec ses chiffres mesurés. Sinon, les résultats le disent, le README aussi, et celles réfutées nomment ce que le compilateur doit améliorer, ordonné comme la décision 0011 ordonne ses étapes : par la mesure.

## Amendement, avant toute mesure

Le 2026-11-11, après l’enregistrement (commit `96e6c43`) et avant qu’aucune version ne soit écrite ni aucune mesure prise, le propriétaire a fixé un principe : Peras ne prend aucune dépendance envers un autre langage que Rust, et un banc ne doit pas non plus en introduire dans le dépôt. Mesurer DuckDB-wasm aurait ajouté un paquet d’environ 149 Mo, en version de développement, à ce que surveille `pnpm audit`. Le protocole a été amendé en conséquence :

- La version **E, DuckDB-wasm**, est retirée.
- Les hypothèses qui comparaient Peras à elle sont retirées : **H4** enregistrée (le premier graphique avant la base, à froid) et **H5** (aussi rapide que la base, à chaud), et la première borne de **H6** enregistrée (un centième de ses octets) ; la seconde borne de H6 est gardée, comme **H4**.
- Les autres hypothèses sont inchangées et renumérotées : H7, H8 et H9 enregistrées deviennent **H5**, **H6** et **H7**. Aucun seuil n’a changé.
- Le temps jusqu’au premier graphique, à froid, reste une mesure, sans hypothèse.

Aucune mesure n’ayant été prise, l’amendement ne peut pas avoir été guidé par un résultat.

## Journal de l’étude

### Décidé après l’enregistrement, avant la première mesure

Écrit avec les versions et le script, dans le commit `b06da25`, avant le début de l’exécution 1 :

- **Une ligne vide est une ligne cassée.** Le protocole dit à la fois « toute autre ligne non vide après l’en-tête est cassée » et range « une ligne vide » parmi les lignes cassées ; la liste est suivie. Le `\n` qui termine la dernière ligne n’en commence pas une.
- **Ce que reçoit C.** Le `split` et le `parseI32` de Peras travaillent sur une `String` ; un tableau d’octets ne peut pas être lu comme du texte. C reçoit le texte : le temps de C comprend le décodage des octets et la copie du texte dans le module.
- **D ne peut pas être écrite.** `perasc` refuse `bench/data/dashboard-parallel.peras` : `error[E0241]: this loop cannot run in parallel: it makes, grows, moves or releases a string, an array or a map`. Le fichier est gardé tel qu’écrit ; selon le protocole, H5 est réfutée.
- **Lignes de code.** Les types du résumé sont déclarés dans `summary.ts` pour A et B, qui n’est pas compté ; C les déclare dans son propre fichier, en `export struct`. Ils sont exclus du compte de C de la même façon.
- **Une collecte avant chaque exécution chronométrée**, `gc()` avec `--expose-gc`, pour qu’une version ne paie pas les déchets de la précédente.
- **Mémoire.** Le plus grand `VmHWM` parmi les processus de rendu d’un navigateur lancé pour l’exécution, la page étant la seule ouverte.
- **Le réseau bridé**, exploratoire : 9 Mb/s en réception, 1,5 en émission, 170 ms de latence, à 100 000 lignes seulement, 3 chargements : à cette vitesse, 140 Mo prendraient deux minutes par chargement.
- **Deux bogues du compilateur** ont été trouvés en écrivant C, tous deux refusés par son vérificateur de libération plutôt que livrés, et corrigés avec un test chacun avant la mesure (commit `e1f58b4` ; [constats](../findings.md), F001 et F002).

### Exécution 1, au commit `b06da25` : les résultats

Mesuré le 2026-11-11 de 06:54 à 07:02 UTC, sur un Intel Xeon à 2,10 GHz à 4 cœurs, Chromium sans écran ; le rapport complet est dans [bench/data/run-1/RESULTS.md](../../../bench/data/run-1/RESULTS.md) et chaque nombre dans son `results.json`. Les trois versions donnent le même résumé, octet pour octet, à chaque taille.

| Hypothèse                                        | Verdict     | Mesuré                                                                |
| ------------------------------------------------ | ----------- | --------------------------------------------------------------------- |
| H1, Peras simple bat TypeScript simple           | **réfutée** | C à 0,85 fois la vitesse de A à 1 000 000 lignes, 0,88 à 4 000 000    |
| H2, Peras simple approche le TypeScript optimisé | **réfutée** | C à 0,10 fois la vitesse de B à 1 000 000 lignes, 0,09 à 4 000 000    |
| H3, moins de mémoire                             | **réfutée** | le maximum de C vaut 1,12 fois celui de A à 4 000 000 lignes          |
| H4, un petit programme                           | **réfutée** | C livre 12 169 octets, 15,4 fois les 788 de A                         |
| H5, les cœurs aident                             | **réfutée** | D ne peut pas être écrite (E0241)                                     |
| H6, linéaire                                     | tient       | le temps de C à 4 000 000 lignes vaut 4,17 fois son temps à 1 000 000 |
| H7, pas plus de code                             | tient       | C compte 99 lignes, 1,08 fois les 92 de A                             |

À chaud, en millisecondes, médianes de 15 exécutions :

| Version                    | 100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | ------: | --------: | --------: |
| A. TypeScript, idiomatique |    75,7 |   1 015,4 |   4 360,4 |
| B. TypeScript, optimisé    |    12,8 |     121,5 |     469,1 |
| C. Peras                   |   105,6 |   1 193,0 |   4 976,5 |

Les exécutions ne se chevauchent pas là où cela compte : à 4 000 000 lignes, les quinze temps de C vont de 4 776 à 5 507 ms, tous au-dessus de la médiane de A. À froid, le premier graphique arrive dans le même ordre, 5,5 s pour C, 4,8 pour A et 0,9 pour B à 4 000 000 lignes ; sous le réseau bridé, à 100 000 lignes, le téléchargement en prend presque tout, 3,7 à 3,8 s pour chaque version.

### Analyse

- **Où C perd son temps.** Un profil fait en écrivant C, à 1 000 000 lignes dans Node, hors protocole : passer le texte au module coûte 7 ms, le découper en lignes 20 ms, en champs 126 ms, contre 586 ms pour le `split` de JavaScript ; lire deux entiers par ligne avec `parseI32` le porte à 434 ms, environ 150 ns par entier, et les tables à clés textuelles construites à chaque ligne à 793 ms. Peras découpe le texte plus vite que JavaScript ; il perd sur la lecture des entiers et sur les tables à clés textuelles ([constats](../findings.md) F201, F202).
- **La marge existe.** B montre ce que la machine permet pour le même travail : 8,4 à 9,3 fois la vitesse de A, et 9,8 à 10,6 fois celle de C, à 1 000 000 et 4 000 000 lignes. Rien dans les données ni dans la requête n’empêche un langage compilé de l’atteindre ; ce qui en empêche Peras, c’est la lecture d’entiers de son runtime, ses tables à clés textuelles, et la copie du texte dans le module.
- **La mémoire.** C tient le texte une seconde fois, copié dans sa mémoire linéaire, que WebAssembly ne rend pas ; à 4 000 000 lignes, cette copie seule fait 122 Mio. C’est pourquoi le maximum de C dépasse celui de A, où le texte est partagé avec la chaîne décodée.
- **Le programme.** C livre son runtime, les tables, les chaînes, le tri : 12 169 octets, contre 788 pour A, qui utilise ceux de JavaScript. À 100 000 lignes sous un réseau bridé, l’écart se perd dans le téléchargement des données, 3,7 à 3,8 s pour chaque version.
- **Ce qui tient.** Le coût de C croît linéairement avec les lignes, et il s’écrit en autant de lignes que le TypeScript, 99 contre 92, avec la même structure : Peras simple n’est pas plus difficile à écrire pour ce travail, il est plus lent.

### Ce que cela veut dire

Sur l’ingestion de données textuelles, l’affirmation que cette étude testait, Peras ne tient pas aujourd’hui : Peras simple est plus lent que TypeScript simple, tient plus de mémoire, livre quinze fois plus de code et ne peut pas utiliser les cœurs. L’étude le dit, et dit pourquoi, en des termes sur lesquels le compilateur peut agir. Dans l’ordre que donnent les mesures, comme la décision 0011 ordonne ses étapes :

1. **Lire les entiers vite** : `parseI32` à la vitesse d’une boucle sur des octets (F201).
2. **Des tables à clés textuelles sans une chaîne par ligne** : une clé cherchée depuis la tranche d’où elle vient, copiée seulement quand elle est nouvelle (F202).
3. **Pas de copie du texte** : lire les octets là où ils sont, ce qui demande `u8` compilé (F302), la prochaine étape déjà décidée.
4. **Agréger sur les cœurs** : `parallel for` avec une réduction par thread et l’allocation dans un tour (F301).
5. **Un runtime plus léger** pour ce qu’un module n’utilise pas (F203).

Chacune sera mesurée de nouveau sous ce protocole, inchangé, comme exécution 2 et suivantes, et rapportée quoi qu’elle donne.

### Limites

- **La mémoire sous le pic du chargement n’est pas vue.** `VmHWM` ne monte qu’au-delà de son point le plus haut ; charger le fichier, comme réponse puis tampon, atteint déjà un pic. La requête de B reste en dessous à 1 000 000 et 4 000 000 lignes, et sa mémoire se lit 0 : pas rien, mais moins que le chargement des données. A et C le dépassent tous deux, ce qui garde H3 lisible.
- Les autres limites sont celles enregistrées : données générées sans guillemets, versions écrites par les auteurs, une machine.

### Exécution 2, au commit `6ef6283` : les résultats

Mesuré le 2026-11-11 de 11:51 à 11:59 UTC, sur la même machine, sous le même protocole, inchangé ; le rapport est dans [bench/data/run-2/RESULTS.md](../../../bench/data/run-2/RESULTS.md), celui de l’exécution 1 est gardé dans [bench/data/run-1](../../../bench/data/run-1/RESULTS.md). Les trois versions donnent le même résumé, octet pour octet, à chaque taille.

**Ce qui a changé dans le compilateur depuis l’exécution 1**, chaque fois avec ses tests et consigné dans [les constats](../findings.md) : un `split` en `const` seulement lu garde des vues au lieu de copies, et `parseI32` lit ses chiffres une fois avec une seule division (F201, commit `19930d1`) ; une chaîne dont la casse change seulement pour être comparée est comparée octet par octet, sans être créée (F205, commit `51635c5`). La source de la version C n’a pas changé.

| Hypothèse                                           | Exécution 1         | Exécution 2                                       | Verdict     |
| --------------------------------------------------- | ------------------- | ------------------------------------------------- | ----------- |
| H1, C au moins 2 fois A                             | 0,85 et 0,88        | **1,26** à 1 000 000 lignes, **1,23** à 4 000 000 | **réfutée** |
| H2, C au moins 0,8 fois B                           | 0,10 et 0,09        | 0,13 et 0,12                                      | **réfutée** |
| H3, la mémoire de C au plus la moitié de celle de A | 1,12                | 1,12                                              | **réfutée** |
| H4, C au plus 3 fois les octets de A                | 15,4                | 15,1, 11 928 octets                               | **réfutée** |
| H5, D au moins 2 fois C                             | impossible à écrire | impossible à écrire                               | **réfutée** |
| H6, linéaire                                        | 4,17                | 3,80                                              | tient       |
| H7, C au plus 1,25 fois les lignes de A             | 1,08                | 1,08                                              | tient       |

À chaud, en millisecondes, médianes de 15 exécutions :

| Version                    | 100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | ------: | --------: | --------: |
| A. TypeScript, idiomatique |   115,7 |   1 264,1 |   4 674,9 |
| B. TypeScript, optimisé    |    14,7 |     135,0 |     475,5 |
| C. Peras                   |    83,9 |   1 002,2 |   3 810,0 |

**Peras simple est maintenant plus rapide que TypeScript simple**, là où l’exécution 1 le trouvait plus lent : 1,23 à 1,26 fois sa vitesse, en deçà des 2 enregistrés. À froid, son premier graphique arrive maintenant avant celui de A à 1 000 000 et 4 000 000 lignes (1,2 s contre 1,5 ; 4,6 contre 5,3). Les exécutions sont plus dispersées qu’à l’exécution 1 et se chevauchent à leurs extrémités (à 4 000 000 lignes, C de 3 607 à 4 672 ms, A de 4 259 à 5 597) : ce sont les médianes, pas des exécutions isolées, qui portent la comparaison. Les temps de A eux-mêmes sont 7 à 25 % plus lents qu’à l’exécution 1 sur la même machine : ce qui est comparé, ce sont les versions d’une même exécution entre elles, en alternance, comme le protocole l’a enregistré.

**Ce qui n’a pas changé.** Le TypeScript optimisé reste 8 fois plus rapide que Peras simple ; Peras tient toujours le texte une seconde fois en mémoire (H3), livre son runtime (H4), et ne peut pas agréger sur plusieurs threads (H5). L’ordre du travail est celui qu’a donné l’exécution 1, F201 et F205 faits : les tables à clés textuelles (F202), pas de copie du texte (F204, qui demande `u8` compilé), l’agrégation sur les cœurs (F301), un runtime plus léger (F203).

**Écarts au protocole.**

- **Le commit du rapport est faux.** Le rapport nomme `93d4d25`, le commit à sa fin : le script le lisait alors, et un commit a été fait pendant l’exécution. Le compilateur mesuré a été construit au commit `6ef6283` ; `93d4d25` change trois fichiers de documentation et aucun code. Le script lit désormais le commit à son démarrage.
- **Le processeur a été utilisé pendant l’exécution**, contre la règle de cette étude : un commit à 11:55 UTC, avec son formatage et sa vérification des traductions, quelques secondes, pendant les chargements à froid ou les mesures de mémoire. La mémoire, un pic par processus, n’est pas affectée par un autre processus ; un chargement à froid a pu être ralenti, et les temps à froid ne sont testés contre aucune hypothèse.

### Exécution 3, au commit `df51da3` : les résultats

Mesurée le 2026-11-12 de 02:17 à 02:22 UTC, Chromium sans écran, sous le même protocole, inchangé ; rien d’autre n’a tourné. Le rapport est dans [bench/data/run-3/RESULTS.md](../../../bench/data/run-3/RESULTS.md) ; celui de l’exécution 2 est gardé dans [bench/data/run-2](../../../bench/data/run-2/RESULTS.md). Les trois versions donnent le même résumé, octet pour octet, à chaque taille.

**Ce qui a changé dans le compilateur depuis l’exécution 2.** Toute la [décision 0016](0016-faster-from-the-literature.md) :

- les littéraux de texte prêtés depuis un en-tête statique ;
- le runtime lié dans le module ;
- un repli calme de `unwrapOr` construit seulement s’il sert ;
- un cache des petits blocs libérés ;
- une recherche dans la table au lieu de deux ;
- le séparateur d’un split cherché seize octets à la fois ;
- les parties d’un split parcourues une à une.

Le source de la version C n’a pas changé, ni A, ni B, ni les données.

| Hypothèse                                  | Exéc. 1             | Exéc. 2             | Exéc. 3                                              | Verdict     |
| ------------------------------------------ | ------------------- | ------------------- | ---------------------------------------------------- | ----------- |
| H1, C au moins 2 fois A                    | 0,85 et 0,88        | 1,26 et 1,23        | **2,36** à 1 000 000 de lignes, **2,06** à 4 000 000 | **tient**   |
| H2, C au moins 0,8 fois B                  | 0,10 et 0,09        | 0,13 et 0,12        | 0,28 et 0,25                                         | **réfutée** |
| H3, la mémoire de C au plus la moitié de A | 1,12                | 1,12                | 0,78                                                 | **réfutée** |
| H4, C au plus 3 fois les octets de A       | 15,4                | 15,1                | 12,5, 9 869 octets                                   | **réfutée** |
| H5, D au moins 2 fois C                    | ne peut être écrite | ne peut être écrite | ne peut être écrite                                  | **réfutée** |
| H6, linéaire, au plus 4,4                  | 4,17                | 3,80                | 4,42                                                 | **réfutée** |
| H7, C au plus 1,25 fois les lignes de A    | 1,08                | 1,08                | 1,08                                                 | tient       |

À chaud, en millisecondes, médianes de 15 passes :

| Version                    | 100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | ------: | --------: | --------: |
| A. TypeScript, idiomatique |    69,5 |     790,0 |   3 038,4 |
| B. TypeScript, optimisé    |    10,5 |      93,3 |     370,2 |
| C. Peras                   |    30,5 |     334,1 |   1 477,7 |

**H1 tient pour la première fois.** Peras écrit simplement calcule le tableau de bord 2,06 à 2,36 fois plus vite que TypeScript écrit simplement, au-delà des 2 enregistrés. À froid, son premier graphique arrive en 0,54 s contre 1,04 à 1 000 000 de lignes, et en 2,06 s contre 3,79 à 4 000 000. Son pic de mémoire est passé de 1,12 fois celui de A à 0,78, encore au-dessus de la moitié enregistrée. Il livre 9 869 octets au lieu de 11 928.

**Ce qui n’a pas changé.** Le TypeScript optimisé reste 3,6 à 4 fois plus rapide que Peras écrit simplement, contre 8 à l’exécution 2. Peras tient toujours le texte une seconde fois, livre son runtime, et ne peut pas agréger sur plusieurs threads.

**H6 est réfutée de 0,02.** Le temps de C croît de 4,42 fois de 1 000 000 à 4 000 000 de lignes, pour au plus 4,4. La règle enregistrée s’applique telle quelle : un seuil manqué de peu est manqué. Les exécutions 1 et 2 donnaient 4,17 et 3,80, donc la croissance est proche de la borne à chaque exécution. Rien ne montre qu’elle se soit dégradée.

**Écart au protocole : la machine a changé.** Les exécutions 1 et 2 ont tourné sur un Intel Xeon à 2,10 GHz, l’exécution 3 sur un Intel Xeon à 2,30 GHz, tous deux à 4 cœurs. Le cloud donne un nouveau conteneur à une nouvelle session, et son processeur n’est pas choisi. Les temps de A eux-mêmes sont 31 à 38 % plus bas qu’à l’exécution 2. Donc les millisecondes d’une version ne se comparent pas d’une exécution à l’autre. Ce que compare le protocole, ce sont les versions d’une même exécution entre elles, en alternance, sur une même machine : les rapports. Ceux-ci tiennent (constat M016).

### Exécution 4, au commit `d9fbe35` : les résultats

Mesurée le 2026-11-12 de 07:45 à 07:49 UTC, Chromium sans écran, sous le même protocole, inchangé ; rien d’autre n’a tourné. La machine est du même modèle que celle de l’exécution 3, un Intel Xeon à 2,30 GHz à 4 cœurs. Le rapport est dans [bench/data/run-4/RESULTS.md](../../../bench/data/run-4/RESULTS.md) ; celui de l’exécution 3 est gardé dans [bench/data/run-3](../../../bench/data/run-3/RESULTS.md). Les trois versions donnent le même résumé, octet pour octet, à chaque taille.

**Ce qui a changé dans le compilateur depuis l’exécution 3.** Les étapes 1 à 3 de la [décision 0017](0017-aggregating-without-copies.md) :

- un repli qui copie n’est construit que s’il sert ;
- une entrée de table sortie, changée et remise n’est cherchée qu’une fois ;
- les entiers d’un gabarit sont écrits dans sa chaîne.

Le source de la version C n’a pas changé, ni A, ni B, ni les données.

| Hypothèse                                  | Exéc. 2             | Exéc. 3             | Exéc. 4                                              | Verdict     |
| ------------------------------------------ | ------------------- | ------------------- | ---------------------------------------------------- | ----------- |
| H1, C au moins 2 fois A                    | 1,26 et 1,23        | 2,36 et 2,06        | **2,63** à 1 000 000 de lignes, **2,39** à 4 000 000 | **tient**   |
| H2, C au moins 0,8 fois B                  | 0,13 et 0,12        | 0,28 et 0,25        | 0,31 et 0,26                                         | **réfutée** |
| H3, la mémoire de C au plus la moitié de A | 1,12                | 0,78                | 0,78                                                 | **réfutée** |
| H4, C au plus 3 fois les octets de A       | 15,1                | 12,5                | 12,5, 9 853 octets                                   | **réfutée** |
| H5, D au moins 2 fois C                    | ne peut être écrite | ne peut être écrite | ne peut être écrite                                  | **réfutée** |
| H6, linéaire, au plus 4,4                  | 3,80                | 4,42                | 4,88                                                 | **réfutée** |
| H7, C au plus 1,25 fois les lignes de A    | 1,08                | 1,08                | 1,08                                                 | tient       |

À chaud, en millisecondes, médianes de 15 passes :

| Version                    | 100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | ------: | --------: | --------: |
| A. TypeScript, idiomatique |    62,8 |     779,8 |   3 460,6 |
| B. TypeScript, optimisé    |    10,7 |      93,0 |     374,2 |
| C. Peras                   |    27,4 |     296,9 |   1 448,1 |

**Ce que cela dit.** Peras écrit simplement calcule le tableau de bord 2,39 à 2,63 fois plus vite que TypeScript simple, contre 2,06 à 2,36 à l’exécution 3. Face au TypeScript optimisé à la main, il passe de 0,25–0,28 à 0,26–0,31. Les exécutions 3 et 4 avaient le même processeur, donc les millisecondes de C se comparent aussi, avec prudence (M016). À 1 000 000 de lignes, C passe de 334,1 à 296,9 ms, 11 % de moins ; à 4 000 000 de lignes, de 1 477,7 à 1 448,1, 2 % de moins.

**Le gain est plus petit sur le plus grand fichier, et H6 s’éloigne de sa borne** : 3,80, 4,42, puis 4,88, pour au plus 4,4. La décision 0017 a retiré du travail fait à chaque ligne, ce qui se voit à 1 000 000 de lignes. Ce qui croît plus vite que les lignes ne dépend pas de ce travail. À 4 000 000 de lignes, les tables des paires, des produits et des utilisateurs sont plus grandes et moins souvent en cache (F215). C’est une lecture des chiffres, pas une mesure de sa cause. C’est la première chose à profiler à 4 000 000 de lignes avant la prochaine décision sur le tableau de bord.

### Exécution 5, au commit `8f98a87` : les résultats

Mesurée le 2026-11-12 de 09:43 à 09:47 UTC, sur le même modèle de machine que les exécutions 3 et 4, un Intel Xeon à 2,30 GHz à 4 cœurs, Chromium sans écran ; rien d’autre n’a tourné. Le rapport est dans [bench/data/RESULTS.md](../../../bench/data/RESULTS.md) ; celui de l’exécution 4 est gardé dans [bench/data/run-4](../../../bench/data/run-4/RESULTS.md). Les trois versions donnent le même résumé, octet pour octet, à chaque taille.

**Ce qui a changé depuis l’exécution 4.** La décision 0020 : une fonction exportée qui prend un `String` prend aussi un `Uint8Array`, copié tel quel et vérifié être de l’UTF-8. **Écart à l’exécution 4, enregistré dans la décision 0020 avant tout code :** le code d’appel de la version C appelle `peras.summarize(bytes)` au lieu de `peras.summarize(new TextDecoder().decode(bytes))`, comme A et B prennent déjà les octets. Le source Peras de C, A, B, les données et les seuils n’ont pas changé.

| Hypothèse                                  | Exéc. 3             | Exéc. 4             | Exéc. 5                                              | Verdict     |
| ------------------------------------------ | ------------------- | ------------------- | ---------------------------------------------------- | ----------- |
| H1, C au moins 2 fois A                    | 2,36 et 2,06        | 2,63 et 2,39        | **3,00** à 1 000 000 de lignes, **2,52** à 4 000 000 | **tient**   |
| H2, C au moins 0,8 fois B                  | 0,28 et 0,25        | 0,31 et 0,26        | 0,35 et 0,28                                         | **réfutée** |
| H3, la mémoire de C au plus la moitié de A | 0,78                | 0,78                | **0,23**                                             | **tient**   |
| H4, C au plus 3 fois les octets de A       | 12,5                | 12,5                | 12,9, 10 147 octets                                  | **réfutée** |
| H5, D au moins 2 fois C                    | ne peut être écrite | ne peut être écrite | ne peut être écrite                                  | **réfutée** |
| H6, linéaire, au plus 4,4                  | 4,42                | 4,88                | 4,91                                                 | **réfutée** |
| H7, C au plus 1,25 fois les lignes de A    | 1,08                | 1,08                | 1,08                                                 | tient       |

À chaud, en millisecondes, médianes de 15 passes :

| Version                    | 100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | ------: | --------: | --------: |
| A. TypeScript, idiomatique |    62,7 |     810,9 |   3 350,4 |
| B. TypeScript, optimisé    |    10,6 |      94,1 |     373,3 |
| C. Peras                   |    24,6 |     270,7 |   1 327,8 |

Mémoire, le pic ajouté par la requête, médianes de 5 :

| Version                    |  100 000 | 1 000 000 | 4 000 000 |
| -------------------------- | -------: | --------: | --------: |
| A. TypeScript, idiomatique | 17,5 Mio |  84,9 Mio | 219,3 Mio |
| B. TypeScript, optimisé    |  4,8 Mio |   0,0 Mio |   0,0 Mio |
| C. Peras                   |  6,2 Mio |  24,1 Mio |  49,4 Mio |

**Ce que cela dit.**

- **H3 tient pour la première fois.** Le pic de Peras à 4 000 000 de lignes passe de 171,6 Mio à l’exécution 4 à 49,4 Mio : 0,23 fois celui de TypeScript simple, quand 0,5 était enregistré. La chaîne que `TextDecoder` construisait, et que le chargeur réencodait dans le module, a disparu.
- **Peras écrit simplement est maintenant 2,52 à 3,00 fois plus rapide que TypeScript simple**, contre 2,39 à 2,63, puisque décoder le texte prenait aussi du temps.
- **À froid, son premier graphique** arrive en 0,45 s contre 1,01 à 1 000 000 de lignes, et en 1,74 s contre 3,85 à 4 000 000.
- **Il livre 10 147 octets,** 294 de plus qu’à l’exécution 4 : la copie des octets par le chargeur et la vérification du runtime.

**Ce que la mesure de la mémoire ne voit pas.** Le protocole mesure le pic au-delà du pic de la page avec les données chargées (`VmHWM`, constat M008). La requête de la version B reste sous ce pic et lit 0 ; une part de celle de C peut l’être aussi. Le module tient toujours une copie du texte, environ 122 Mio à 4 000 000 de lignes, que la décision 0020 dit ne pas pouvoir retirer tant que les données partent de l’extérieur du module. Les 49,4 Mio sont donc le pic que ce protocole voit, pas tout ce que C tient. La comparaison avec A, que H3 a enregistrée, tient : la requête de A monte au-delà de ce pic, et la chaîne de C ne le fait plus.
