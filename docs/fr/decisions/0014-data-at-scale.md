# 0014 : La donnée à l’échelle, agrégée dans le navigateur

_Version anglaise : [../../en/decisions/0014-data-at-scale.md](../../en/decisions/0014-data-at-scale.md)._

_Statut : protocole enregistré le 2026-11-11 (commit `96e6c43`), amendé le même jour avant toute mesure : DuckDB-wasm retiré (voir [Amendement](#amendement-avant-toute-mesure)) ; aucune version encore écrite._

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
