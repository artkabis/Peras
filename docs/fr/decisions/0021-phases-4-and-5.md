# 0021 : Les phases 4 et 5, et les lacunes du langage

_English version: [../../en/decisions/0021-phases-4-and-5.md](../../en/decisions/0021-phases-4-and-5.md)._

_Statut : acceptée le 2026-11-12 par le propriétaire. Un plan : chaque étape arrive avec ses tests, et cette page dit ce qui est arrivé._

## Contexte

La [feuille de route](../reference/roadmap.md) place Peras en phase 3, la bibliothèque standard. Le propriétaire demande ensuite les phases 4 et 5, avec les lacunes que les constats gardent ouvertes :

- **Phase 4, le mode managed.** C’est la moitié du design qui parle au navigateur. Sa prochaine étape, mesurée par la [décision 0011](0011-what-to-build-next.md), est celle des structures entre `.apeiron` et `.peras`.
- **Phase 5, les modules.** Un programme en plusieurs fichiers `.peras` n’est pas encore possible.

Ce qu’une étape change au langage est écrit dans le guide et l’état le jour où elle arrive.

## L’ordre

Chaque étape est assez petite pour arriver seule, et les plus utilisées viennent d’abord.

1. **Les structures et les énumérations entre `.apeiron` et `.peras`.**
   - Aujourd’hui, un module `.apeiron` ne peut appeler une fonction `.peras` qu’avec des nombres, des booléens, des chaînes, et des tableaux, options, résultats et tables de ceux-ci. Le `portable` du vérificateur refuse le reste.
   - Les deux modes donnent déjà une structure à JavaScript comme un simple objet, et une énumération comme `{ kind, values }`.
   - L’étape : un module managed importe les structures et les énumérations qu’un module `.peras` exporte, comme il importe ses fonctions : `import { area, Rect } from "./shapes.peras"`. Il peut les nommer, les construire, les lire, et les faire passer.
2. **Les petites lacunes, chacune vérifiée mais pas compilée :**
   - `%` sur les flottants (F304) ;
   - `==` sur des tableaux de chaînes ou de tableaux (F305) ;
   - `reduce` avec un accumulateur qui tient une chaîne ou un tableau (F307) ;
   - `copy()` sur une structure (F102).
3. **`import` et `export` entre modules `.peras`.**
   - Plusieurs fichiers compilés en un seul module WebAssembly : les noms résolus d’un fichier à l’autre, chaque fichier vérifié contre ce que les autres exportent.
   - Le design est écrit d’abord, dans sa propre décision, avant le code : fichiers, cycles, noms, ce que produit une construction.
4. **Les lacunes moyennes :**
   - le `let` au niveau du module (F306) ;
   - `u8`, `u16`, `u32`, `u64` compilés (F302) ;
   - un champ déplacé hors d’une structure possédée (F214) ;
   - une fonction nommée utilisée comme valeur (F308).

Plus tard, chacune avec sa décision :

- les types qui se contiennent eux-mêmes et `Rc<T>` (F310) ;
- `let … else` et un bras qui quitte une boucle (F101, F105), qui sont des questions de design plus que de code ;
- les API du navigateur que la phase 4 liste encore ;
- `extern` depuis des `.d.ts`, et les paquets.

## Critères de chaque étape

Une étape n’arrive qu’avec tout ceci :

- **Des tests en Rust, du vérificateur ou de l’IR.** Ils incluent les cas qui doivent rester refusés.
- **Des tests de bout en bout sur les deux allocateurs, là où l’étape atteint le navigateur.** Ils comparent avec JavaScript, et rien ne doit rester vivant.
- **Le test différentiel, là où l’étape touche ce que voient les optimisations des décisions 0016 à 0018.** Il tourne une fois avec 2 000 graines, et une fois avec le changement cassé exprès (M015).
- **Chaque vérification de `CLAUDE.md`,** y compris les tests de bout en bout sur un binaire release reconstruit (M018).
- **Le guide, l’état et la feuille de route mis à jour,** en anglais et en français.

## Journal

_Chaque étape est consignée ici quand elle arrive._

**Étape 1, les structures et les énumérations entre `.apeiron` et `.peras` : arrivée.**

- **Ce qui a changé.**
  - Le vérificateur donne à un module managed les structures et les énumérations des modules owned qu’il importe, par leur indice là-bas. Il prend, une fois et sous son nom, chacune qu’un import nomme ou que les types d’une fonction importée atteignent, ses champs et variantes traduits.
  - Une structure générique ne passe pas.
  - Un type du même nom qu’un type que le module managed déclare est signalé (`E0244`, `clash`).
  - Le JavaScript que produit un module managed n’importe que les fonctions.
- **Tests.**
  - Les tests UI du vérificateur : une structure importée et utilisée, une générique refusée, un nom en conflit.
  - Un test de bout en bout, sur les deux allocateurs : une structure imbriquée avec un tableau de chaînes, une énumération dans chaque sens et un tableau de structures, le résultat comparé et rien ne restant vivant.
  - Les champs d’une structure pris dans le désordre, exprès, le font échouer.

**Étape 2, les petites lacunes : en cours.**

- **`%` sur les flottants (F304) : arrivé.** `f64_rem`, une fonction du runtime écrite d’après le `fmod` de musl ; un `f32` calculé en `f64`. Testé contre le `%` de Rust sur 2 000 000 de paires aléatoires et les cas limites d’IEEE 754, contre celui de JavaScript de bout en bout, et dans le test différentiel. Un module qui appelle le runtime sans aucun tampon l’importe désormais (F218).
- **`==` sur des tableaux de chaînes et de tableaux (F305) : arrivé.** `buf_equal`, une fonction du runtime, à qui l’on donne la forme de chaque colonne : son type d’élément, et pour une chaîne ou un tableau, la forme de ce qu’il contient. Elle compare les longueurs, puis les éléments, en descendant dans chacun ; les flottants se comparent comme des nombres. Testé de bout en bout contre JavaScript sur les deux allocateurs, et dans le test différentiel sur 2 000 graines ; des flottants comparés comme des octets, exprès, les font échouer. Les tableaux de tables, d’ensembles, ou d’options qui contiennent de la mémoire restent signalés.
