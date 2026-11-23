# 0041 : Un formateur

_Version anglaise : [../../en/decisions/0041-formatter.md](../../en/decisions/0041-formatter.md)._

_État : protocole enregistré le 2026-11-23, avant tout prototype et toute mesure, comme le demande la règle du projet. C’est le deuxième pas de la phase 7 de la [feuille de route](../reference/roadmap.md), l’outillage, dans l’ordre que le propriétaire a choisi le 2026-11-23, après les tests de la [décision 0040](0040-tests.md) : « un formateur, pour que chaque fichier Peras se ressemble ». Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

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
