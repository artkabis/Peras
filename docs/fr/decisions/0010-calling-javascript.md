# 0010 : Appeler JavaScript, avec des types déclarés vérifiés à la frontière

_Version anglaise, qui fait foi : [../../en/decisions/0010-calling-javascript.md](../../en/decisions/0010-calling-javascript.md)._

## Contexte

Après la [décision 0009](0009-browser-and-resources.md), un module managed atteint un sous-ensemble du navigateur typé à la main, et rien d’autre : aucune autre API, aucun paquet npm. Une évaluation du projet y a vu le premier obstacle à son usage dans une vraie application, et la recherche sur l’adoption va dans le même sens : les bibliothèques et le code existant pèsent le plus dans le choix d’un langage ([la recherche derrière Peras](../research.md#ladoption-et-ses-risques)). Typer le navigateur membre par membre ne peut pas suivre.

Peras a besoin d’une porte de sortie vers JavaScript qui garde ses garanties : ce qu’une fonction Peras reçoit doit être ce que dit son type, même quand cela vient d’une bibliothèque qui dit autre chose.

## Ce que d’autres ont fait

- **TypeScript** fait confiance à ses déclarations : un `.d.ts` faux, ou une bibliothèque qui change, donne à l’exécution des valeurs d’un autre type, et rien ne le remarque.
- **Elm** ne laisse entrer JavaScript que par des ports et des flags, dont les valeurs sont décodées et vérifiées ; une valeur fausse est une erreur à la frontière, jamais à l’intérieur du programme.
- **ReScript** lie JavaScript par des déclarations externes, crues comme celles de TypeScript.
- **io-ts et Zod** vérifient des valeurs contre un schéma à l’exécution, au prix d’écrire le schéma.

## Décision

1. **Les blocs `extern`.** Un module managed déclare le JavaScript qu’il appelle avec des types Peras, depuis un fichier ou un paquet npm :

   ```apeiron
   extern from "./chart.js" {
     type Chart;
     fn makeChart(canvas: CanvasElement): Chart;
     fn plot(self: Chart, values: Array<number>): i32;
     fn onPoint(self: Chart, f: (i32, number) => void): void;
     async fn load(url: String): Promise<Array<number>>;
   }
   ```

   `type Chart;` est une valeur que crée JavaScript et que Peras ne fait que passer. Une fonction dont le premier paramètre est un `self` d’un tel type est une méthode de ses valeurs. Un module owned ne peut pas en déclarer ([E0242](../errors/E0242.md)).

2. **Chaque valeur venue de JavaScript est vérifiée, toujours.** Ce que rend une fonction `extern`, ce que donne sa promesse, et ce que JavaScript passe à un rappel est vérifié contre le type déclaré, en production aussi : les nombres et leurs plages, les chaînes, les booléens, les tableaux élément par élément, les options (`null` et `undefined` sont `None`), les tables, les ensembles, les champs d’une structure, les variantes d’une énumération, `instanceof` pour les types du navigateur. La valeur est reconstruite d’après le type déclaré, pour que JavaScript n’ait plus prise sur elle. Une valeur qui ne correspond pas lève une `TypeError` qui nomme la fonction et le type déclaré, avant que le code Peras ne la voie.
3. **Les rappels sont des fonctions fléchées**, passées là où la fonction `extern` est appelée, avec les types de paramètres que donne la déclaration ; JavaScript les appelle à travers une enveloppe qui vérifie ses arguments.
4. **Les ressources ne traversent pas.** Un écouteur, un minuteur ou un groupe appartient à une variable de Peras, qui le libère ; JavaScript le garderait sans le libérer ([E0247](../errors/E0247.md)).

## Non retenu

- **Faire confiance aux déclarations**, comme TypeScript : aucun coût, et une déclaration fausse ou une bibliothèque qui change casse en silence les garanties de Peras.
- **Vérifier en debug seulement** : une bibliothèque qui se comporte mal en production passerait inaperçue, là où le bug coûte le plus.
- **Lire les fichiers `.d.ts`** : ils donneraient les déclarations, pas les vérifications, et leurs `any`, unions et surcharges ne se traduisent pas. Ils pourront plus tard proposer des blocs `extern` à vérifier et ajuster.

## État

**Construit.**

- **Syntaxe** : `extern from "…" { type …; [async] fn …; }`, et les types fonctions `(A, B) => R` dans ses paramètres.
- **Vérifications** : types opaques, fonctions, méthodes par `self`, rappels écrits comme des fonctions fléchées, et les quatre refus d’E0247.
- **Générateur** : chaque fonction est importée sous un nom à elle et appelée à travers une enveloppe qui vérifie ce qu’elle rend, et chaque rappel à travers une enveloppe qui vérifie ses arguments (`$callback`). Chaque type déclaré est compilé une fois, au chargement du module, en une fonction qui vérifie une valeur de ce type (`$shape`, `$valid`).
- **Tests** : dans Node, les fonctions, méthodes, promesse, option, structure et rappels d’une bibliothèque, et cinq qui mentent, chacune refusée avec son message ; dans Chromium, depuis le serveur de développement de Vite et son build de production, un paquet npm importé par son nom dessine sur un canvas, rappelle, rend un canvas vérifié par `instanceof`, et une de ses méthodes qui ment lève une erreur.

Coût mesuré, dans Node, une fonction `extern` qui rend un tableau d’un million de nombres, médianes de 15 appels : 3,8 à 6,6 ms pour le vérifier et le copier, contre 3 ms pour le `slice()` de JavaScript et 15 ms pour un `map` sur lui. Une première version relisait la description du type pour chaque élément, et prenait 77 ms ; compiler chaque type en une fonction une fois, au chargement du module, l’a rendue 12 à 20 fois plus rapide. Un résultat `void` n’est pas vérifié.

Pas encore : les fonctions `extern` génériques, les exports par défaut, les valeurs (et non les fonctions) exportées par un module, et la lecture des fichiers `.d.ts`.

## Conséquences

- Tout JavaScript, le reste du DOM et npm compris, peut être appelé depuis un module managed, ses valeurs tenues aux types déclarés.
- Une déclaration fausse est trouvée la première fois que le programme la rencontre, avec le nom de la fonction, au lieu de quelque part plus loin.
