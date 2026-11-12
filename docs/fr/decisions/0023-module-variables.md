# 0023 : Les variables de module dans les modules owned

_Version anglaise, qui fait foi : [../../en/decisions/0023-module-variables.md](../../en/decisions/0023-module-variables.md)._

_État : proposée le 2026-11-12, avec ses critères enregistrés avant tout code ; étape 4 de la [décision 0021](0021-phases-4-and-5.md), constat F306._

## Contexte

Un `let` en tête de module est une variable que les fonctions du module partagent, et elle garde sa valeur d’un appel à l’autre. Un module managed (`.apeiron`) la compile en `let` JavaScript. Un module owned (`.peras`) la vérifie mais ne la compile pas : `E0900` (« constantes et variables de module »). Une `const` de module est compilée : sa valeur est écrite là où elle sert.

Ce que fait le code aujourd’hui (lu avant d’écrire ceci, après le constat M020) :

- **Le vérificateur** accepte un `let` de module de n’importe quel type, avec n’importe quelle valeur initiale. Il refuse d’en déplacer la valeur (`E0209`), donc une fonction ne peut que la lire, la prêter ou l’affecter.
- **L’IR** a des locaux et aucun autre endroit où garder une valeur. Chaque optimisation des décisions 0016 à 0018 raisonne sur des locaux : une valeur qu’elle n’a pas vue écrite dans une fonction est tenue pour inchangée.
- **Le backend WebAssembly** n’a pas de global à lui en dehors de ceux du runtime. Le backend Rust n’écrit que des fonctions.
- **Un `parallel for`** exécute ses tours sur des threads, chaque thread étant une instance du module avec ses propres globaux ([décision 0007](0007-threads-over-shared-memory.md)).

## Décision

1. **Une première version pour les nombres et les booléens.** Un `let` de module tient un entier, un flottant ou un booléen, ou une structure, une énumération ou une option faites de ceux-ci. En WebAssembly, chaque scalaire est un global mutable.
   - Une chaîne, un tableau, une table ou un ensemble dans une variable de module restent refusés (`E0900`, avec cette raison).
   - Tenir une telle valeur, c’est plus que la ranger. La mémoire qu’une variable garde resterait vivante entre les appels, donc la vérification des fuites de l’allocateur de debug, qui n’attend rien de vivant après un appel, devrait la compter comme une racine. C’est une étape à part.
2. **Sa valeur initiale est une constante**, selon la règle qu’une `const` de module suit déjà (littéraux, autres constantes, opérateurs sur eux). Elle est écrite dans le module et posée avant tout appel. Aucun code ne s’exécute au chargement du module, et l’ordre entre variables ne compte jamais.
3. **Sa valeur vit autant que l’instance du module.** Deux pages, ou deux workers, ont chacun la leur, comme les variables de module de JavaScript.
4. **Un `parallel for` ne peut ni en lire ni en écrire une** (`E0900`, avec cette raison). Chaque thread a sa propre copie des globaux, donc un tour verrait une autre valeur que le code autour de la boucle.
5. **Les optimisations tiennent une variable de module pour changée par tout appel et toute affectation à elle.**
   - Dans l’IR, la lire est une expression à part, et l’écrire une instruction à part ; ni l’une ni l’autre n’est un local.
   - Une passe qui ne les connaît pas laisse le code qui y touche tel qu’il est écrit. Les vérificateurs contrôlent que cela tient.

## Non retenu

- **Les chaînes et les tableaux dès maintenant.** Il faudrait que la vérification des fuites connaisse des racines, et que la libération de l’ancienne valeur à chaque affectation soit prouvée d’un appel à l’autre. Les deux sont possibles, et chacun est une étape à part.
- **N’importe quelle valeur initiale, exécutée au chargement du module.** Il faudrait un ordre entre variables, et un appel fait avant qu’aucune fonction ne soit appelée, qui peut s’arrêter sur une erreur avant que la page ait un module. Les constantes couvrent les compteurs, les drapeaux, les graines et les bornes.
- **Partager une variable entre les threads d’un `parallel for`.** C’est une course aux données sauf si elle est atomique, et Peras n’a pas d’atomiques.

## Critères, enregistrés avant tout code

L’étape n’arrive qu’avec tous ceux-ci, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape) :

1. **Des valeurs qui durent.** Un test de bout en bout, sur les deux allocateurs : un compteur, une somme flottante et un drapeau booléen, chacun gardé d’un appel de JavaScript à l’autre, et une structure de nombres affectée en entier et par champ. Les résultats égalent ceux de JavaScript.
2. **Chaque refus a un test UI :** une chaîne ou un tableau dans une variable de module, une valeur initiale qui n’est pas constante, et un `parallel for` qui en lit ou en écrit une.
3. **Le test différentiel** gagne une variable de module, lue et écrite dans ses boucles et dans une fonction qu’il appelle, et tourne sur 2 000 graines. Casser exprès la règle qu’un appel peut changer une variable de module le fait échouer, par exemple en laissant une valeur lue avant un appel resservir après lui.
4. **Les deux backends**, WebAssembly direct et Rust, donnent les mêmes résultats (le job « backends agree »).
5. **La documentation :** les constantes de module du guide deviennent les constantes et variables de module, et l’état et `E0900` sont mis à jour, en anglais et en français.

## Registre

_Chaque partie est consignée ici à son arrivée._
