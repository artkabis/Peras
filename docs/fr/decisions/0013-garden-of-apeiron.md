# 0013 : Le jardin d’Apeiron, une scène procédurale mesurée

_Version anglaise, qui fait foi : [../../en/decisions/0013-garden-of-apeiron.md](../../en/decisions/0013-garden-of-apeiron.md)._

_Statut : protocole enregistré le 2026-11-11 à 05:21 UTC (commit `eb861d0`) ; étude achevée avec sa seconde exécution, à 06:02 UTC au commit `f527ff2`. Quatre hypothèses tiennent, une est réfutée : H3, la vitesse. Résultats et registre plus bas._

_Statut du protocole : enregistré avant toute mesure. Les résultats seront ajoutés en dessous, qu’ils confirment les hypothèses ou non ; le protocole lui-même ne changera plus après la première mesure, et tout écart sera signalé comme tel._

## Contexte

La meilleure compression d’une image est le plus court programme qui la dessine, sa complexité de Kolmogorov ; la démoscène en a fait un art, avec des films de quelques kilo-octets. Une scène calculée pixel par pixel à partir de règles peut être dessinée à n’importe quelle résolution, et zoomée sans fin : comme une image vectorielle, elle s’étire, et contrairement à elle, elle montre plus de détails à mesure qu’on zoome, là où une image stockée ne fait que flouter.

Le propriétaire a demandé si Peras peut réunir compression, vitesse, une qualité au-delà de l’existant, et la poésie dans ses algorithmes, et que ce soit fait avec la rigueur de l’ingénierie et de la science, pas naïvement. La poésie est dans le nom du projet : _peras_, la limite, imposée à l’_apeiron_, l’illimité, d’où naît, pour les pythagoriciens, tout ce qui est bon. Le jardin d’Apeiron est un paysage illimité, né de quelques règles finies.

Les risques de naïveté sont connus, et le protocole répond à chacun :

- **Une démo choisie pour gagner.** Les hypothèses, leurs seuils et les concurrents sont fixés ici, avant de mesurer ; une hypothèse réfutée est rapportée comme réfutée.
- **Des versions qui ne font pas le même travail.** Chaque version doit donner les mêmes pixels, bit pour bit, vérifiés par une empreinte de chaque image.
- **Une qualité jugée à l’œil.** La qualité est mesurée par SSIM ([Wang et al., 2004](https://doi.org/10.1109/TIP.2003.819861)) et PSNR, calculés par FFmpeg, pas par nous.
- **Une tautologie.** Une image procédurale est, par définition, sa propre référence à chaque zoom ; ce qui est mesuré, c’est ce qu’une image stockée doit dépenser pour s’en approcher.

## La scène

Une fonction, `pixel(x, y, zoom)`, donne la couleur d’un point du plan, écrite une fois dans chaque langage à partir de la même spécification :

1. **Le ciel** : un dégradé du zénith à l’horizon, et des nuages, une somme fractale de bruit de valeur ([Perlin, 1985](https://doi.org/10.1145/325165.325247)), seuillée.
2. **Les montagnes** : trois chaînes, chacune une crête de bruit fractal à une dimension, plus pâle avec la distance, comme l’air les rend ([la perspective aérienne de Léonard de Vinci](https://fr.wikipedia.org/wiki/Perspective_atmosph%C3%A9rique)).
3. **La prairie** : des fleurs placées par un bruit cellulaire ([Worley, 1996](https://doi.org/10.1145/237170.237267)), une par cellule en un point tiré de l’empreinte de la cellule, chacune un disque d’une couleur tirée de la même façon.

**Des détails illimités.** Le nombre d’octaves de chaque bruit croît avec le logarithme du zoom : zoomé 2 fois, une octave de plus. Le coût d’un pixel ne croît qu’avec le logarithme du zoom.

**Les mêmes pixels partout.** Pour être calculée à l’identique en TypeScript, Peras et Rust, la scène n’utilise que des opérations qu’IEEE 754 définit exactement : `+`, `-`, `*`, `/`, `sqrt`, `floor`, sur des `f64` ; son hasard est une empreinte entière de 32 bits (`lowbias32`, [Wellons, 2018](https://nullprogram.com/blog/2018/07/31/)), calculée par des multiplications qui rebouclent et des décalages logiques. Ni `sin`, ni `exp`, ni `pow`, dont les résultats diffèrent d’une bibliothèque à l’autre. Une couleur devient un octet par `floor(c * 255 + 0.5)` après écrêtage.

## Les versions

- **TypeScript, écrit pour la vitesse** : tableaux typés, boucles simples, la référence.
- **Peras**, un module owned compilé en WebAssembly, écrit aussi simplement que le TypeScript.
- **Rust**, compilé en WebAssembly avec `wasm-bindgen`, comme le construit `bench/browser`.

Chaque version est écrite à partir de la spécification ci-dessus, aussi naturellement que son langage le permet, par les auteurs de Peras ; c’est une menace à la validité, listée plus bas.

## Hypothèses

Chacune avec ce qui la réfute. Les tailles sont celles du JavaScript et du WebAssembly livrés : minifiés par Vite là où cela s’applique, puis gzip.

- **H1, le programme est plus petit que son image.** Le jardin en Peras, module et chargeur, se livre en moins d’octets que le plus petit AVIF de son rendu 1024 × 1024 qui atteint un SSIM de 0,95 contre lui. _Réfutée_ si un AVIF aussi petit ou plus petit atteint 0,95.
- **H2, une image stockée doit grandir avec le zoom ; le programme non.** Pour atteindre un SSIM de 0,95 contre le rendu procédural d’une fenêtre zoomée _z_ fois, pour _z_ = 1, 4, 16 et 64, une image stockée de toute la scène demande environ _z_² fois les octets ; le programme demande les mêmes octets à chaque zoom. _Réfutée_ si l’AVIF nécessaire à _z_ = 64 fait moins de 16 fois celui nécessaire à _z_ = 1.
- **H3, Peras est rapide là où ça compte.** Peras rend une image 1024 × 1024 du jardin au moins 1,3 fois plus vite que le TypeScript, médiane de 15 exécutions dans Chromium, et à au moins 0,85 fois la vitesse de Rust. _Réfutée_ par l’un des rapports sous son seuil.
- **H4, l’œil pardonne de loin.** Vue de plus loin, une image réduite 4 fois comme l’œil l’intègre, le SSIM de chaque format avec perte contre la référence réduite est plus élevé qu’à pleine taille, aux mêmes octets. _Réfutée_ s’il est plus bas pour un format. Cela teste l’intuition du propriétaire qu’un détail perdu se voit moins une fois dézoomé.
- **H5, aucun coût de la sûreté à l’image.** Le jardin en Peras construit en debug, son débordement d’entier vérifié (décision 0011, étape 5), rend à moins de 1,5 fois son build de production. _Réfutée_ au-dessus.

## Mesures

- **Mêmes pixels** : SHA-256 du rendu RGBA 1024 × 1024 de chaque version, aux zooms 1, 4, 16 et 64, tous égaux.
- **Octets** : ce que livre chaque version, minifié et gzip ; PNG, WebP et AVIF du rendu, à des qualités de la plus basse à la plus haute, encodés par FFmpeg.
- **Qualité** : SSIM et PSNR entre une image stockée, décodée et agrandie par Lanczos à la fenêtre, et le rendu procédural de cette fenêtre, par les filtres `ssim` et `psnr` de FFmpeg.
- **Temps** : millisecondes par image 1024 × 1024, médiane de 15 exécutions après 2 de chauffe, dans Chromium ; le temps CPU et la mémoire de la décision 0012.
- **Code** : lignes sans commentaires ni lignes vides, par version.

## Menaces à la validité

- Une seule machine, sans écran, 4 cœurs ; un téléphone peut ordonner les versions autrement.
- Les auteurs ont écrit chaque version : un expert d’un langage peut l’écrire plus rapide.
- Un paysage de bruit convient au procédural ; une photographie n’a pas de programme court, et rien ici ne dit le contraire.
- SSIM est un modèle de la perception parmi d’autres ; Butteraugli et SSIMULACRA2, qui modélisent l’œil de plus près, ne sont pas installés sur la machine qui mesure, et sont laissés à une exécution ultérieure.

## Conséquences

- `bench/garden`, `node scripts/bench-garden.mjs`, et ses résultats sous ce protocole ; une vérification en CI que les versions donnent les mêmes pixels.
- Si les hypothèses tiennent, le jardin devient la seconde démo du README, zoomée du paysage entier jusqu’à une seule fleur. Sinon, les résultats le disent, et le README aussi.

## Registre de l’étude

Un cahier de laboratoire : chaque étape, datée, avec le commit qui la contient, ce qui a été décidé après l’enregistrement du protocole, et ce qui a été observé. Rien ici ne change les hypothèses ni leurs seuils.

### Étapes

| Quand (UTC, 2026-11-11) | Commit    | Étape                                                                                                                                                                                                                                                                                                                                                     |
| ----------------------- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 05:21                   | `eb861d0` | Le protocole est enregistré, avant tout code de la scène et toute mesure.                                                                                                                                                                                                                                                                                 |
| 05:23–05:50             | —         | La référence est écrite en TypeScript ; la scène est dessinée une première fois, aux zooms 1, 8 et 64, pour voir qu’elle se dessine. Le script a affiché le temps de ces rendus, 327 ms en 512 × 512 au zoom 1 dans Node : une mesure informelle de la référence seule, avant que les autres versions n’existent, après l’enregistrement de chaque seuil. |
| 05:50                   | —         | Les versions en Peras et en Rust, d’après la même spécification ; chacune donne les octets de la référence en 256 × 256 et aux zooms 1 à 64.                                                                                                                                                                                                              |
| 05:54                   | `0fb61c9` | Les versions et `scripts/bench-garden.mjs` sont commités, avec la première exécution conservée dans `bench/garden/run-1`, et son défaut.                                                                                                                                                                                                                  |
| 05:55                   | `f527ff2` | Le formatage de la référence, après la première exécution ; ses pixels inchangés. La seconde exécution démarre sur ce commit.                                                                                                                                                                                                                             |
| 06:02                   | `f527ff2` | La seconde exécution s’achève ; ses résultats plus bas.                                                                                                                                                                                                                                                                                                   |

### Décidé après l’enregistrement, avant la première mesure

Le protocole fixe la structure de la scène, pas chacun de ses nombres. Avant toute mesure, après avoir regardé les premiers rendus :

- **Les couleurs.** Un ciel d’aube plutôt que de midi, des nuages seuillés à 0,56 plutôt qu’à 0,5, des montagnes du violet au vert, et une prairie dont la lisière ondule selon un bruit au lieu d’une droite en _y_ = 0,74. Esthétique seulement : aucune hypothèse ne dépend d’une couleur.
- **Le zoom comme nombre.** `render` prend le zoom, et non son logarithme, pour qu’une vidéo puisse zoomer en continu ; aux zooms du protocole, des puissances de deux, le calcul est exact et le même dans chaque version.

### Paramètres que le protocole laissait ouverts

- Les rendus vérifiés pour les mêmes pixels sont centrés sur (0,37 ; 0,83), dans la prairie, là où sont les fleurs de chaque niveau.
- H1 compresse la scène entière au zoom 1, centrée sur (0,5 ; 0,5).
- H2 tire 8 tuiles par zoom d’un générateur de graine 13 ; au zoom 1 la seule tuile est la scène entière.
- L’AVIF est encodé par libaom via FFmpeg, `-still-picture 1 -cpu-used 6`, le CRF cherché par dichotomie de 0 à 63 ; le WebP par libwebp, `-compression_level 6`.
- SSIM et PSNR sont ceux de FFmpeg, sur les deux images converties en YUV 4:4:4, la valeur des trois plans (« All »).
- H4 compare le WebP aux qualités 20, 50 et 80 et l’AVIF aux CRF 50, 35 et 20, puis les mêmes fichiers réduits 4 fois par moyenne des surfaces.
- Les temps de H3 et H5 viennent d’une seule page Chromium qui charge chaque version, les versions alternant à chaque exécution.

### Exécution 1, au commit `0fb61c9` : conservée, avec un défaut

[Résultats de l’exécution 1](../../../bench/garden/run-1/RESULTS.md). **Le défaut.** Le plus petit AVIF de chaque tuile de H2 était cherché en 4:2:0 seulement, le sous-échantillonnage gagnant au zoom 1. Pour certaines tuiles zoomées, aux fleurs petites et saturées, aucun AVIF en 4:2:0 n’atteignait un SSIM de 0,95, même au CRF 0 ; la recherche ne rendait aucune taille, le script l’écrivait infinie et comptait H2 comme tenue. H2 était indéterminée par cette exécution.

**La correction**, au commit `0fb61c9`, sur la méthode seulement : l’AVIF de chaque tuile est cherché en 4:4:4 et en 4:2:0, le plus petit gardé, comme le dit le « plus petit AVIF » du protocole ; une tuile qu’aucun AVIF n’amène à 0,95 est comptée non atteinte, et un zoom avec une tuile non atteinte rend H2 indéterminée.

**Ce que l’exécution 1 a mesuré et que le défaut ne touche pas**, provisoire jusqu’à l’exécution 2 :

| Hypothèse    | Exécution 1  | Mesuré                                                                                                         |
| ------------ | ------------ | -------------------------------------------------------------------------------------------------------------- |
| Mêmes pixels | oui          | TypeScript, Peras, Peras en debug et Rust, aux zooms 1, 4, 16, 64 en 1024 × 1024                               |
| H1           | tient        | Peras livre 5 872 octets ; le plus petit AVIF à SSIM 0,95, 9 774 (4:2:0, CRF 53)                               |
| H2           | indéterminée | le défaut ci-dessus                                                                                            |
| H3           | **réfutée**  | Peras à 1,21 fois la vitesse de TypeScript, sous le 1,3 enregistré ; à 1,13 fois celle de Rust                 |
| H4           | tient        | pour chacun des six fichiers, le SSIM est plus haut réduit 4 fois, de 0,948 à 0,977 pour le WebP en qualité 20 |
| H5           | tient        | Peras en debug à 1,02 fois son temps de production                                                             |

Une observation que les hypothèses n’avaient pas prévue, et qui doit accompagner H1 : le même programme en TypeScript se livre en 1 119 octets, cinq fois moins que Peras ; c’est la scène procédurale qui est plus petite que son image, et Peras paie le poids de son runtime WebAssembly et de son chargeur.

### Exécution 2, au commit `f527ff2` : les résultats

[Résultats de l’exécution 2](../../../bench/garden/RESULTS.md), [chaque nombre](../../../bench/garden/results.json). Achevée à 06:02 UTC.

| Hypothèse                                                                | Verdict     | Mesuré                                                                                                                        |
| ------------------------------------------------------------------------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Mêmes pixels                                                             | oui         | TypeScript, Peras, Peras en debug et Rust, aux zooms 1, 4, 16, 64 en 1024 × 1024                                              |
| H1, le programme plus petit que son image                                | **tient**   | Peras livre 5 872 octets ; le plus petit AVIF à SSIM 0,95, 9 774 octets (4:2:0, CRF 53) ; le PNG sans perte, 482 545          |
| H2, une image stockée grandit avec le zoom                               | **tient**   | au zoom 64, la scène entière demande 4 452 fois les octets AVIF du zoom 1, contre 16 requis                                   |
| H3, Peras à 1,3 fois la vitesse de TypeScript et 0,85 fois celle de Rust | **réfutée** | 769 ms par image contre 914 pour TypeScript : 1,19 fois ; 855 ms pour Rust : Peras 1,11 fois plus rapide que Rust             |
| H4, l’œil pardonne de loin                                               | **tient**   | le SSIM de chacun des six fichiers monte réduit 4 fois : WebP en qualité 20 de 0,948 à 0,977, AVIF au CRF 50 de 0,956 à 0,966 |
| H5, aucun coût de la sûreté à l’image                                    | **tient**   | Peras en debug, son débordement vérifié, à 1,02 fois son temps de production                                                  |

### Analyse

**H2 résiste à son échantillon.** Les octets de chaque zoom viennent de 8 tuiles, aux coûts très dispersés : de 0,3 Kio pour une tuile de ciel à 83 Kio pour une tuile de prairie au zoom 16. Même si chaque tuile du zoom 64 coûtait le moins observé, la scène entière demanderait 142 fois les octets du zoom 1, contre 16 requis : aucun tirage de l’échantillon ne pouvait réfuter H2. Aux zooms 16 et 64, la croissance suit _z_² de près, 288 contre 256 et 4 452 contre 4 096.

**Une anomalie, inexpliquée.** Au zoom 4, la scène entière a demandé 0,9 fois les octets du zoom 1, et non environ 16 fois comme H2 le prévoyait pour chaque zoom ; les huit tuiles du zoom 4 coûtent toutes environ 0,5 Kio. Une explication est qu’aucune ne soit tombée dans la prairie, un quart du plan : huit tirages la manquent avec une probabilité d’environ 9 %. C’est une hypothèse, pas un résultat ; une exécution ultérieure avec plus de tuiles le dirait. Le verdict de H2 repose sur le zoom 64, comme enregistré.

**Reproductible.** Les deux exécutions concordent : les mêmes pixels et octets, et des temps à 1 % près, Peras à 1,21 puis 1,19 fois la vitesse de TypeScript.

**H3, réfutée deux fois.** Peras est plus rapide que TypeScript, et que Rust, à chaque exécution, mais pas de la marge enregistrée. V8 compile ce genre de code, de l’arithmétique flottante en boucles simples, près de ce que fait WebAssembly ; l’écart vient de l’empreinte entière et de l’absence de vérifications d’indices sur les pixels. L’avantage de Peras est plus grand là où il a été mesuré avant, sur les entiers et les tableaux de nombres ([banc dans le navigateur](../../../bench/README.fr.md#banc-dessai-dans-le-navigateur-benchbrowser)) ; cette scène n’est pas de ce genre. La marge de 1,3 était une estimation faite avant de mesurer, et elle était fausse.

**Ce que H1 ne dit pas.** Le même programme en TypeScript se livre en 1 119 octets, cinq fois moins que les 5 872 de Peras, qui portent le runtime WebAssembly et son chargeur ; Rust livre 11 695. C’est la scène procédurale qui est plus petite que son image ; des trois, TypeScript la livre la plus petite.

### Ce que cela signifie

- **Le programme comme compression tient.** Une scène de 6 Ko de code donne ce que donne une image de 10 Ko à un zoom, et ce que donneraient 42 Mo d’AVIF au zoom 64. Étirée, elle ne floute pas : elle ajoute des détails, ce qu’aucune image stockée ne peut.
- **L’œil pardonne de loin**, selon le modèle de SSIM : chaque format avec perte gagne réduit 4 fois, de 0,01 à 0,03. L’intuition du propriétaire tient ; les modèles plus proches de l’œil, Butteraugli et SSIMULACRA2, sont laissés à une exécution ultérieure.
- **La sûreté ne coûte rien ici** : les vérifications de débordement de la décision 0011 coûtent 2 % d’une image en debug, et rien en production.
- **Peras n’est pas beaucoup plus rapide que TypeScript sur des boucles flottantes.** Il rend la scène 1,2 fois plus vite, et 1,1 fois plus vite que Rust ; c’est la marge honnête, plus petite qu’espéré.
- **Son runtime pèse.** 5 Ko des 6 que livre Peras sont son runtime et son chargeur, dont une scène aussi petite n’a pas besoin : livrer sans eux un module sans tableaux sur le tas est une piste pour plus tard, mesurable par ce banc.

### Limites

Une seule machine, sans écran, 4 cœurs ; des versions écrites par les auteurs de Peras ; une scène de bruit, qui convient au procédural, là où une photographie n’a pas de programme court ; SSIM comme seul modèle de l’œil ; 8 tuiles par zoom pour H2. Aucune n’est cachée par les verdicts ci-dessus.
