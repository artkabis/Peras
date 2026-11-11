# Un flux de capteurs

_English version: [../en/stream.md](../en/stream.md)._

Une application qui fait ce pour quoi Peras est fait : elle lit un flux de données, en texte, dans le navigateur, et le transforme en nombres assez vite pour suivre. Elle se mesure aussi pendant qu’elle tourne, et remet ce qu’elle a mesuré.

**[L’ouvrir](https://artkabis.github.io/Peras/stream/)** ([source](../../examples/stream)). Pour la lancer depuis le dépôt : `cargo build --release -p perasc`, puis `pnpm --filter peras-example-stream build` et `pnpm --filter peras-example-stream preview` pour mesurer. `dev` fonctionne aussi, avec un module compilé en debug, chaque accès mémoire et chaque débordement d’entier vérifiés, et plus lent.

## Ce qu’elle fait

- **Le flux.** 480 capteurs dans douze zones d’un bâtiment envoient une mesure toutes les cinq secondes : heure, capteur, zone, température, humidité et puissance, en texte CSV. Environ une ligne sur 400 est cassée, de cinq façons : un champ manquant, `NaN`, une virgule décimale, une zone en majuscules, un `%` après un nombre. Un lot est une fonction pure de son numéro et d’une graine : le même lot donne les mêmes octets à chaque exécution, dans chaque navigateur.
- **Le calcul, en Peras** ([`telemetry.peras`](../../examples/stream/src/telemetry.peras)). Un seul passage sur un lot : il découpe les lignes, lit trois entiers ou noms et trois décimaux avec `parseF64`, refuse ce qui est cassé, et somme par zone les mesures, les alertes, les températures, leur minimum et leur maximum, l’humidité et l’énergie ; il compte aussi un histogramme des températures et trouve la mesure la plus chaude. Il tourne dans un Web Worker : la page n’attend jamais.
- **Le même calcul en TypeScript** ([`digest.ts`](../../examples/stream/src/digest.ts)), écrit simplement, comme souvent. Si on le demande, le worker exécute les deux sur chaque lot, l’un puis l’autre à tour de rôle, et compare leurs résultats : les mêmes nombres, sommes de flottants comprises, puisque les deux additionnent dans le même ordre.
- **La page** ajoute le résultat de chaque lot aux totaux et les dessine : le temps par lot de chaque langage, les températures lues, et un tableau des zones.

## Ce qu’elle mesure

Chaque lot donne un échantillon :

| Champ          | Ce que c’est                                                                        |
| -------------- | ----------------------------------------------------------------------------------- |
| `batch`        | son numéro, à partir de 0                                                           |
| `rows`         | ses lignes                                                                          |
| `bytes`        | la taille de son texte                                                              |
| `generateMs`   | le temps de fabriquer son texte, hors calculs                                       |
| `perasMs`      | le calcul de Peras, passage du texte vers WebAssembly et retour du résultat compris |
| `typescriptMs` | le calcul TypeScript, ou vide quand il n’est pas comparé                            |
| `first`        | lequel a tourné en premier, `peras` ou `typescript`                                 |
| `agree`        | si les deux ont donné les mêmes nombres, ou vide quand ils ne sont pas comparés     |
| `live`         | les blocs de la mémoire de Peras encore vivants après l’appel : 0, ou une fuite     |
| `broken`       | les lignes que Peras a refusées                                                     |
| `injected`     | les lignes que le générateur a cassées exprès : `broken` doit l’égaler              |
| `at`           | les millisecondes depuis le début du flux, à la fin du lot                          |

La page en tire : les lignes lues ; les lignes par seconde de chaque calcul seul ; combien de fois Peras est plus rapide, sur les lots où les deux ont tourné ; le temps de Peras par lot aux 50e, 95e et 99e centiles ; combien de lots ont donné les mêmes nombres ; les blocs restants ; les lignes cassées ; et le texte lu par seconde.

**Exporter en JSON** donne le rapport entier : l’environnement (navigateur, cœurs, mémoire quand le navigateur la donne, le build), les réglages (lignes par lot, comparaison, graine), le résumé et chaque échantillon. **Exporter en CSV** donne les échantillons, une ligne par lot. La taille d’un lot et la comparaison sont figées dès que le flux a démarré, pour que chaque échantillon d’un export ait été pris sous les réglages qu’il nomme ; **Recommencer** repart de zéro.

## Les lignes cassées

Les lignes refusées ne sont pas perdues à cause de la vitesse. Le générateur casse environ une ligne sur 400 exprès, et sait lesquelles : la page compte, lot par lot, les lignes que Peras a refusées et celles cassées exprès, et n’affiche la tuile en vert que si les deux sont égales. Un test le vérifie ligne par ligne : sur 40 000 lignes, Peras refuse chaque ligne cassée et garde chaque ligne saine, avec les deux allocateurs ; le TypeScript refuse les mêmes, puisque les deux calculs donnent les mêmes comptes.

Refuser est un choix. `parseF64` et `parseI32` ne prennent un nombre que si tout le texte en est un : une lecture tolérante ferait de `21,37` un 21 et de `45.3%` un 45,3, des nombres faux sans bruit dans les moyennes. Peras compte plutôt la ligne comme cassée, et la part de lignes cassées est elle-même une mesure de la qualité des données. Mettre les lignes cassées de côté pour les examiner, plutôt que seulement les compter, n’est pas encore fait.

## Ce qu’elle n’est pas

- **Pas une mesure sous protocole.** Les chiffres que montre la page sont ceux d’une machine, à un moment, avec ce qu’elle fait tourner d’autre. Ils servent à observer et à exporter ; une affirmation sur Peras ne se fait que sous un protocole enregistré avant toute mesure, comme le fait la [décision 0014](decisions/0014-data-at-scale.md) pour une version plus grande du même travail.
- **Pas contre le TypeScript le plus rapide.** Le TypeScript est écrit comme souvent : `split`, expressions régulières, `Number`, une `Map` d’objets. Écrit à la main pour la vitesse, il irait plus vite ; la décision 0014 a mesuré une telle version, et Peras ne l’atteint pas encore.
- **Pas la mémoire du worker.** Aucun navigateur ne donne à un worker son propre tas JavaScript, et le chargeur ne dit pas jusqu’où la mémoire du module a grandi ([constat F108](findings.md)) : `live` dit seulement que rien ne fuit.

## Testé

- `examples/stream/e2e/agree.test.ts` : sur douze lots de 20 000 lignes et des cas limites, le calcul de Peras donne exactement celui de TypeScript, avec les deux allocateurs, sans fuite.
- `examples/stream/e2e/app.test.ts` : dans Chromium, l’application lit des lots, trouve les mêmes nombres dans chacun, ne fuit pas, alterne celui qui passe en premier, et exporte un rapport JSON et un CSV qui concordent.
- `examples/stream/test/metrics.test.ts` : les centiles, le résumé, l’ajout des résultats et le CSV.
