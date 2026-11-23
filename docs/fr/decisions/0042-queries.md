# 0042 : Des requêtes pour les outils, la moitié du serveur de langage dans le cœur ouvert

_Version anglaise : [../../en/decisions/0042-queries.md](../../en/decisions/0042-queries.md)._

_État : protocole enregistré le 2026-11-23, avant tout prototype et toute mesure, comme le demande la règle du projet. C’est le troisième pas de la phase 7 de la [feuille de route](../reference/roadmap.md), l’outillage : « une extension d’éditeur et un serveur de langage ». Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

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
