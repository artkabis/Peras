# Peras pour les décideurs

**Peras est un langage pour les parties d’une application web où la vitesse et la justesse comptent le plus.** Il se lit comme TypeScript. Un module écrit en Peras est soit compilé en WebAssembly, avec une mémoire vérifiée par le compilateur et sans ramasse-miettes, soit compilé en JavaScript ordinaire. Les deux genres s’appellent depuis une application TypeScript existante, une fonction à la fois : rien d’autre ne change dans l’application.

Cette page donne ce dont un CTO, un architecte ou un responsable d’équipe a besoin pour décider s’il faut l’essayer, où et comment. Chaque chiffre y est mesuré dans ce dépôt, et lié à la façon dont il est mesuré.

## Deux genres de modules

| Fichier    | Compilé en  | Pour                                                                                                 |
| ---------- | ----------- | ---------------------------------------------------------------------------------------------------- |
| `.peras`   | WebAssembly | le calcul : traitement de données, images, simulations, analyse de texte, tout ce qui tourne souvent |
| `.apeiron` | JavaScript  | le code applicatif qui attend et appelle le calcul : `async`, `await`, `number`                      |

Les deux sont le même langage. Le compilateur vérifie la frontière entre eux, et entre eux et TypeScript ([modules managed](managed.md)).

```peras build
export fn average(samples: Array<f64>): f64 {
  return samples.reduce((sum, x) => sum + x, 0.0) / samples.length.toF64();
}
```

```ts
import { average } from "./stats.peras";

average([1, 2, 3, 4]); // 2,5, calculé en WebAssembly
```

## Ce qu’il apporte, mesuré

- **Le code écrit de la façon habituelle va à la vitesse du code écrit par un expert.** Face au JavaScript tel qu’on l’écrit d’habitude, dans Chromium : entiers 64 bits 20 fois plus rapides, tri 17 fois plus rapide que `array.sort((a, b) => a - b)`, texte 3,2 à 3,3 fois plus rapide que `split` et `filter`, structures 1,5 fois plus rapides qu’un objet par élément ([banc d’essai dans le navigateur](../../../bench/README.fr.md)).
- **Face au JavaScript le plus rapide qu’écrit un expert**, il est plus rapide sur les nombres en tableaux (1,6 à 2,4 fois), le tri, le texte et les entiers 64 bits, et aussi rapide sur l’arithmétique flottante seule. Il est plus lent sur les appels très courts, qui coûtent 1,4 à 1,7 fois un appel JavaScript : le travail doit traverser la frontière par gros morceaux.
- **Tous les cœurs** : une boucle `parallel for` tourne sur les cœurs de la machine avec les mêmes résultats, bit pour bit, 2,7 à 2,8 fois plus vite sur 4 cœurs pour une simulation ([threads](../decisions/0007-threads-over-shared-memory.md)).
- **Face à Rust et AssemblyScript**, les deux autres façons d’écrire du WebAssembly pour le web, écrits comme on les écrit d’habitude : à 6 % de Rust au plus, ou plus rapide (0,94 à 1,52 fois sa vitesse), 2,4 à 7,3 fois plus rapide qu’AssemblyScript là où il alloue ou parcourt des tableaux.
- **Le JavaScript qu’il écrit** (`.apeiron`) va à la vitesse du même JavaScript écrit à la main : 0,93 à 1,31 fois ([banc des modules managed](../../../bench/README.fr.md#modules-managed-benchmanaged)).

## Ce qu’il empêche

- **Les erreurs mémoire** : un bloc utilisé après sa libération, libéré deux fois, ou jamais libéré. Le compilateur refuse le programme, et chaque module est vérifié à nouveau par un vérificateur et, en développement, par un allocateur qui attrape ce qui passerait ([mémoire](memory.md)).
- **Les valeurs fausses silencieuses** : aucune conversion implicite, des entiers qui se replient partout de la même façon, un indice hors limites arrêté au lieu d’être lu, chaque cas d’un `match` traité. Un système de types statique seul attrape déjà environ 15 % des bugs publics des projets JavaScript ([la recherche derrière Peras](../research.md#les-vérifications-statiques-attrapent-des-bugs)).
- **Les échecs inexpliqués** : chaque erreur donne sa cause et sa correction, en français ou en anglais, et a sa propre page ([codes d’erreur](../errors/index.md)).

## Où il convient, et où pas encore

| Convient                                                                     | Ne convient pas encore                                                                                                                     |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| une fonction ou un module de calcul appelé depuis TypeScript                 | une application entière : seul un [sous-ensemble du navigateur](../reference/browser.md) est typé pour l’instant                           |
| des données volumineuses, numériques, ou traitées de nombreuses fois         | du code qui repose sur beaucoup de bibliothèques npm : chacune se déclare à la main dans un [bloc `extern`](managed.md#appeler-javascript) |
| une équipe avec une ou deux personnes prêtes à apprendre des règles strictes | une équipe qui a besoin dès aujourd’hui de l’outillage de TypeScript : pas encore d’extension d’éditeur                                    |

## La maturité, dite simplement

- **Version 0.0.1.** Jusqu’à la 1.0, une nouvelle version peut casser le langage. Ce qui est compilé, seulement vérifié, ou prévu est listé fonctionnalité par fonctionnalité dans [ce qui fonctionne aujourd’hui](../reference/status.md).
- **Un seul mainteneur**, avec un assistant IA ; pas encore de communauté. Le code est open source sous AGPL, avec une exception qui laisse les programmes compilés avec Peras à leurs auteurs ([licences](../decisions/0006-licensing.md)).
- **Testé à chaque changement** : chaque exemple de cette documentation est compilé par les tests ; les résultats de Peras sont comparés à ceux de JavaScript, de Rust et de l’interpréteur du compilateur ; les tests de bout en bout vérifient que les modules ne fuient pas. Les risques relevés par une revue extérieure sont listés dans [avant d’utiliser Peras en production](../reference/roadmap.md#avant-dutiliser-peras-en-production).

## Comment l’essayer sans risque

1. **Choisissez un module isolé** où la vitesse compte : un filtre, un analyseur, un calcul refait à chaque image. Gardez le reste de l’application en TypeScript.
2. **Confiez-le à une ou deux personnes volontaires** intéressées par la performance. La recherche sur l’apprentissage des langages montre que ce qu’un développeur sait déjà gêne l’apprentissage d’un nouveau langage qui lui ressemble ([la recherche derrière Peras](../research.md#ladoption-et-ses-risques)) : c’est plus facile à absorber dans une petite équipe.
3. **Mesurez avant et après**, en prenant les bancs d’essai de ce dépôt pour modèle. Gardez Peras là où la mesure montre un gain.
4. **Lisez les décisions** : chaque choix du langage est écrit, avec ce qui a été mesuré et ce qui a été écarté ([décisions](../decisions/0001-webassembly-backend.md), [recherche](../research.md)).

## Pour aller plus loin

- **Ingénieurs** : [premiers pas](getting-started.md), puis [valeurs et variables](basics.md).
- **Architectes** : [Peras et JavaScript](javascript.md), [modules managed](managed.md), [architecture](../architecture.md).
- **Responsables et auditeurs** : [ce qui fonctionne aujourd’hui](../reference/status.md), [feuille de route](../reference/roadmap.md), [la recherche derrière Peras](../research.md).
