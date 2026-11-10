# Vectoriser à travers la boucle extérieure — un plafond, mesuré avant de décider

_Version anglaise, qui fait foi : [README.md](README.md)._

**La question.** Les boucles vectorielles de Peras exécutent plusieurs tours d’une boucle intérieure à la fois. Mandelbrot et n-body n’y entrent pas : leur boucle intérieure est une suite (les itérations d’un point, la somme des attractions sur un corps) dont l’ordre fixe les bits du résultat. Leur boucle extérieure, elle, y entre : les pixels, et les corps, sont indépendants. En calculer deux à la fois dans les deux voies `f64` du SIMD de WebAssembly, chaque voie faisant ses propres calculs dans son propre ordre, laisse les résultats identiques, bit pour bit. JavaScript n’a pas de SIMD : cela changerait les égalités du [banc d’essai dans le navigateur](../../README.fr.md#banc-dessai-dans-le-navigateur-benchbrowser) en gains face au JavaScript écrit par un expert. L’estimation était de 1,5 à 1,8 fois pour les deux.

**La méthode.** [`src/lib.rs`](src/lib.rs) écrit chaque noyau deux fois à la main en Rust, avec les fonctions SIMD sûres de `core::arch::wasm32`, sans `unsafe` : une valeur à la fois, comme Peras les compile aujourd’hui, et deux voies à la fois, comme Peras pourrait les émettre, les tableaux en colonnes comme Peras les range. [`run.mjs`](run.mjs) vérifie que les deux, Peras aujourd’hui et JavaScript habituel et expert donnent le même résultat, bit pour bit, puis les chronomètre en alternance dans Chromium, trois tours. Les résultats sont dans [RESULTS.md](RESULTS.md).

**Ce qu’elle a trouvé.**

| Noyau      | Deux voies face à Peras aujourd’hui | Face au JavaScript le plus rapide |
| ---------- | ----------------------------------: | --------------------------------: |
| n-body     |                         1,81 – 1,85 |                      environ 1,85 |
| Mandelbrot |                         0,93 – 0,97 |                      environ 1,02 |

- **n-body gagne 1,8 fois**, comme estimé : chaque voie exécute la même boucle intérieure, mille attractions ; la paire d’un corps avec lui-même est masquée dans sa propre voie.
- **Mandelbrot perd 3 à 7 %**, contre l’estimation. Deux points voisins s’échappent rarement au même pas : une paire tourne aussi longtemps que son point le plus lent, sous un masque, et deux voies ne laissent pas de quoi le payer. Une première version, qui comparait à chaque tour le nombre de pas de chaque voie, perdait 12 à 14 % ; un seul compteur pour les deux voies l’a amenée à 3 à 7 %, pas au-delà. Quatre voies demanderaient des `f32`, qui changent les résultats.

**La décision.** Vectoriser à travers une boucle extérieure vaut la peine d’être construit pour les boucles comme celle de n-body : une boucle intérieure au même nombre de tours dans chaque voie, une condition au plus, prise sous un masque. Cela ne vaut pas la peine pour une boucle intérieure qui finit sur une condition qui diffère entre les voies, comme celle de Mandelbrot : une telle boucle reste une valeur à la fois. Comme pour le [SIMD de la décision 0004](../../../docs/fr/decisions/0004-strings-without-allocation.md#état), l’estimation était juste une fois et fausse une fois ; seule la mesure décide.
