# 0011 : Que construire ensuite, décidé par la mesure

_Version anglaise, qui fait foi : [../../en/decisions/0011-what-to-build-next.md](../../en/decisions/0011-what-to-build-next.md)._

## Contexte

Le mode managed se compile désormais en JavaScript ([0008](0008-managed-mode.md)), atteint le navigateur avec des ressources libérées par leur propriétaire ([0009](0009-browser-and-resources.md)), et appelle JavaScript et npm avec chaque valeur vérifiée ([0010](0010-calling-javascript.md)). Une évaluation du projet a demandé s’il apporte une vraie valeur face à TypeScript, ou s’il est naïf, et a listé plusieurs directions sans les classer : les fonctions fléchées à instructions, la règle contre l’état mutable de module, un tri avec comparaison, les structures entre modules, et davantage du navigateur.

Cette décision les classe sur des mesures, pas sur des avis : deux nouveaux bancs d’essai, chacun avec un script que chacun peut relancer.

## Méthode

1. **Bugs d’applications web** (`node scripts/bench-web-bugs.mjs`, [bench/web-bugs](../../../bench/web-bugs/RESULTS.md)). Dix-sept bugs d’applications web, chacun écrit comme on l’écrit naturellement en TypeScript et dans un module Peras managed : un élément absent, un écouteur ou un minuteur jamais arrêté, une bibliothèque qui ment sur ses types, un identifiant 64 bits au-delà de 2⁵³, un `await` oublié, un cas manquant, un indice au-delà de la fin, un total entier au-delà de 2³¹, un alias qui trie le tableau de l’appelant, la moyenne d’une liste vide, des données sans un champ. Pour chacun, le premier moment où la chaîne d’outils l’arrête : à la compilation (`tsc` au plus strict, puis le preset strict typé de typescript-eslint ; `perasc check`), à l’exécution par une erreur, dans Chromium, ou pas du tout, le programme donnant un résultat faux ou un écouteur écoutant encore.
2. **Une étude de cas** (`node scripts/bench-case-study.mjs`, [bench/case-study](../../../bench/case-study/RESULTS.md)). Un tableau de commandes, cherché après un délai, trié par montant, avec un résumé, retiré proprement : écrit en TypeScript (99 lignes), puis en Peras par un développeur venant de TypeScript, chaque refus du compilateur noté au fil de l’écriture. Puis les deux mesurés dans Chromium, sur 1 000 et 10 000 commandes.

## Résultats

### Les bugs

| Arrêtés                        | TypeScript (strict + typescript-eslint) | Peras, avant cette décision | Peras, après |
| ------------------------------ | --------------------------------------: | --------------------------: | -----------: |
| à la compilation               |                                       7 |                           8 |            9 |
| à l’exécution, avec un message |                                       1 |                           5 |            5 |
| **en silence**                 |                                   **8** |                       **3** |        **2** |
| impossibles à écrire           |                                       0 |                           1 |            1 |

TypeScript n’arrête rien des écouteurs et minuteurs laissés en marche (trois cas), ni d’une bibliothèque qui ment (deux). Peras refuse les cinq, les trois premiers à la compilation, les autres à la frontière avec le nom de la fonction. Peras était en retard à trois endroits :

- **Un `await` oublié** : typescript-eslint le refuse, Peras non. Corrigé : [E0248](../errors/E0248.md).
- **Un indice au-delà de la fin** : TypeScript le refuse à la compilation (`noUncheckedIndexedAccess`) ; Peras l’arrête à l’exécution, avec l’indice et la longueur.
- **Un total au-delà de 2³¹ dans un `i32`** : il se replie en silence en Peras, là où le `number` de JavaScript est exact jusqu’à 2⁵³. Encore ouvert : voir le point 5 plus bas.

Deux messages étaient trompeurs, et sont corrigés : un élément qui peut manquer était signalé comme une constante modifiée, et une ressource gardée dans une variable de module comme libérée aussitôt. TypeScript et Peras manquent tous deux un alias qui trie le tableau de l’appelant.

### L’étude de cas

Chaque refus rencontré en écrivant la version Peras, dans l’ordre :

1. **Une fonction fléchée ne peut pas contenir d’instructions** : `(e) => { ascending = !ascending; render(); }` est une erreur de syntaxe. L’état passe dans des variables de module, et le tableau ne peut donc être affiché qu’une fois par page.
2. **Un paramètre ne peut pas prendre le nom d’une variable de module** (`E0201`), conséquence du point 1.
3. **Pas de tri avec comparaison** : `sort()` ne trie que des nombres. Un tri par insertion écrit à la main le remplace.
4. **Une fonction fléchée n’est pas une valeur** hors des méthodes qui en prennent une.
5. **Le délai de recherche ne peut pas s’écrire** : il faut un minuteur gardé entre deux frappes et remplacé à chacune, et une ressource ne peut ni être gardée dans une variable de module, ni être remplacée. Le délai a été retiré.
6. **Une structure ne passe pas de `.apeiron` à `.peras`** (`E0244`) : le calcul ne peut pas passer en WebAssembly sans éclater chaque commande en tableaux parallèles.
7. **Pas de formatage des nombres** : `toFixed(2)` n’existe pas, et la page affiche `60016.28000000002` là où TypeScript affiche `60016.28`.
8. **Libérer le groupe laisse les lignes dans la page** : les écouteurs sont retirés, les éléments créés non.

Mesuré dans Chromium, deux séries de médianes de 5 pages neuves ; la vitesse de Peras en fraction de celle de TypeScript :

| Commandes | Étape   |   TypeScript | Peras, tri par insertion | Peras, tri par `extern` |
| --------: | ------- | -----------: | -----------------------: | ----------------------: |
|     1 000 | montage |   4,4–6,6 ms |              0,48 – 0,61 |             0,75 – 0,90 |
|     1 000 | tri     |   0,6–0,7 ms |              0,40 – 0,50 |             0,55 – 0,64 |
|    10 000 | montage | 21,1–25,4 ms |          **0,11 – 0,15** |         **0,74 – 0,79** |
|    10 000 | tri     |   4,8–4,9 ms |              0,54 – 0,56 |             0,67 – 0,79 |

Le tri par insertion, imposé par le point 3, rend Peras 7 à 9 fois plus lent à 10 000 commandes ; avec le tri de JavaScript par un bloc `extern`, Peras va à 0,74 à 0,90 fois la vitesse de TypeScript au montage. La version Peras compte 101 lignes contre 89, et son JavaScript pèse 1 777 octets gzip contre 860, l’essentiel étant les fonctions du runtime.

## Décision

Le mode managed garde sa place : sur les bugs mesurés, il en laisse passer 2 en silence là où TypeScript en laisse passer 8, et les cinq qu’il est seul à arrêter sont ceux qui coûtent le plus dans une application, les fuites et les bibliothèques qui mentent. Son coût est dans l’écriture, pas dans la vérification : chacun des huit refus de l’étude de cas est une fonctionnalité manquante, pas une règle qui a attrapé un bug. Les étapes suivantes sont rangées selon le nombre de ces refus que chacune supprime, et les bugs qu’elle ferme :

1. **Des fonctions fléchées à instructions, qui capturent ce qu’elles utilisent**, dans les modules managed : supprime les points 1, 2 et 4, et permet à un composant de garder son état dans ses propres variables, donc d’être affiché plus d’une fois. Une ressource capturée reste refusée, comme aujourd’hui.
2. **Remplacer une ressource libère la précédente** : `timer = setTimeout(…)` libère le minuteur qu’il remplace, comme une affectation libère une valeur en Rust. Le minuteur relancé du banc de bugs devient correct par construction, et le délai de recherche peut s’écrire (point 5). Une variable de module ne peut toujours pas tenir une ressource.
3. **Trier par une clé**, `xs.sortBy((o) => o.amount)`, nombres et chaînes, dans les deux modes : supprime le point 3 ; mesuré, 0,11–0,15 de la vitesse de TypeScript à 10 000 lignes devient 0,74–0,79.
4. **`toFixed(digits)`** et quelques formats de nombres : le point 7, que voient les utilisateurs.
5. **Le débordement d’entier s’arrête en debug**, comme en Rust : `+`, `-` et `*` sur les entiers lèvent une erreur au lieu de se replier dans un build de debug, et se replient comme aujourd’hui en production ; `wrappingAdd` et les autres se replient toujours exprès. Cela ferme le seul bug où TypeScript a raison et Peras se tait. Son coût dans les boucles WebAssembly est mesuré avant d’être adopté.
6. **Les structures entre `.apeiron` et `.peras`**, converties à la frontière comme les tableaux : le point 6, le calcul en WebAssembly sans tableaux parallèles.
7. **Des éléments possédés par un groupe**, retirés quand il est libéré : le point 8.

Construit avec cette décision, parce que chacun ferme un écart mesuré et coûte peu : [E0248](../errors/E0248.md) pour une promesse jamais attendue, une ressource dans une variable de module signalée là où elle est déclarée, un membre qui n’existe pas signalé une fois et comme tel, et `click()` sur les éléments.

## Non retenu pour l’instant

- **Lire les fichiers `.d.ts`** : l’étude de cas n’en a pas eu besoin, les blocs `extern` ont suffi.
- **Davantage du navigateur typé à la main** : les blocs `extern` atteignent le reste ; l’étude de cas n’en a eu besoin d’aucun.
- **Refuser les alias qui changent un paramètre** : les deux langages le manquent, et refuser `let sorted = items` changerait la façon dont les modules managed partagent les valeurs ([0008](0008-managed-mode.md)) ; à mesurer à nouveau quand les fonctions fléchées à instructions existeront.
- **Une sortie plus petite** : la moitié des octets de l’étude de cas sont les fonctions du runtime, écrites seulement quand elles servent ; à mesurer quand les fonctionnalités ci-dessus existeront.

## Avancement

- **Étape 1, faite** : des fonctions fléchées à instructions, qui capturent ce qu’elles utilisent, et les fonctions comme valeurs dans les modules managed ([guide](../guide/managed.md#fonctions-fléchées)). L’étude de cas garde son état dans le tableau qu’elle monte : les points 1, 2 et 4 ont disparu, et le banc vérifie que deux tableaux dans une page gardent chacun leur état, ce que la version d’avant ne faisait pas. 97 lignes au lieu de 101, 1 687 octets gzip au lieu de 1 777 ; le banc de bugs ne bouge pas, 2 en silence.
- **Étape 2, faite** : remplacer la ressource d’une variable `let` libère celle qu’elle tenait. Une fonction fléchée ne peut pas capturer une ressource ; une ressource qu’elle doit remplacer est donc gardée dans un `Slot` d’un groupe, `group.slot()` : `slot.set(timer)` libère le minuteur d’avant, et l’emplacement est libéré avec son groupe, qui tient aussi les écouteurs qui l’utilisent. L’étude de cas cherche après un délai de 50 ms, comme la version TypeScript : le point 5 a disparu. 100 lignes, 1 858 octets gzip, l’essentiel de la hausse étant les minuteurs et emplacements du runtime. Le minuteur relancé du banc de bugs reste refusé à la compilation, puisqu’il est dans une variable de module.
- **Étape 3, faite** : `sortBy` et `sortByDescending`, dans les deux modes ([méthodes](../reference/methods.md#trier-par-une-clé)). Mesuré d’abord : en JavaScript, un tri par base de chiffres de 8 bits sur les bits des clés, qui emporte les indices, a battu un tri d’indices par leurs clés et la comparaison de TypeScript dès 300 éléments ; sous 256, le tri natif est gardé. En WebAssembly, le runtime trie les indices, puis met chaque colonne dans leur ordre. L’ordre est le même dans les deux modes, chaînes comprises, et vérifié contre un tri stable de Rust dans l’interpréteur, en WebAssembly et en JavaScript. L’étude de cas monte 10 000 commandes à 0,68 à 0,85 de la vitesse de TypeScript au lieu de 0,13, sur quatre séries, autant qu’avec le tri de JavaScript par `extern` : le point 3 a disparu, et ce qui reste de l’écart n’est pas le tri. 90 lignes contre 89 ; son JavaScript pèse 2 500 octets gzip, fonctions du tri comprises ; [le banc de tri](../../../bench/sort/RESULTS.md) donne 3 à 7 fois la vitesse de TypeScript par un nombre dès 10 000 enregistrements.

## Conséquences

- Chaque étape suivante a une mesure qui dit si elle a réussi : l’étude de cas est réécrite avec elle, et les bancs relancés. Une étape qui ne les fait pas bouger n’est pas faite.
- La feuille de route suit cet ordre ; la page d’état et les bancs disent où en est chaque étape.
