# 0006 : Les licences : un cœur ouvert sous AGPL, des programmes compilés libres, des outils propriétaires

_Version anglaise, qui fait foi : [../../en/decisions/0006-licensing.md](../../en/decisions/0006-licensing.md)._

## Contexte

Jusqu’à cette décision, Peras n’avait pas de licence. Le README disait « à décider », et les manifestes déclaraient `MIT OR Apache-2.0` en attendant. Un dépôt sans licence ne peut être ni utilisé ni enrichi en sécurité ; un texte provisoire qui se lit comme une licence permissive dit le contraire de ce que veut le projet. Une revue extérieure du projet a cité l’absence de licence parmi les raisons de ne pas l’adopter encore.

Ce que veut le projet :

- le langage reste open source : chacun peut le lire, le compiler, le modifier et le partager ;
- son code ne peut pas être repris dans un produit fermé, vendu ou proposé comme service, sans que le code de ce produit soit ouvert aussi ;
- les programmes écrits en Peras appartiennent à leurs auteurs, aux conditions qu’ils choisissent, fermées comprises : un langage dont le résultat porterait sa licence ne serait pas utilisé ;
- les outils greffés à Peras, l’analyseur de fuites mémoire et l’extension VS Code, sont gardés sous des conditions bien plus strictes.

Une licence open source, au sens de l’Open Source Initiative, ne peut pas interdire la vente. Ce qu’elle peut faire, c’est exiger qu’une version modifiée, quand elle est partagée, le soit avec son code source sous la même licence : c’est le copyleft.

## Options

| Licence                     | Ce qu’elle exige                                                                                      | Pourquoi pas                                                                                       |
| --------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| MIT, Apache-2.0             | rien que la mention                                                                                   | n’importe qui peut refermer Peras dans un produit : l’inverse du but                               |
| MPL-2.0                     | les fichiers de Peras modifiés sont publiés                                                           | Peras peut être intégré à un produit fermé, ses propres fichiers mis à part                        |
| GPL-3.0                     | un Peras modifié, quand il est distribué, est publié sous GPL                                         | un Peras modifié exécuté seulement sur un serveur, comme service, n’a pas à être publié            |
| **AGPL-3.0**                | l’exigence de la GPL, aussi quand un Peras modifié est proposé à des utilisateurs à travers le réseau | choisie                                                                                            |
| PolyForm Noncommercial, BSL | aucun usage commercial, ou pas avant une date                                                         | pas open source : les distributions, les entreprises et la plupart des contributeurs s’en écartent |

## Décision

1. **Le cœur est sous AGPL-3.0-or-later** : le compilateur et ses crates, le runtime, les plugins de build (`vite-plugin-peras`, `vitest-peras`, `create-peras`), le site et la documentation. Le texte complet est dans [LICENSE](../../../LICENSE).
2. **Les programmes compilés sont libres, par une exception.** Le runtime, les allocateurs et le chargeur JavaScript sont écrits dans chaque programme compilé : sous la seule AGPL, tout programme compilé avec Peras serait sous AGPL. L’[exception de runtime de Peras](../../../RUNTIME-EXCEPTION.fr.md), sur le modèle de la GCC Runtime Library Exception, permet à chacun d’utiliser et de distribuer un programme compilé aux conditions de son choix. Elle ne couvre ni Peras lui-même, ni le runtime extrait d’un programme compilé et utilisé seul : ceux-là restent sous AGPL, si bien que le runtime ne peut pas être repris dans un autre produit.
3. **Les exemples sont libres.** Les exemples de code de la documentation et de `examples/` sont aussi sous la licence BSD Zero Clause : les copier dans un programme ne lui impose rien.
4. **Les outils sont propriétaires.** L’analyseur de fuites mémoire et l’extension VS Code vivront dans un dépôt séparé et privé, tous droits réservés, distribués sous une licence d’utilisateur final, gratuits ou payants selon ce que décide le projet. Ils utilisent Peras par ce qu’il publie (sa ligne de commande, ses diagnostics en JSON, ses modules compilés), jamais en y reprenant du code sous AGPL, pour que leur propre code reste le leur. Les contrôles de fuites déjà présents dans Peras restent dans le cœur ouvert : l’allocateur de débogage qui détecte un bloc libéré deux fois ou laissé en vie, et `__perasHeap.checkLeaks()` ; l’analyseur est un outil construit au-dessus. Ce qui d’autre est un outil de ce genre, le serveur de langage par exemple, se décide quand il est construit.
5. **Les contributions gardent ouverte la porte d’une licence commerciale.** Une entreprise qui veut intégrer Peras lui-même dans un produit fermé peut se voir proposer une licence commerciale ; il faut pour cela les droits sur chaque contribution. [CONTRIBUTING.fr.md](../../../CONTRIBUTING.fr.md#licence-des-contributions) demande à chaque contributeur de placer sa contribution sous AGPL avec l’exception, et d’accorder au propriétaire du projet le droit de la licencier aussi à d’autres conditions ; les contributeurs gardent leur droit d’auteur.

## À vérifier avec un juriste avant une première version

- L’exception est écrite pour ce projet, d’après la GCC Runtime Library Exception ; un juriste devrait la lire avant qu’une version soit publiée sous elle.
- La plus grande partie de ce dépôt a été écrite avec un assistant d’IA, sous la direction du propriétaire du projet. Si et comment le droit d’auteur s’attache à un tel code diffère d’un pays à l’autre ; cela compte surtout pour vendre des licences commerciales.
- Le nom « Peras » n’est protégé par aucune licence ; une marque, si elle est voulue, est une démarche à part.

## Conséquences

- Les manifestes déclarent `AGPL-3.0-or-later`, `examples/fib` `0BSD`. `cargo deny` vérifie les licences des dépendances comme avant ; les crates du workspace, toutes `publish = false`, sont laissées hors de cette vérification.
- Chaque dépendance du compilateur est sous une licence compatible avec l’AGPL-3.0 (MIT, Apache-2.0, BSD, ISC, Unicode, Zlib) ; `libFuzzer` (NCSA) n’est utilisé que par les cibles de fuzzing.
- La feuille de route dit quels outils sont propriétaires.
