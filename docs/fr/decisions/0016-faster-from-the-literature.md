# 0016 : Rendre le compilateur plus rapide, d’après ce qui a été mesuré et ce qui est publié

_English version: [../../en/decisions/0016-faster-from-the-literature.md](../../en/decisions/0016-faster-from-the-literature.md)._

_Statut : proposée le 2026-11-11. Chaque changement est mesuré quand il arrive, par une nouvelle exécution du protocole de la [décision 0015](0015-stream-measured.md), inchangé ; un changement qui n’y montre pas son gain est rapporté comme tel._

## Contexte

La décision 0015 a mesuré le calcul de l’application de flux sous un protocole enregistré : Peras est 1,5 à 1,7 fois plus rapide que le TypeScript tel qu’on l’écrit, mais 0,87 à 0,96 fois aussi rapide que le TypeScript écrit pour la vitesse. Un profil par retrait (`node scripts/profile-stream.mjs`) a ensuite trouvé où va le temps, pour 1 000 000 de lignes, sur environ 363 ms :

| Étape                                                               | Coût                          | Constat                |
| ------------------------------------------------------------------- | ----------------------------- | ---------------------- |
| Vérifier les lettres d’une zone avec `for (const c of s.split(""))` | 90 à 98 ms                    | [F207](../findings.md) |
| Découper chaque ligne en un tableau de champs                       | 40 à 58 ms                    | [F208](../findings.md) |
| Rendre `Option<Reading>`, et la valeur de repli de `unwrapOr`       | 35 à 55 ms                    | [F209](../findings.md) |
| `has` puis `get` sur une table à clés texte                         | 41 à 44 ms                    | [F210](../findings.md) |
| Parcourir les lignes                                                | environ 33 ms                 |                        |
| Lire les nombres                                                    | environ 30 ms en tout, bruité |                        |

Aucun de ces coûts ne vient de WebAssembly : la frontière fait 5 % (0015, H4). Chacun est un travail que le compilateur fait faire au programme sans qu’il l’ait demandé : un tableau qu’il ne garde jamais, un bloc pour une valeur qui vit un instant, un hachage calculé deux fois. Les compilateurs savent retirer chacun d’eux depuis des décennies. Cette décision liste, pour chacun, la technique publiée, comment elle s’appliquerait à Peras, et ce qu’elle devrait économiser.

## Les techniques

**1. Fusionner le parcours des caractères avec ce qui le consomme** (F207). Coutts, Leshchinskiy et Stewart, _Stream Fusion: From Lists to Streams to Nothing at All_, ICFP 2007, retirent la liste intermédiaire entre un producteur et son consommateur en les changeant tous deux en flux et en les fusionnant. Peras le fait déjà pour `for (const part of text.split(sep))` avec un séparateur littéral (F206), pas pour le séparateur vide. Le changement : le même parcours au fil de l’eau pour `split("")`, un caractère à la fois dans une seule vue, et `charCodeAt(i)` pour vérifier une classe de caractères sur un entier, sans aucune chaîne. **Attendu** : l’essentiel des 90 ms ; la vérification devient une boucle sur des octets.

**2. Trouver la recherche redondante et la faire une fois** (F210). Morel et Renvoise, _Global Optimization by Suppression of Partial Redundancies_, Communications of the ACM 22(2), 1979, retirent un calcul fait deux fois sur un chemin. Ici, le calcul est la recherche d’une clé : `m.has(k)` puis `m.get(k)`, ou `m.get(k)` puis `m.set(k, …)`, sans changement de `m` ni de `k` entre les deux. Le changement : l’IR reconnaît la paire et garde l’emplacement trouvé par la première. SipHash reste (Aumasson et Bernstein, INDOCRYPT 2012) : il protège contre des clés choisies pour entrer en collision, et le changement divise par deux le nombre de fois où il tourne. **Attendu** : environ la moitié des 41 à 44 ms.

**3. Ne pas allouer ce qui ne s’échappe pas** (F208). Choi, Gupta, Serrano, Sreedhar et Midkiff, _Escape Analysis for Java_, OOPSLA 1999, trouvent les objets qui ne survivent pas à la méthode qui les crée et les allouent sur sa pile. Tofte et Talpin, _Region-Based Memory Management_, Information and Computation 132(2), 1997, infèrent où la mémoire peut être libérée d’un coup. Reinking, Xie, de Moura et Leijen, _Perceus: Garbage Free Reference Counting with Reuse_, PLDI 2021, réutilisent un bloc libéré juste avant une allocation de même taille. Le changement : un tableau de vues créé dans le corps d’une boucle et qui n’en sort pas réutilise un même bloc d’une itération à l’autre. **Attendu** : l’essentiel des 40 à 58 ms.

**4. Rendre les petites valeurs en valeurs** (F209). La même analyse d’échappement s’applique au résultat d’une fonction : un `Option` de structure que l’appelant déballe aussitôt n’a pas besoin d’être un bloc du tas ; les résultats multiples de WebAssembly peuvent le porter. Et `unwrapOr(repli)` n’évalue sa valeur de repli que si la valeur manque, comme `??` en JavaScript, ce que demande F104. **Attendu** : l’essentiel des 35 à 55 ms.

**5. Parcourir plusieurs octets à la fois** (lignes, champs, nombres). Langdale et Lemire, _Parsing Gigabytes of JSON per Second_, The VLDB Journal 28(6), 2019, trouvent les caractères structurants d’un texte avec des instructions SIMD, plusieurs octets par instruction. WebAssembly a un SIMD de 128 bits, que Peras utilise déjà dans les boucles. Le changement : `split` et le parcours au fil de l’eau cherchent leur séparateur 16 octets à la fois. Lemire, _Number Parsing at a Gigabyte per Second_, Software: Practice and Experience 51(8), 2021, lit les décimaux exactement sans le chemin lent dans la plupart des cas ; le `parseF64` de Peras utilise le chemin rapide de Clinger (_How to Read Floating Point Numbers Accurately_, PLDI 1990) et demande à l’hôte au-delà. **Attendu** : une part des 33 ms des lignes et des nombres ; plus petite, et plus tard.

## Ordre

Par gain sur coût, chacun mesuré avant le suivant :

1. Caractères au fil de l’eau et `charCodeAt` (technique 1).
2. Une seule recherche pour `has` ou `get` puis `get` ou `set` d’une même clé (technique 2).
3. Réutilisation du tableau de vues du corps d’une boucle (technique 3).
4. `Option` de structure rendu en valeurs, et `unwrapOr` paresseux (technique 4).
5. Recherche SIMD des séparateurs (technique 5).

Si 1 à 4 donnent chacun l’essentiel de ce que montre le profil, le calcul passe d’environ 363 à environ 200 ms pour 1 000 000 de lignes : plus rapide que le TypeScript écrit pour la vitesse (B, environ 418 ms à 100 000 lignes dans 0015). C’est un espoir, pas un résultat : un profil par retrait donne le maximum qu’un changement peut économiser, et les moteurs de TypeScript progressent aussi. Chaque exécution de 0015 dira ce qui en tient.

## Ce qui ne change pas

- Le protocole de 0015, ses versions A et B, et ses seuils. Le programme Peras de l’application n’est pas réécrit à la main pour gagner : le gain doit venir du compilateur, pour tout programme écrit simplement.
- La sûreté : chaque changement garde les vérifications de l’allocateur de debug, le vérificateur de release et les tests des deux modes. Un bloc réutilisé reste un bloc dont le vérificateur peut rendre compte.

## Mesuré après l’étape 1

L’étape 1, les caractères de `split("")` parcourus au fil de l’eau, est faite : juste et testée, elle n’a fait gagner aucun temps (constat M013). Son profil a été refait, une chose retirée à la fois, et deux coûts qu’il cachait ont été mesurés :

- **Les allocations** (F211). Le calcul alloue 7 blocs du tas par ligne et en libère 7 : un littéral de chaîne copié à chaque usage, la vue d’un `split`, un curseur, deux copies. Une allocation et sa libération coûtent environ 13 ns : environ 90 ns par ligne, un quart du temps.
- **Les appels au runtime** (F212). Le runtime est sa propre instance ; un appel par un import coûte 2,06 ns dans Chromium, contre 0,73 pour un appel interne au module, que le moteur intègre (`node scripts/call-cost.mjs`). Le calcul fait 30 de ces appels par ligne : environ 40 ns, 11 % du temps.

L’ordre change donc. D’abord, **les littéraux en blocs statiques** (F211) : prêtés sans copie, jamais libérés ; c’est petit, et cela retire trois des sept allocations de chaque ligne. Ensuite, **le runtime lié dans le module** (F212) : cela retire le surcoût de chaque appel, environ 11 %, et laisse le moteur intégrer les petites fonctions du runtime, ce qui peut donner plus ; cette part n’est pas encore mesurée. Puis les étapes ci-dessus, qui retirent chacune une allocation.
