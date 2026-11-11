# 0013 : Le jardin d’Apeiron, une scène procédurale mesurée

_Version anglaise, qui fait foi : [../../en/decisions/0013-garden-of-apeiron.md](../../en/decisions/0013-garden-of-apeiron.md)._

_Statut : protocole, enregistré avant toute mesure. Les résultats seront ajoutés en dessous, qu’ils confirment les hypothèses ou non ; le protocole lui-même ne changera plus après la première mesure, et tout écart sera signalé comme tel._

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
