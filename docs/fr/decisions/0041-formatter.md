# 0041 : Un formateur

_Version anglaise : [../../en/decisions/0041-formatter.md](../../en/decisions/0041-formatter.md)._

_État : protocole enregistré le 2026-11-23, avant tout prototype et toute mesure, comme le demande la règle du projet ; options mesurées au commit `878186e` ; f2 retenue par le propriétaire le 2026-11-23. C’est le deuxième pas de la phase 7 de la [feuille de route](../reference/roadmap.md), l’outillage, dans l’ordre que le propriétaire a choisi le 2026-11-23, après les tests de la [décision 0040](0040-tests.md) : « un formateur, pour que chaque fichier Peras se ressemble ». Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait Peras aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Rien ne formate un fichier Peras.** `perasc` n’a pas de commande `fmt` ; Prettier, qui formate le TypeScript, le Markdown et le JSON du dépôt à une largeur de 100 (`.prettierrc`), ne lit ni `.peras` ni `.apeiron`.
- **Le lexer écarte les commentaires et les blancs** (`Lexed::tokens`, « comments and whitespace excluded »), mais chaque jeton garde son intervalle : ce qui se trouve entre deux jetons, blancs et commentaires, se relit dans la source.
- **L’arbre syntaxique garde les parenthèses** (`ExprKind::Paren`) et un intervalle sur chaque nœud, mais un littéral garde sa valeur, pas son texte : `Int(u64)`, `Float(f64)`, si bien que `1_000`, `0xff` ou `1.50` ne se retrouvent que par leur intervalle.
- **Le dépôt contient 369 fichiers Peras, 12 147 lignes**, écrits à la main avec deux espaces d’indentation ; 63 lignes dépassent 100 caractères, un fichier contient une tabulation, 938 lignes contiennent un commentaire. Certains sont refusés exprès (les bugs des bancs d’essai).

## Options

- **f0. Comme aujourd’hui** : rien ne formate ; chaque auteur met en page comme il l’entend.
- **f1. Un formateur de jetons** : `perasc fmt` indente selon les crochets ouverts, met les espaces entre deux jetons d’après une table de leurs sortes, garde les retours à la ligne que l’auteur a écrits (une suite de lignes vides devient une ligne vide), et chaque commentaire là où il était. Une ligne longue reste longue.
- **f2. Une impression avec une largeur** : `perasc fmt` imprime le fichier comme Prettier imprime TypeScript, à une largeur de 100 : une liste, un appel ou une chaîne qui ne tient pas est coupé à raison d’un élément par ligne, avec une virgule finale ; les littéraux écrits comme la source les écrivait, les commentaires attachés au jeton qu’ils précèdent ou suivent. La mise en page est celle de l’imprimeur, pas de l’auteur, sauf les lignes vides entre instructions, ramenées à une. Elle est construite sur l’arbre que forment les crochets des jetons, pas sur l’arbre syntaxique : une liste, un appel, un bloc et une chaîne sont ce qu’elle coupe, et ils se trouvent dans les crochets ; chaque jeton, son texte et les commentaires qui l’entourent sont gardés par construction, ce que demande le premier seuil, là où une impression de l’arbre syntaxique reconstruirait chacun depuis son nœud. L’impression de l’arbre syntaxique est la manière de Prettier et de rustfmt ; elle est construite aussi si f2 manque un seuil ou le point 2 par ce que les crochets ne voient pas.
- **f3. Un plugin de Prettier** : confronté aux règles du projet (constat M004) et non construit : l’imprimeur TypeScript de Prettier ne sait pas lire Peras (`fn`, `inout`, `match`, `parallel for`, `test`), si bien qu’un plugin serait un second analyseur et un imprimeur écrits en JavaScript, ce que la règle « aucune dépendance à un autre langage que Rust » exclut pour les outils du compilateur lui-même ; un utilisateur devrait aussi installer Prettier pour formater un fichier que le compilateur lit.

## Protocole, enregistré avant tout prototype

f1 et f2 sont prototypées derrière `PERAS_PROTO_FMT`, dans `perasc fmt` (`--check` pour dire ce qui changerait, sans rien écrire). Chaque chiffre nomme le build du compilateur mesuré, un build de production (constat M029).

1. **Seuils, avant tout classement** : une option qui en manque un sur un fichier du corpus est déclarée en échec, avec le fichier.
   - **Sûreté** : le fichier formaté se découpe en les mêmes jetons, commentaires compris et dans l’ordre, à une virgule finale près que f2 ajoute ou retire ; et un fichier qui se construit donne le même module après formatage, octet pour octet, construit pour la production (`--allocator tlsf`, sans `--tests` : un build de test écrit la ligne d’un échec, qui bouge avec les lignes ; vérifié avant l’enregistrement qu’un module, possédé ou géré, de debug ou de production, ne garde aucune ligne de sa source) ; un fichier que le vérificateur refuse est refusé avec les mêmes codes.
   - **Idempotence** : formater un fichier formaté ne change rien.
   - **Entrée refusée** : un fichier avec une erreur de syntaxe est laissé tel quel, l’erreur signalée, jamais à moitié formaté.
2. **Chaque fichier se ressemble** (le but de la feuille de route) : six programmes, chacun écrit deux fois avec des mises en page différentes (sur une ligne, coupé partout, indentation mêlée, appels et chaînes longs, commentaires entre des arguments) ; si chaque option donne un seul texte pour les deux, et sinon combien de lignes diffèrent. Les lignes de plus de 100 caractères qui restent dans le corpus après formatage.
3. **Ce qu’elle change dans le corpus** : les lignes que chaque option change dans les 369 fichiers, et les fichiers changés.
4. **Vitesse et charge du processeur** : le corpus formaté avec `--check`, les deux options en alternance dans un même processus (constat M017), 11 tours, le rapport médian tour par tour et son étendue, le temps CPU sur le temps réel ; et le plus grand fichier du corpus seul, le temps qu’attendrait un enregistrement dans un éditeur.
5. **Mémoire et octets** : le pic de mémoire en formatant le plus grand fichier ; la taille que l’option ajoute à `perasc`, que chaque utilisateur télécharge.
6. **Facilité** : les notions qu’un utilisateur apprend (commandes, réglages), et les lignes du code du formateur, ses tests à part, que le projet entretient.
7. **Une option non construite** est déclarée comme telle, avec pourquoi, jamais classée sur une supposition (constat M001) : f3 ci-dessus.

`node scripts/bench-fmt.mjs` exécute les points 1 à 5 ; `--check` vérifie les seuils sur le corpus et tourne avec chaque vérification du projet.

## Registre

### La mesure (2026-11-23)

`node scripts/bench-fmt.mjs` au commit `878186e`, perasc construit pour la production, la table complète dans [`bench/fmt/RESULTS.md`](../../../bench/fmt/RESULTS.md). Le corpus est les 372 fichiers Peras du dépôt à ce commit, les douze programmes du point 2 parmi eux.

|                                                          | f0, comme aujourd’hui              | f1, jetons                                                                                         | f2, une largeur                                                                                        |
| -------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Seuils : refusés pour une erreur de syntaxe              |                                    | 43, chacun un fichier écrit faux exprès (tests du lexer et de l’analyseur, bugs des bancs d’essai) | les mêmes 43                                                                                           |
| Seuils : non idempotent, module changé, refus changé     |                                    | 0, 0, 0                                                                                            | 0, 0, 0                                                                                                |
| Seuils : jetons et commentaires gardés (`format_corpus`) |                                    | chaque fichier                                                                                     | chaque fichier                                                                                         |
| 2. Six programmes écrits deux fois : un seul texte       |                                    | aucun sur six, 6 à 16 lignes diffèrent                                                             | six sur six                                                                                            |
| 2. Lignes de plus de 100 qui restent                     | 66                                 | 66                                                                                                 | 7 : cinq gabarits, jamais coupés ; une somme, que f2 ne coupe pas ; un fichier refusé, laissé tel quel |
| 3. Ce qu’elle change dans le corpus                      |                                    | 16 fichiers, 52 lignes ajoutées, 53 retirées                                                       | 72 fichiers, 485 ajoutées, 290 retirées                                                                |
| 4. Le corpus avec `--check`, médiane de 11 tours         |                                    | 28 ms                                                                                              | 56 ms, 1,93 fois le temps de f1 (1,29 à 2,78)                                                          |
| 4. Temps CPU sur temps réel                              |                                    | 0,98                                                                                               | 0,96                                                                                                   |
| 4. Le plus grand fichier seul (`pond-bands.peras`)       |                                    | 4 ms                                                                                               | 5 ms : toutes deux à la résolution de l’horloge et du démarrage d’un processus                         |
| 5. Pic de mémoire, le plus grand fichier                 |                                    | 4,8 Mo                                                                                             | 5,7 Mo                                                                                                 |
| 5. Octets ajoutés à `perasc` (4 576 144 sans formateur)  |                                    | 16 400                                                                                             | 41 528                                                                                                 |
| 6. Notions qu’un utilisateur apprend                     | aucune, et aucun fichier semblable | `perasc fmt`, `--check` ; la mise en page reste à son choix                                        | `perasc fmt`, `--check` ; rien à choisir                                                               |
| 6. Lignes du code du formateur, ses tests à part         |                                    | 98, et 183 partagées                                                                               | 629, et 183 partagées                                                                                  |

Ce que disent les chiffres :

- **Les deux sont sûres** sur chaque fichier du dépôt : les mêmes jetons, les mêmes commentaires, le même module construit pour la production, les mêmes refus, un second passage qui ne change rien.
- **Seule f2 atteint le but de la feuille de route**, chaque fichier semblable : les six programmes écrits deux fois donnent chacun un seul texte avec f2 et aucun avec f1, qui garde ce que l’auteur a coupé. f1 change 16 fichiers du corpus, des espaces et de l’indentation ; f2 en change 72, surtout de longues listes de paramètres et d’arguments coupées à un élément par ligne, et des déclarations coupées comme Prettier coupe une interface.
- **f2 coûte plus là où personne n’attend** : deux fois le temps de f1 sur tout le corpus, 56 ms pour 372 fichiers ; un fichier enregistré dans un éditeur attend environ 5 ms avec l’une ou l’autre. 0,9 Mo de mémoire en plus, 25 Ko de plus dans le compilateur, six fois le code.
- **Ce que f2 ne fait pas encore** : elle ne coupe aucune arithmétique (`a * 100000 + b * 10000 + …` reste sur une ligne), et ne garde pas de ligne vide entre les champs d’une structure. Aucun des deux ne change un seuil ; les deux sont le travail d’une installation, mesuré par le même script.

Écarts au protocole : le pic de mémoire est lu dans Linux (`VmHWM` de `/proc/self/status`, demandé par `PERAS_FMT_PEAK`), cette machine n’ayant pas `/usr/bin/time` ; les octets sont ceux de trois builds de `perasc` avec une option, l’autre ou aucune, faits hors du dépôt aux commits `878186e` et `2f5f460`. f2 a été corrigée quatre fois avant la mesure, chaque fois après avoir lu ce qu’elle faisait au corpus (un bloc sans `;` imprimé comme une liste, des déclarations repliées, un gabarit coupé dans son `${…}`, l’argument seul d’un appel sans sa virgule finale) ; la mesure est celle du prototype tel que commité.

### Proposition

**f2** : la seule option qui rend chaque fichier semblable, ce que la feuille de route demande d’un formateur, aussi sûre que f1 sur chaque fichier du dépôt, pour un coût qu’un utilisateur ne sent pas (5 ms par fichier enregistré) et 41 Ko dans le compilateur. f1 est moins chère et laisse la mise en page de l’auteur, ce qu’elle ne peut pas s’empêcher de faire : deux fichiers du même programme restent deux mises en page. Le propriétaire décide. Si f2 est retenue, l’installation retire f1 et `PERAS_PROTO_FMT`, coupe l’arithmétique, garde une ligne vide entre les champs, et donne au serveur de langage de l’éditeur (le pas suivant de la phase 7) son formateur.

### Décision et installation (2026-11-23)

Le propriétaire a retenu **f2**. `perasc fmt [--check] fichiers…` imprime chaque fichier à une largeur de 100, sans réglage ; f1 et `PERAS_PROTO_FMT` sont partis, leurs chiffres plus haut et leur code dans git au commit 878186e, et la lecture du pic de mémoire aussi (`PERAS_FMT_PEAK`), une aide à la mesure seulement. Les messages de la commande sont en anglais et en français, un fichier qu’elle laisse pour une erreur de syntaxe nommé avec le nombre de ses erreurs. Le [guide](../guide/formatting.md) montre un fichier avant et après.

Les deux choses que la mesure a trouvé f2 ne pas faire sont faites : une longue somme se coupe après chaque `+` et `-` comme une longue condition après chaque `&&` et `||`, un signe jamais ; le corps d’une déclaration garde les lignes vides entre ses champs. Avec elles, 73 fichiers du corpus changent et 6 lignes restent plus longues que 100, les cinq gabarits et le fichier refusé. `scripts/bench-fmt.mjs` mesure le build d’aujourd’hui dans `bench/fmt/CURRENT.md`, son `--check`, dans la CI, tenant les seuils et le point 2 ; [`RESULTS.md`](../../../bench/fmt/RESULTS.md) garde les options telles que mesurées.

Pas fait, et laissé à leurs propres pas : les fichiers du dépôt lui-même ne sont pas encore formatés, puisqu’un test du lexer ou de l’analyseur vérifie un diagnostic à une ligne et une colonne que donne sa mise en page ; le plugin Vite ne formate pas ; le serveur de langage, le pas suivant de la phase 7, formatera à travers `perasc_syntax::format`.

Non-régression, `scripts/check-regression.mjs bea573e` : 231 modules comparés, aucun plus grand, plus petit ni changé ; temps de compilation 1,001 (0,979 à 1,081) ; le compilateur 1 568 octets plus petit qu’avec les deux prototypes, 45 288 octets plus grand que sans formateur (`2f5f460`), ce qu’apporte cette décision.
