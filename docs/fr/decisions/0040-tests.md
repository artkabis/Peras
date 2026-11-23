# 0040 : Des tests écrits en Peras

_Version anglaise : [../../en/decisions/0040-tests.md](../../en/decisions/0040-tests.md)._

_État : protocole enregistré le 2026-11-23, avant tout prototype et toute mesure, comme le demande la règle du projet. Il ouvre la phase 7 de la [feuille de route](../reference/roadmap.md), l’outillage, dans l’ordre que le propriétaire a choisi le 2026-11-23 : les tests, puis un formateur, puis un serveur de langage, puis `create-peras`. La [spécification](../spec.md#outillage-tests-et-sécurité) prévoit un bloc `test "nom" { … }` lancé par Vitest à travers un adaptateur. Sous la règle du propriétaire du 2026-11-17, l’option retenue doit être une amélioration, jamais une régression._

## Contexte

Ce que fait Peras aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Aucun test ne s’écrit en Peras.** Le langage n’a pas de bloc `test` ; `packages/vitest-peras` est un `package.json` et rien d’autre.
- **Un test s’écrit en TypeScript** : Vitest se sert du plugin Vite, si bien qu’un fichier de test importe un module `.peras` ou `.apeiron` et appelle ses fonctions exportées. Une fonction qui n’est pas exportée ne peut pas être testée ; une structure passe par les conversions du chargeur ; un échec désigne le TypeScript, pas le Peras.
- **Aucun exemple du dépôt n’a de tests à lui** ; les tests du compilateur sont en Rust et en TypeScript.

## Options

- **v0. Comme aujourd’hui** : des tests en TypeScript, qui appellent les fonctions exportées.
- **v1. Des blocs `test` dans le fichier** : `test "nom" { … }` au niveau supérieur d’un fichier `.peras` ou `.apeiron`, avec `assert(cond)` et `assertEqual(a, b)` ; un bloc atteint chaque fonction de son fichier, exportée ou non ; il n’est compilé que pour les tests, et `vitest-peras` présente chaque bloc comme un test de Vitest.
- **v2. v1, avec des comparaisons qui expliquent** : `expect(a).toBe(b)`, `expect(a).toEqual(b)`, `expect(r).toBeErr()` comme Vitest les écrit ; un échec dit ce qui était attendu, ce qui a été obtenu, et où dans le fichier `.peras`.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_TEST` ; le build sans elle garde le comportement d’aujourd’hui, v0. Chaque module mesuré est nommé avec son build (constat M029).

1. **Facilité d’écriture** : trois modules et leurs tests, écrits pour chaque option, comptés comme la décision 0028 les compte (lignes, notions à apprendre, erreurs rencontrées en écrivant) :
   - **M1**, un module owned, `parsePort(text): Result<u16, String>` et une fonction auxiliaire privée, dix cas ;
   - **M2**, un module managed, un texte de compte à rebours à partir d’une `Duration`, six cas ;
   - **M3**, un module owned de tableaux et de structures, une déduplication qui garde le premier de chaque clé, six cas.
2. **Bugs trouvés** : pour chaque module, trois versions avec un bug planté (une erreur d’un, une mauvaise branche, un cas oublié) ; pour chaque option, si la suite échoue, et ce que dit son échec : le test qui a échoué, les valeurs, la ligne de la source Peras.
3. **Coût** : un build de production d’un module avec ses tests donne, octet pour octet, le module sans eux ; la taille du build de test est donnée.
4. **Les deux modes** : chaque test passe dans un module owned et dans un module managed là où le module le permet ; un test de bout en bout de `vitest-peras`.
5. **Vitesse** : le temps de la commande au résultat de chaque suite, l’exécution propre de Vitest dans un même processus, 11 tours, la médiane et son étendue ; il est donné, non classé en dessous d’une seconde.
6. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-tests.mjs` lance les points 1, 2, 3 et 5 ; `--check` vérifie que les suites de chaque option passent sur les modules et échouent sur les bugs plantés, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
