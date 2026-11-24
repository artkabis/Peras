# 0042 : Des requêtes pour les outils, la moitié du serveur de langage dans le cœur ouvert

_Version anglaise : [../../en/decisions/0042-queries.md](../../en/decisions/0042-queries.md)._

_État : protocole enregistré le 2026-11-23, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `ca5dd0e` ; q2 retenue par le propriétaire le 2026-11-23. C’est le troisième pas de la phase 7 de la [feuille de route](../reference/roadmap.md), l’outillage : « une extension d’éditeur et un serveur de langage ». Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

La [décision 0006](0006-licensing.md) laissait ouvert si le serveur de langage est un outil du cœur ouvert ou un outil propriétaire, « décidé quand il sera construit ». Le propriétaire a décidé le 2026-11-23 : **le serveur de langage et l’extension VS Code sont propriétaires**, dans un dépôt privé (`peras-tools`), et **le cœur ouvert publie ce dont ils ont besoin**, une interface de requêtes de `perasc`, utilisable par tout outil, pour qu’aucun code AGPL n’entre en eux. Cette décision est cette interface.

Ce que fait Peras aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **`perasc check --json`** donne les diagnostics de fichiers sur disque, rien d’autre : pas ceux d’un texte qu’un éditeur tient sans l’avoir enregistré, et aucune réponse sur une position.
- **Le vérificateur sait déjà ce que les requêtes demandent** (`perasc_check::Analysis`) : le type de chaque expression (`types`), la déclaration à laquelle renvoie chaque nom lu (`names`), chaque déclaration avec son intervalle et son type (`bindings`), la méthode qu’atteint chaque appel (`method_calls`), les structures et leurs champs, les énumérations et leurs variantes.
- **Un fichier avec une erreur de syntaxe n’est pas vérifié** : `check_file` renvoie les diagnostics de l’analyseur et s’arrête, si bien qu’une requête dans un fichier en cours de frappe, `xs.` avant que sa méthode soit écrite, n’a pas d’analyse d’où répondre.
- **Une vérification prend environ 10 ms** sur le plus grand fichier du dépôt (`pond-bands.peras`, 355 lignes) avec le binaire dans le cache de pages, 0,34 s la première fois.

## Options

Chacune répond aux mêmes requêtes, en JSON, depuis la même analyse :

- `diagnostics` d’un texte, enregistré ou non ;
- `hover` à une position : le nom, ce qu’il est, son type ;
- `definition` à une position : où le nom est déclaré, dans son fichier ou dans un fichier qu’il importe ;
- `references` à une position : chaque endroit où la déclaration est lue, celle-ci comprise, ce dont un renommage a besoin ;
- `symbols` d’un fichier : ses fonctions, structures, énumérations, variables de module, avec leurs intervalles ;
- `completion` à une position : les champs et méthodes de la valeur avant un `.`, ou les noms en portée ;
- `format` d’un texte (décision 0041).

Les options :

- **q0. Comme aujourd’hui** : `perasc check --json` sur des fichiers enregistrés ; rien d’autre.
- **q1. Un processus par requête** : `perasc query <sorte> <fichier> [--at ligne:colonne]`, le texte sur l’entrée standard quand il n’est pas enregistré, une réponse JSON sur la sortie standard.
- **q2. Un processus pour la session d’un éditeur** : `perasc serve`, une requête JSON par ligne sur l’entrée standard et une réponse par ligne sur la sortie standard ; il garde chaque fichier ouvert et sa dernière analyse, ne vérifie de nouveau un fichier que quand son texte change, et répond depuis ce qu’il garde.
- **q3. Le serveur de langage dans le cœur ouvert** : pas une option depuis la décision du propriétaire ci-dessus ; nommée pour qu’on ne la croie pas oubliée.

## Protocole, enregistré avant tout prototype

q1 et q2 sont prototypées derrière `PERAS_PROTO_QUERY` ; sans elle, `perasc` est comme aujourd’hui, q0. Chaque chiffre nomme le build de `perasc` mesuré, un build de production (constat M029).

1. **Réponses, avant tout classement** : `bench/query/`, six programmes des sortes du dépôt (modules possédés et gérés, un import, une structure et une énumération, une fonction générique, des fermetures) et cinquante requêtes sur eux, chacune avec sa réponse attendue écrite à la main avant qu’aucun prototype ne tourne. Une option qui donne une seule mauvaise réponse est déclarée en échec, avec la requête. Les deux options répondent depuis le même code, donc donnent les mêmes réponses ; le seuil est que ce code soit juste.
2. **Un fichier en cours de frappe** : dix requêtes sur les mêmes programmes avec une erreur de syntaxe avant la position (une ligne inachevée, un `.` sans rien après) ; pour chaque option, quelles requêtes répondent encore, et quoi.
3. **Vitesse et charge du processeur**, depuis un client en Node, un seul processus alternant les options (constat M017), 11 tours :
   - chaque sorte de requête sur trois fichiers : 50 lignes, le plus grand du dépôt (355), et un de 5 000 lignes fait en répétant ses fonctions ; le temps médian de la requête envoyée à la réponse lue, et son 95e centile ;
   - un scénario de frappe : 100 modifications d’un fichier de 355 lignes, chacune suivie de `diagnostics` et `hover`, comme le demande un éditeur ; son temps total, et le temps CPU sur le temps réel.
4. **Mémoire et octets** : le pic de mémoire du processus de q2 avec chaque fichier du dépôt ouvert, et d’un processus q1 sur le plus grand fichier ; les octets que chaque option ajoute à `perasc`.
5. **Facilité** : le client qui exécute le scénario, écrit pour chaque option : ses lignes, les notions dont il a besoin (un processus, un protocole, un état à garder), les erreurs rencontrées en l’écrivant.
6. **Une option non construite** est déclarée comme telle, avec pourquoi, jamais classée sur une supposition (constat M001) : q3 ci-dessus.

`node scripts/bench-query.mjs` exécute les points 1 à 4 ; `--check` vérifie les réponses des points 1 et 2 et tourne avec chaque vérification du projet.

## Registre

### La mesure (2026-11-23)

`node scripts/bench-query.mjs` au commit `ca5dd0e`, perasc construit pour la production, les tables complètes dans [`bench/query/RESULTS.md`](../../../bench/query/RESULTS.md).

|                                                                           | q1, un processus par requête                                               | q2, un processus par session                                                                                                     |
| ------------------------------------------------------------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| 1. Bonnes réponses, sur 50                                                | 50                                                                         | 50                                                                                                                               |
| 2. Un fichier en cours de frappe, sur 10                                  | 10                                                                         | 10                                                                                                                               |
| 3. Une requête sur 53 lignes, médiane                                     | 3,0 à 3,7 ms                                                               | 0,1 à 0,4 ms                                                                                                                     |
| 3. Une requête sur 356 lignes, médiane (95e centile)                      | 7,1 à 11,5 ms (jusqu’à 22,3)                                               | 0,1 à 2,2 ms (jusqu’à 4,4)                                                                                                       |
| 3. Une requête sur 5 340 lignes, médiane (95e centile)                    | 72 à 139 ms (jusqu’à 165)                                                  | 0,1 à 27 ms ; un texte vérifié la première fois 63 ms                                                                            |
| 3. 100 modifications, chacune suivie de `diagnostics` et `hover`, médiane | 2 046 ms (1 915 à 2 087)                                                   | 862 ms (819 à 954), 2,4 fois plus vite                                                                                           |
| 3. Temps CPU sur temps réel, même scénario                                | 1,00                                                                       | 0,97                                                                                                                             |
| 4. Pic de mémoire                                                         | 5,5 Mo, un processus sur 356 lignes                                        | 20,7 Mo, les 378 fichiers Peras du dépôt ouverts                                                                                 |
| 4. Octets ajoutés à `perasc` (4 621 432 sans requêtes)                    | 124 976                                                                    | 166 584                                                                                                                          |
| 5. Le client du scénario                                                  | 18 lignes ; un processus par requête, le texte sur son entrée ; aucun état | 32 lignes ; un processus, des requêtes numérotées et leurs réponses appariées, les fichiers ouverts avertis de chaque changement |
| 5. Erreurs rencontrées en écrivant le client                              | aucune                                                                     | les types des réponses appariées par numéro, que l’outil de lint demandait                                                       |

Ce que disent les chiffres :

- **Les deux sont justes** : les cinquante réponses écrites à la main, et les dix sur un fichier en cours de frappe, depuis un seul moteur ; un fichier avec une erreur de syntaxe est analysé depuis l’arbre que l’analyseur a rétabli, ce qui répond à un survol, une définition ou aux membres après un `.` sur la ligne en cours de frappe. Le point 2 ne départage pas les options : elles répondent depuis le même code.
- **q2 répond à ce qu’un éditeur demande pendant qu’une personne tape, q1 non** : une réponse en un dixième de milliseconde depuis ce que q2 garde, contre 7 à 11 ms pour q1 sur le plus grand fichier du dépôt, et 72 à 139 ms sur 5 340 lignes, au-delà des 50 ms sous lesquelles on attend le survol d’un éditeur. Sur le scénario de frappe, q2 met 862 ms là où q1 en met 2 046.
- **q2 coûte de la mémoire et des octets** : 20,7 Mo avec chaque fichier du dépôt ouvert, là où un processus q1 prend 5,5 Mo et dure une requête ; 42 Ko de plus dans `perasc`, la lecture des requêtes JSON ; et un client qui garde un processus et numérote ses requêtes, 14 lignes de plus.

Écarts au protocole : une position de `queries.json` écrite avant tout prototype était fausse, sur le mot `mean` du texte d’un gabarit plutôt que sur `s.mean` (`report.peras`, ligne 5, colonne 30 pour 39) ; trouvée par la première exécution de q1, elle a été corrigée dans le commit des prototypes et sa réponse attendue laissée telle qu’écrite. Le temps CPU des processus de q1 est lu dans Linux comme le temps des enfants que Node a attendus (`cutime` et `cstime` de `/proc/self/stat`), le pic de mémoire d’un processus q1 échantillonné chaque milliseconde pendant qu’il tourne ; les octets sont ceux de trois builds de `perasc`, avec une option, l’autre ou aucune, faits hors du dépôt aux commits `ca5dd0e` et `628ce00`.

### Proposition

**q2**, `perasc serve` : le serveur de langage de `peras-tools` garde un processus pour la session d’un éditeur et obtient ses réponses en moins d’une milliseconde, là où q1 dépense une vérification pour chacune et dépasse 50 ms sur un grand fichier. Ses coûts sont ceux d’un processus qui reste : 20 Mo avec tout un dépôt ouvert, 42 Ko dans `perasc`. q1 est plus simple pour un script qui demande une fois, ce que servent déjà `perasc check --json` et `perasc fmt`. Le propriétaire décide. Si q2 est retenue, l’installation retire q1 et `PERAS_PROTO_QUERY`, écrit le protocole de `perasc serve` dans la référence, une page sur laquelle construit le dépôt privé, et ses réponses à un fichier qui en importe un autre suivent les changements de cet autre.

### Décision et installation (2026-11-23)

Le propriétaire a retenu **q2**. `perasc serve` est une commande de `perasc`, sans réglage ; `perasc query`, q1, et `PERAS_PROTO_QUERY` sont partis, leurs chiffres plus haut et leur code dans git au commit ca5dd0e. Son protocole est [une page de la référence](../reference/serve.md), sur laquelle construit le dépôt privé ; la référence de la commande la liste, ainsi que `perasc fmt`, que l’installation de la décision 0041 y avait oublié.

Un fichier suit maintenant les fichiers qu’il importe : le pilote lit le texte qu’un éditeur tient ouvert avant le fichier sur disque (`perasc_driver::set_open`), et un changement, ou une fermeture, d’un fichier analyse de nouveau chaque fichier ouvert qui l’importe. `scripts/bench-query.mjs` le vérifie, une fonction renommée dans `stats.peras` sans être enregistrée donnant ses erreurs à `report.peras`, renommée comme avant n’en donnant plus ; cassé exprès, les fichiers qui importent n’étant plus analysés de nouveau, le contrôle échoue, comme il le doit. Le script mesure le build d’aujourd’hui dans `bench/query/CURRENT.md`, son `--check`, dans la CI, tenant les cinquante réponses, les dix sur un fichier en cours de frappe et celle-ci ; [`RESULTS.md`](../../../bench/query/RESULTS.md) garde les options telles que mesurées.

Non-régression, `scripts/check-regression.mjs 8050bf7` : 242 modules comparés, aucun plus grand, plus petit ni changé ; temps de compilation 1,017 (0,968 à 1,127) ; le compilateur 8 528 octets plus grand qu’avec les deux prototypes, le retrait de q1 rendant peu d’un code qu’il partageait avec q2, les textes ouverts et les fichiers qui importent ajoutant davantage ; 180 504 octets plus grand que sans requêtes (`628ce00`), ce qu’apporte cette décision.

Après l’installation (2026-11-23) : le serveur de langage de `peras-tools` a besoin de l’intervalle et du message de chaque erreur, que `diagnostics` ne donnait pas ; il répond maintenant, à côté des codes, chaque erreur comme `perasc check --json` l’écrit (`items`), les mêmes objets, si bien qu’un outil lit une seule forme de diagnostic depuis les deux commandes.
