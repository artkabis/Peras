# `perasc serve` : des requêtes pour les outils

`perasc serve` répond aux questions qu’un éditeur pose pendant qu’une personne tape : les erreurs, le type d’un nom, où il est déclaré, où il est lu, ce qui peut s’écrire après un `.` ([décision 0042](../decisions/0042-queries.md)). Un processus dure la session d’un éditeur ; il garde chaque fichier ouvert avec ce que le vérificateur en a appris, et répond depuis cela en un dixième de milliseconde sur le plus grand fichier du dépôt. C’est par lui qu’un serveur de langage, une extension ou tout autre outil utilise Peras, sans son code ([décision 0006](../decisions/0006-licensing.md)).

## Le protocole

Une requête par ligne sur l’entrée standard, une réponse par ligne sur la sortie standard, toutes deux en JSON. Une requête a un `id`, que sa réponse rapporte, et une méthode `method` :

| `method`         | Champs                                          | Fait                                                                                                           | Répond           |
| ---------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------- |
| `open`, `change` | `file`, `text` (absent : le fichier sur disque) | tient `text` pour le fichier, enregistré ou non ; l’analyse de nouveau, et chaque fichier ouvert qui l’importe | `{ "ok": true }` |
| `close`          | `file`                                          | relit le fichier sur disque ; analyse de nouveau les fichiers ouverts qui l’importent                          | `{ "ok": true }` |
| `query`          | `file`, `kind`, `at`                            | répond depuis ce qu’il tient ; un fichier non ouvert est lu sur disque                                         | ci-dessous       |
| `exit`           |                                                 | termine le processus                                                                                           | rien             |

```json
{"id": 1, "method": "change", "file": "src/stats.peras", "text": "…"}
{"id": 1, "result": {"ok": true}}
{"id": 2, "method": "query", "file": "src/stats.peras", "kind": "hover", "at": "17:16"}
{"id": 2, "result": {"name": "total", "what": "function", "type": "fn total(xs: Array<f64>): f64"}}
```

Une position `at` est `ligne:colonne`, toutes deux à partir de 1, la colonne comptée en caractères. Une requête qui ne trouve rien à sa position répond `null`.

## Les requêtes

| `kind`        | Répond                                                                                                                                                                                                                                                                               |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `diagnostics` | `{ "codes": [...], "items": [...] }` : les codes des erreurs du fichier, dans l’ordre, comme `perasc check` les trouve, et chaque erreur comme `perasc check --json` l’écrit, son message, ses intervalles, sa cause et son correctif, dans la langue de `PERAS_LANG`                |
| `hover`       | `{ "name", "what", "type" }` : `what` vaut `local`, `parameter`, `function`, `structure`, `enumeration`, `variant`, `field` ou `module variable` ; `type` est écrit comme Peras l’écrit (`number` dans un module géré), celui d’une fonction comme sa déclaration écrit sa signature |
| `definition`  | `{ "file", "at" }` : la déclaration du nom, dans son fichier ou dans le fichier d’où il est importé                                                                                                                                                                                  |
| `references`  | `{ "at": [...] }` : chaque position où la déclaration est lue, la sienne comprise, dans l’ordre : ce que change un renommage                                                                                                                                                         |
| `symbols`     | `{ "symbols": [{ "name", "what", "line" }] }` : les fonctions, structures, énumérations et variables de module du fichier                                                                                                                                                            |
| `completion`  | `{ "names": [...] }` : après un `.`, les champs et méthodes de la valeur qui le précède ; ailleurs, les noms en portée                                                                                                                                                               |
| `format`      | `{ "unchanged", "text" }` : le fichier tel que `perasc fmt` l’imprime ([décision 0041](../decisions/0041-formatter.md)), ou `{ "refused": true }` pour un fichier avec une erreur de syntaxe                                                                                         |

Un fichier avec une erreur de syntaxe est analysé depuis l’arbre que l’analyseur rétablit : sur une ligne en cours de frappe, `xs.` avant que sa méthode soit écrite, un survol, une définition ou une complétion répondent encore.
