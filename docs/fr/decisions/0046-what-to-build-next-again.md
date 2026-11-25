# 0046 : Que construire ensuite, mesuré de nouveau

_Version anglaise : [../../en/decisions/0046-what-to-build-next-again.md](../../en/decisions/0046-what-to-build-next-again.md)._

_Statut : protocole enregistré le 2026-11-25, avant qu’aucun programme soit écrit, comme le demande la règle du projet. Demandée par le propriétaire le 2026-11-25 : décider de la suite par la mesure, comme l’a fait la [décision 0011](0011-what-to-build-next.md), plutôt que par l’ordre de la [feuille de route](../reference/roadmap.md)._

## Contexte

La dernière fois que ce qu’il faut construire a été classé par la mesure, c’était la décision 0011, il y a trente-quatre décisions : dix-sept bogues d’applications web et une étude de cas écrite depuis TypeScript. Depuis, les décisions ont suivi l’ordre de la feuille de route, et trois de suite ont gardé ce qui était là : les arènes (0043) et les pools (0044) ont été mesurés et pas gardés, et le premier verdict de la 0045 aussi. Entre-temps, la seule friction trouvée en écrivant des programmes comme un utilisateur, une option non restreinte à travers `&&` et `||` (constat F118), valait une décision à elle seule : huit erreurs sur cinq programmes, aucune depuis la 0045.

Les frictions connues et pas encore classées, chacune d’un constat : le JSON dans les modules owned (`E0900`), un `let` jamais restreint, les lectures d’un champ que les outils ne savent pas résoudre (F313), un test en échec qui ne peut montrer ni tableau ni structure (F227), aucune allocation dans un tour d’un `parallel for` (F301), des variables de module qui ne peuvent être ni chaînes ni tableaux (décision 0023), les paquets (phase 5). Et l’étape suivante de la feuille de route, la règle `@frame`.

## Méthode, enregistrée avant qu’aucun programme soit écrit

### Les programmes

Quatre applications, chacune écrite d’abord en TypeScript, la spécification, puis en Peras en la traduisant comme le ferait un développeur venu de TypeScript, sans la remodeler pour ce que Peras offre. Elles sont choisies pour couvrir les deux modes et les sortes de travail que nomme la feuille de route, et aucune n’est un programme que le dépôt a déjà :

- **A1, des notes** (un module managed) : une liste de notes gardée dans le stockage de la page en JSON, cherchée à mesure que l’utilisateur tape, triée par date, avec des étiquettes ; le DOM, les événements, les minuteurs, le stockage, le JSON.
- **A2, un livre de comptes** (un module owned appelé depuis un module managed) : un CSV de transactions lu, regroupé par mois et par catégorie, avec un solde courant et les plus grands mouvements ; chaînes, tables, tri, montants d’argent.
- **A3, une scène de particules** (un module owned) : une image de particules qui naissent, bougent, se heurtent sur une grille et meurent, dessinées dans un framebuffer ; tableaux de structures, le travail d’une image, ce que `@frame` régirait.
- **A4, un calcul d’itinéraire** (un module owned) : le plus court chemin entre les arrêts d’un petit réseau lu depuis du JSON, avec des options pour un arrêt manquant ; des structures qui pointent les unes vers les autres, des options, une file de priorité.

### Ce qui est compté, pour chaque programme

1. **Les erreurs rencontrées** : chaque exécution de `perasc check --json` sur la version Peras telle qu’écrite, puis après chaque correctif, jusqu’à ce qu’elle se construise ; chaque diagnostic consigné avec son code, son message et le correctif fait. Un correctif est le plus petit changement qu’un utilisateur ferait à partir du diagnostic.
2. **Ce qui ne peut pas s’écrire** : une partie du TypeScript que la version Peras ne peut pas exprimer du tout, consignée avec ce qui a été fait à la place (`E0900`, une méthode manquante, un type manquant).
3. **Les lignes** : de chaque version, lignes vides et commentaires à part, et les lignes que la version Peras change par rapport à une traduction littérale.
4. **Les résultats** : chaque version Peras donne ce que donne son TypeScript, sur les mêmes entrées, construite pour la production (`--allocator tlsf`, constat M029).
5. **Le temps**, en second : le temps de chaque version sur sa charge face à celui de son TypeScript, dans un seul processus, l’ordre alterné à chaque tour, 21 tours, avec un contrôle A/A, le TypeScript chronométré contre lui-même dans les mêmes tours (constat M042).

### Le classement

Chaque erreur, chaque chose qui ne peut pas s’écrire et chaque ligne changée est rattachée à une cause : un constat existant, une étape de la feuille de route, ou un constat nouveau. Une cause est classée par les programmes qu’elle bloque (une partie qui ne peut pas s’écrire), puis par les erreurs qu’elle cause, puis par les lignes qu’elle ajoute. La liste classée est l’ordre des prochaines décisions ; une cause qu’aucun programme ne rencontre n’est pas classée, et le dit.

### Ce que ceci ne peut pas montrer, dit maintenant

Les programmes sont écrits par les auteurs de Peras, qui savent ce qu’il refuse : ils couvrent les applications que les auteurs connaissent, pas toutes les applications, comme la décision 0011 le disait de son corpus. Pour le borner : chaque version TypeScript est écrite et committée avant sa version Peras, et pas changée ensuite ; chaque correctif est celui que propose le diagnostic, ou le plus petit qu’un utilisateur ferait, consigné comme tel.

## Journal

_À venir : les programmes, les mesures, le classement._
