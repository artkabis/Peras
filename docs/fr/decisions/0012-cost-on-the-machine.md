# 0012 : Le coût sur la machine, mesuré à chaque étape

_Version anglaise, qui fait foi : [../../en/decisions/0012-cost-on-the-machine.md](../../en/decisions/0012-cost-on-the-machine.md)._

## Contexte

Peras promet des programmes sans fuites, fortement typés, proches de TypeScript. Un langage qui tiendrait ces promesses en livrant plus de code, en tenant plus de mémoire ou en occupant davantage le processeur en reporterait le coût sur les machines des utilisateurs : leur batterie, leur forfait de données, l’énergie qu’ils consomment. Le propriétaire a demandé que ce coût soit surveillé dès maintenant, avant l’arrivée des outils d’audit, et que l’énergie soit pensée tout de suite.

Un navigateur ne dit pas à une page l’énergie qu’elle dépense, et la plupart des machines virtuelles non plus : les compteurs du processeur (RAPL sous Linux) sont absents de la machine où tournent les bancs. Ce qui se mesure, et ce qu’utilisent les études de l’énergie du web, ce sont les quantités qui la déterminent : les octets envoyés sur le réseau, le temps pendant lequel un cœur est occupé, et la mémoire tenue.

## Décision

Chaque étape du langage est mesurée sur quatre quantités, dans l’étude de cas de la [décision 0011](0011-what-to-build-next.md) (`node scripts/bench-case-study.mjs`, [résultats](../../../bench/case-study/RESULTS.md)), contre le même programme en TypeScript :

1. **Les octets livrés** : le JavaScript minifié par Vite, puis gzip, tel qu’une page le reçoit.
2. **Le temps CPU** : le temps pendant lequel les tâches de la page occupent un cœur sur tout le scénario, le `TaskDuration` de Chromium, la mesure de l’énergie la plus proche que donne un navigateur.
3. **La mémoire** : le tas JavaScript, après un ramassage, pendant que le tableau est affiché et une fois retiré.
4. **L’énergie** : celle du processeur, lue depuis Linux (RAPL) là où la machine la donne ; indiquée comme non mesurée ailleurs, jamais estimée.

Les fuites ne sont pas laissées à ces mesures : les modules owned passent leurs tests sur l’allocateur de debug, qui compte chaque bloc vivant ([décision 0001](0001-webassembly-backend.md)), et les modules managed signalent en debug chaque ressource ramassée sans avoir été libérée ([décision 0009](0009-browser-and-resources.md)).

Les octets livrés sont la seule quantité qu’une machine donne identique à chaque fois : la CI les tient à un **budget**, 1 600 octets pour l’étude de cas. Un changement qui en demande plus relève le budget ici, avec sa raison. Les autres varient d’environ 10 % d’une exécution à l’autre, et sont rapportées, pas vérifiées.

Trois règles en découlent, déjà appliquées :

- **Une fonction du runtime n’est écrite que si elle sert, et seulement dans la variante que ses types demandent.** `sortBy` par un nombre apporte le tri par base, par une chaîne la comparaison des points de code, jamais les deux.
- **Un build de production ne porte aucun code de débogage.** Sans le détecteur de fuites, une ressource est un simple objet avec `release()`.
- **Ce que JavaScript fait nativement, il le fait.** `toFixed` est celui de JavaScript dans un module managed, et celui de l’hôte dans un module owned : aucun octet ajouté. Une chaîne entièrement ASCII est passée en minuscules par `toLowerCase()`, 2,7 fois plus vite que le chemin général, avec le même résultat.

## Résultats

L’étude de cas à cette décision, médianes de 5 pages dans Chromium :

| Quantité                                      | TypeScript |      Peras |
| --------------------------------------------- | ---------: | ---------: |
| octets livrés, minifiés et gzip               |        633 |      1 512 |
| temps CPU, 1 000 commandes                    |   78–84 ms |   82–91 ms |
| temps CPU, 10 000 commandes                   | 467–539 ms | 520–540 ms |
| tas tenu pendant l’affichage, 1 000 commandes |    100 Kio |    123 Kio |
| tas laissé une fois retiré, 1 000 commandes   |    121 Kio |    143 Kio |

Ces mesures ont orienté le travail de cette étape : les fonctions du tri ont été séparées par type de clé et le code de débogage retiré des builds de production, de 2 513 à 2 124 octets gzip avant minification ; la mise en minuscules d’une chaîne, 1,3 ms gagnée pour 10 000 noms. Le temps CPU est dans le bruit des exécutions à 10 000 commandes, et de 5 à 10 % au-dessus à 1 000. Les 850 octets de plus sont le runtime qui tient les promesses : ressources, minuteurs libérés quand ils sont remplacés, groupes, tri. Le tas laissé une fois retiré est plus élevé parce que libérer un groupe laisse les lignes dans la page, le point 8 de la décision 0011, que retirera l’étape 7.

## Non retenu pour l’instant

- **Estimer l’énergie à partir des octets ou du temps** avec un modèle comme CO2.js : les coefficients d’un modèle ne sont pas une mesure ; les quantités dont il partirait sont rapportées telles quelles.
- **Un budget sur le temps CPU** : son bruit ferait échouer la CI au hasard ; une machine dédiée et calme pourrait en tenir un.
- **Écrire les fonctions du runtime plus courtes à la main** : un minifieur le fait pour les octets livrés, et la source reste lisible.

## Conséquences

- Les outils d’audit à venir peuvent partir de ces quatre quantités, dans les résultats de chaque banc.
- Chaque étape suivante de la décision 0011 les rapporte, et une étape qui coûte davantage à la machine dit pourquoi.
