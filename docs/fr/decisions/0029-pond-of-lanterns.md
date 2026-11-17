# 0029 : Le bassin aux mille lanternes, une scène en 3D mesurée

_Version anglaise : [../../en/decisions/0029-pond-of-lanterns.md](../../en/decisions/0029-pond-of-lanterns.md)._

_État : protocole enregistré le 2026-11-17 (commit `724bd40`), avant toute version autre que la référence et avant toute mesure ; étude terminée avec sa seconde exécution, au commit `43ad222`. Quatre hypothèses tiennent (H2, H5, H6, H7), deux sont réfutées (H1, la vitesse face à TypeScript, et H3, le module managed), une est indéterminée (H4, les threads). Résultats et registre ci-dessous. Le protocole a été choisi par le propriétaire, avant `Rc<T>` (constat F310), pour que ses options soient mesurées sur cette scène._

## Contexte

Le propriétaire a demandé si Peras produit des éléments 3D dans un canvas plus vite que JavaScript, TypeScript et WebAssembly, avec un banc qui soit aussi un poème. La [décision 0013](0013-garden-of-apeiron.md) a mesuré une scène calculée pixel par pixel ; celle-ci mesure ce que fait un moteur 3D : des maillages tournés, projetés et remplis avec un tampon de profondeur, mille fois par image.

Ce qu’elle peut dire et ne peut pas dire, avant de mesurer :

- **Elle mesure le processeur, pas la carte graphique.** Une version WebGL dessinerait cette scène plus vite que toutes les versions ici ; elle est hors du protocole, Peras n’ayant pas encore de type pour WebGL. Ce qui est comparé, c’est une image calculée par le processeur, comme la calcule un moteur logiciel, un pas de physique ou un worker qui prépare une géométrie.
- **Peras écrit du WebAssembly.** Face à Rust compilé en WebAssembly, le mieux qu’il puisse atteindre est la même vitesse ; la question est à quelle distance, et à quel coût d’écriture.
- **Peras est mesuré avant sa phase graphique.** `Framebuffer`, les types de pixels et les nombres aléatoires à graine sont prévus (phases 3 et 6 de la feuille de route) ; la scène s’écrit avec ce qui est compilé aujourd’hui, et ce qui lui manque est consigné dans les constats.
- **Un maillage est partagé par mille lanternes.** Sans `Rc<T>`, le maillage est gardé une fois et chaque lanterne le lit, comme le fait le code orienté données dans tout langage ; ce que coûterait à écrire une lanterne tenant sa propre référence au maillage est noté, pour que la décision 0030 y mesure les options de `Rc<T>`.

## La scène

La nuit, un bassin. Mille lanternes de papier, chacune le même maillage de trois anneaux de huit points, 44 triangles, dérivent sur l’eau ; un quart sont des lanternes célestes, qui montent lentement et repartent de l’eau. La lune regarde et l’eau garde sa trace. Une image, numérotée _t_, se dessine ainsi, dans cet ordre, par chaque version :

1. **Le ciel et l’eau**, chaque pixel : un dégradé ; des étoiles là où un hash du pixel tombe sous 0,0015, scintillant avec _t_ ; la lune, un disque au bord doux ; sous l’horizon, l’eau plus sombre de près, et le chemin de la lune sur elle, brisé par les rides.
2. **Chaque lanterne**, dans l’ordre de son numéro : sa place, sa rotation, sa taille, sa couleur et sa flamme tirées d’un hash de son numéro et déplacées avec _t_. Son maillage est tourné, placé et projeté ; chaque triangle est rempli là où le centre d’un pixel est à l’intérieur et plus proche que ce que tient le tampon de profondeur, d’une teinte plate, le papier éclairé de l’intérieur, plus vif là où il fait face à l’œil.
3. **Son reflet** : les mêmes triangles en miroir dans l’eau, chaque point déplacé de côté par une ride de sa hauteur à l’écran, plus sombres.
4. **Son halo, puis le reflet du halo** : chaque pixel proche de la flamme ajoute la couleur de la lanterne, qui s’efface avec le carré de la distance.
5. **Les couleurs**, chacune bornée et arrondie à un octet, rangées en un mot RGBA par pixel.

**Les mêmes pixels partout.** La scène n’utilise que des opérations qu’IEEE 754 définit exactement, `+`, `-`, `*`, `/`, `sqrt`, `floor`, `abs`, `min`, `max`, sur des `f64`, et un hash entier de 32 bits (`lowbias32`, [Wellons, 2018](https://nullprogram.com/blog/2018/07/31/)), comme la décision 0013. Une lanterne tourne sans `sin` ni `cos`, par la paramétrisation rationnelle du cercle ; elle se balance et vacille par une onde triangulaire. Les additions se font dans le même ordre dans chaque version, pour que leurs arrondis soient les mêmes.

**La spécification est la référence.** `bench/lanterns/pond.ts`, au commit qui enregistre ce protocole, dit chaque nombre et chaque ordre ; chaque autre version la traduit opération par opération.

## Les versions

- **TypeScript, écrit pour la vitesse** : tableaux typés, boucles simples ; la référence.
- **Peras, owned**, compilé en WebAssembly, écrit aussi simplement que le TypeScript, construit pour la production (TLSF) et pour le débogage.
- **Peras, owned, sur des threads** : le même, ses lignes réparties par `parallel for` avec `--threads on`, si le langage permet de l’écrire (constat F301 : ni appel, ni allocation dans un tour).
- **Peras, managed** : la même source dans un module `.apeiron`, compilée en JavaScript.
- **Rust**, compilé en WebAssembly avec `wasm-bindgen`, comme le construit `bench/garden/rust`, sa chaîne d’outils épinglée.

Chaque version reçoit ses tampons de l’appelant, les pixels, la profondeur et la lumière, alloués une fois, comme un moteur les garde d’une image à l’autre. Chaque version est écrite par les auteurs de Peras ; c’est une menace à la validité, listée plus bas. Chaque concurrent respecte les règles du projet : Rust est le seul autre langage, aucun paquet npm n’est installé pour une version (constat M004).

## Hypothèses

Chacune avec ce qui la réfute. Les temps sont ceux de l’image 600 en 1024 × 768 avec 1 000 lanternes, dans Chromium.

- **H1, Peras est plus rapide que TypeScript.** Peras owned dessine une image au moins 1,3 fois plus vite que le TypeScript, la médiane des rapports tour par tour. _Réfutée_ sous 1,3. Le seuil est celui de la décision 0013, où le jardin a atteint 1,19.
- **H2, Peras est proche de Rust.** Au moins 0,85 fois la vitesse de Rust. _Réfutée_ en dessous.
- **H3, le managed ne coûte rien.** Le module managed dessine en au plus 1,15 fois le temps du TypeScript. _Réfutée_ au-dessus.
- **H4, les threads paient.** Sur des threads, au moins 2 fois plus vite que Peras owned sur un seul, sur les 4 cœurs de la machine. _Réfutée_ en dessous ; _indéterminée_ si `parallel for` ne peut pas exprimer l’image, avec le diagnostic pour preuve (constat M001).
- **H5, un petit module.** Peras owned, module et chargeur, minifiés et gzip, ne pèse pas plus d’octets que le module de Rust et sa colle `wasm-bindgen`. _Réfutée_ au-dessus.
- **H6, aussi facile à écrire que TypeScript.** Peras owned fait au plus 1,25 fois les lignes du TypeScript, commentaires et lignes vides mis à part. _Réfutée_ au-dessus. Les erreurs rencontrées avant qu’il se construise sont listées avec leurs codes, et chaque friction est consignée comme constat.
- **H7, pas de coût de la sûreté à l’image.** Le build de débogage, débordements d’entiers et indices vérifiés, en au plus 1,5 fois le temps de la production. _Réfutée_ au-dessus.

## Mesures

- **Mêmes pixels** : SHA-256 du rendu de chaque version aux images 0, 300 et 600, tous égaux.
- **Temps** : millisecondes par image, médiane de 15 exécutions après 2 d’échauffement, les versions alternant dans chaque exécution dans une seule page Chromium, et le rapport au TypeScript tour par tour (constat M017).
- **Charge du processeur et mémoire** : le temps CPU sur le temps réel des exécutions de chaque version, et la mémoire de chaque module, comme les mesure la décision 0012.
- **Octets** : ce que livre chaque version, minifié par Vite là où il s’applique, brut et gzip.
- **Code** : les lignes sans commentaires ni lignes vides ; les erreurs rencontrées à l’écriture ; les notions que TypeScript ne demande pas.
- **Le maillage partagé** : une lanterne qui tient son maillage, écrite comme un utilisateur venu de JavaScript l’écrit, construite par Peras ; ce qu’il refuse, noté comme point de départ de la décision 0030.

`node scripts/bench-lanterns.mjs` les lance ; `--check` vérifie les mêmes pixels en 256 × 192 avec 120 lanternes, aux images 0 et 600, et tourne avec chaque vérification du projet.

## Menaces à la validité

- Une machine, sans écran, 4 cœurs ; un téléphone peut ordonner les versions autrement.
- Les auteurs ont écrit chaque version, à partir du TypeScript ; un expert d’un autre langage peut l’écrire plus rapide.
- Des opérations exactes seulement : ni `sin`, ni `cos`, ni `pow`, qu’un vrai moteur utilise et dont les résultats diffèrent d’une bibliothèque à l’autre. La scène est façonnée par cette règle ; la vitesse de ces fonctions n’est pas mesurée.
- Les triangles d’une scène de cette taille sont petits : le coût par triangle, que font la projection et la mise en place, pèse plus que dans une scène de grands triangles.

## Conséquences

- `bench/lanterns`, `node scripts/bench-lanterns.mjs`, et ses résultats sous ce protocole, avec une image de la scène.
- La décision 0030, `Rc<T>`, mesure ses options sur cette scène.

## Registre de l’étude

| Quand (2026-11-17)     | Commit    | Étape                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------------- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| avant l’enregistrement | —         | La référence écrite en TypeScript, puis dessinée aux images 0, 300 et 600 dans Node pour voir que la spécification dessine et qu’elle est déterministe. Vu sur les premiers rendus, avant toute autre version : les lanternes faisaient un mur à l’horizon, elles sont donc réparties plus large et plus loin et l’œil est monté de 1,2 à 3 ; leur halo était faible, il porte donc plus loin ; la lune, son chemin sur l’eau et les lanternes célestes ont été ajoutés. Le script a affiché le temps de la référence dans Node, 45 à 48 ms par image après une d’échauffement : une mesure informelle de la référence seule, qui ne porte sur aucun rapport des hypothèses, dont les seuils sont ceux de la décision 0013. |
| après l’enregistrement | `afca8f0` | Peras owned et managed, et Rust, traduits de la référence ; chacun donne ses pixels au bit près aux images 0, 300 et 600, en 256 × 192 et en 1024 × 768, rien de vivant. La première version Peras est gardée telle qu’écrite, refusée avec 45 erreurs ; les threads et le maillage partagé sont gardés tels qu’écrits, refusés. Le script a affiché le temps de ces vérifications dans Node : informel, après l’enregistrement de chaque seuil.                                                                                                                                                                                                                                                                            |
|                        | `afca8f0` | L’exécution 1, gardée avec ses défauts dans [run-1](../../../bench/lanterns/run-1/RESULTS.md).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
|                        | `43ad222` | Les corrections de l’exécution 1 ; l’exécution 2, ci-dessous.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

### Exécution 1, au commit `afca8f0` : gardée, avec deux défauts

- **Rust a été construit sans le SIMD de WebAssembly**, là où le protocole le construit comme `bench/garden/rust`, avec `+simd128`, et comme Peras le fait par défaut. H2 a été mesurée contre un Rust affaibli : 0,82, réfutée.
- **Le tas JavaScript laisse de côté les octets des tableaux typés** : les 27 Mio de tampons dans lesquels dessinent TypeScript et le module managed n’étaient comptés nulle part, si bien que leur mémoire paraissait 36 fois plus petite que celle des modules WebAssembly.

**Les corrections**, au commit `43ad222`, sur la méthode seulement : Rust construit avec `+simd128` ; les octets de chaque version hors du tas comptés, sa mémoire WebAssembly ou les tableaux typés dans lesquels elle dessine.

### Exécution 2, au commit `43ad222` : les résultats

[Résultats de l’exécution 2](../../../bench/lanterns/RESULTS.md). Un Intel Xeon à 2,30 GHz, 4 cœurs, Node 24, Chromium sans écran ; image 600 en 1024 × 768 avec 1 000 lanternes, 15 exécutions après 2 d’échauffement, les versions alternant.

| Hypothèse                      | Verdict      | Mesuré                                                                                         |
| ------------------------------ | ------------ | ---------------------------------------------------------------------------------------------- |
| H1, plus rapide que TypeScript | **réfutée**  | 0,97 fois la vitesse de TypeScript (0,88 à 1,15) ; seuil 1,3                                   |
| H2, proche de Rust             | tient        | 0,89 fois la vitesse de Rust (0,80 à 0,99) ; seuil 0,85                                        |
| H3, le managed ne coûte rien   | **réfutée**  | 2,41 fois le temps de TypeScript (2,15 à 3,13) ; seuil 1,15                                    |
| H4, les threads paient         | indéterminée | `parallel for` ne peut pas exprimer l’image (E0241)                                            |
| H5, un petit module            | tient        | 8 164 octets gzip contre 16 491 pour Rust                                                      |
| H6, aussi facile à écrire      | tient        | 302 lignes, 1,10 fois les 275 de TypeScript ; 45 erreurs rencontrées avant qu’il se construise |
| H7, pas de coût de la sûreté   | tient        | le débogage à 1,08 fois le temps de la production (0,97 à 1,13)                                |

| Version                |    Image | CPU / réel | Mémoire, tas et hors du tas | Livré, gzip |
| ---------------------- | -------: | ---------: | --------------------------: | ----------: |
| TypeScript             |  46,0 ms |       1,00 |         763 Kio et 27,0 Mio |       1 866 |
| Peras, managed         | 106,2 ms |       1,00 |         753 Kio et 27,0 Mio |       2 492 |
| Peras, owned           |  45,0 ms |       1,00 |         711 Kio et 27,1 Mio |       8 164 |
| Peras, owned, débogage |  49,6 ms |       1,00 |         711 Kio et 27,1 Mio |           - |
| Rust                   |  41,1 ms |       1,00 |         695 Kio et 28,3 Mio |      16 491 |

### Ce que dit l’étude

- **Sur une image 3D calculée par le processeur, Peras est aussi rapide que TypeScript, pas plus** (H1). V8 compile cette boucle d’arithmétique flottante sur des tableaux typés à peu près aussi bien que WebAssembly l’exécute ; le jardin de la décision 0013, dont chaque pixel appelle bien des fois les fonctions de bruit, donnait 1,19 à Peras. Un gain sur JavaScript doit venir de ce que JavaScript ne fait pas : le SIMD sur les pixels, ou les threads, que H4 n’a pas pu atteindre.
- **Peras est à 0,89 de Rust** (H2), en deux fois moins d’octets (H5), et sa sûreté coûte 8 % (H7).
- **Le module managed est 2,4 fois plus lent que le TypeScript** (H3, constat F221) : la même source compilée en JavaScript ne donne pas le JavaScript qu’une personne écrit. Pas examiné ici.
- **Peras est aussi long que TypeScript, mais pas aussi facile à écrire** : 1,10 fois ses lignes une fois construit (H6 tient), après 45 erreurs, chacune un littéral entier là où un `f64` est attendu ou un tableau typé par son premier élément (constat F112), le plus qu’un programme ait rencontré.
- **Ce qui manque à la scène, consigné comme constats** : `parallel for` ne peut pas la dessiner (F301), et un maillage partagé par mille lanternes ne peut pas être tenu par chacune (F310, décision 0030).

### Écarts au protocole

- **H4 n’a pas de mesure** : sa version ne peut pas s’écrire, comme le protocole le prévoyait ; `bench/lanterns/threads.peras` garde l’essai et son diagnostic.
- **Le temps CPU sur le temps réel** est mesuré sur chaque version seule dans une page neuve, 5 images après 2, les versions n’alternant pas ; les temps de H1 à H3 et H7 viennent des exécutions alternées.
- **La mémoire hors du tas** a été ajoutée après l’exécution 1, la mesure du protocole, le tas seul, laissant de côté les tampons des versions JavaScript.
