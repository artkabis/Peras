# Des requêtes pour les outils : les programmes et les réponses attendues

Pour la [décision 0042](../../docs/fr/decisions/0042-queries.md). [`programs/`](programs) contient six programmes des sortes du dépôt ; [`queries.json`](queries.json) contient cinquante requêtes sur eux, `queries`, et dix sur les mêmes programmes en cours de frappe, `typing`, chacune avec sa réponse, écrite à la main avant qu’aucun prototype ne tourne.

Le contrat d’une réponse :

- Une position est `ligne:colonne`, toutes deux à partir de 1, la colonne comptée en caractères, au premier caractère d’un nom.
- `hover` : le nom `name`, ce qu’il est `what` (`local`, `parameter`, `function`, `structure`, `enumeration`, `variant`, `field`, `module variable`), et son type `type` tel que Peras l’écrit ; pour une fonction, sa signature telle que sa déclaration l’écrit.
- `definition` : le fichier `file` et la position `at` du nom dans sa déclaration.
- `references` : chaque position où la déclaration est lue, la sienne comprise, dans l’ordre.
- `symbols` : les déclarations au premier niveau d’un fichier, chacune `name`, `what` et `line`.
- `completion` : les noms qu’elle propose comprennent (`includes`) ceux listés ; d’autres peuvent venir avec eux.
- `diagnostics` : les codes `codes` des erreurs du fichier, dans l’ordre.
- `format` : si `perasc fmt` laisse le texte inchangé (`unchanged`).

Une requête de `typing` remplace d’abord la ligne `replace.line` de son fichier par `replace.text`, ce qui laisse une erreur de syntaxe avant la position, puis demande.
