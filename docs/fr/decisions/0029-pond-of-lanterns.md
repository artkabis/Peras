# 0029 : Le bassin aux mille lanternes, une scène en 3D mesurée

_Version anglaise : [../../en/decisions/0029-pond-of-lanterns.md](../../en/decisions/0029-pond-of-lanterns.md)._

_État : protocole enregistré le 2026-11-17, avant toute version autre que la référence et avant toute mesure ; choisi par le propriétaire, avant `Rc<T>` (constat F310), pour que ses options soient mesurées sur cette scène. Les résultats seront ajoutés sous lui, qu’ils confirment les hypothèses ou non ; le protocole ne changera pas après la première mesure, et tout écart sera listé comme tel._

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

| Quand (2026-11-17)     | Commit | Étape                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| avant l’enregistrement | —      | La référence écrite en TypeScript, puis dessinée aux images 0, 300 et 600 dans Node pour voir que la spécification dessine et qu’elle est déterministe. Vu sur les premiers rendus, avant toute autre version : les lanternes faisaient un mur à l’horizon, elles sont donc réparties plus large et plus loin et l’œil est monté de 1,2 à 3 ; leur halo était faible, il porte donc plus loin ; la lune, son chemin sur l’eau et les lanternes célestes ont été ajoutés. Le script a affiché le temps de la référence dans Node, 45 à 48 ms par image après une d’échauffement : une mesure informelle de la référence seule, qui ne porte sur aucun rapport des hypothèses, dont les seuils sont ceux de la décision 0013. |
