# 0033 : Les valeurs partagées dans un module managed

_Version anglaise : [../../en/decisions/0033-shared-values-in-managed-modules.md](../../en/decisions/0033-shared-values-in-managed-modules.md)._

_État : protocole enregistré le 2026-11-17, avant tout prototype et toute mesure, comme le demande la règle du projet. Il suit la [décision 0032](0032-shared-values.md), dont l’option r1, `Rc<T>` en lecture seule et copié à l’écriture, est compilée par les modules owned ; un module managed refuse `Rc` (E0243). Le propriétaire a demandé cette étape le 2026-11-17._

## Contexte

Ce que fait le compilateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Un module managed partage ses structures.** C’est du JavaScript : une structure est un objet, et `let b = a; b.n = 5;` change aussi l’objet que nomme `a` (`two()` donne 455, `crates/perasc-codegen-js`). Un module owned refuse ce programme (E0204, `a` déplacé) : aucun programme ne se construit dans les deux modes en étant en désaccord.
- **Le `Rc<T>` d’un module owned est une valeur.** Écrire par un détenteur donne à ce détenteur sa propre valeur (décision 0032) : `let b = a; b.items.push(4);` laisse `a` avec trois éléments. Pour s’accorder, un module managed doit copier quelque part.
- **`copy()` d’une structure est `structuredClone`** (`crates/perasc-codegen-js/src/methods.rs`) ; un module managed n’émet aucune libération : ce qu’un détenteur tient part quand le ramasse-miettes voit que personne ne le tient.
- **Les tests comparent les deux modes** : un programme qui se construit dans les deux doit donner le même résultat (les tests de bout en bout des décisions 0028 et 0030).

## Options

- **m0. Refusé**, E0243, comme aujourd’hui : un module managed qui a besoin d’une valeur partagée l’importe d’un module owned.
- **m1. Partagé, copié à chaque écriture.** `Rc.new(v)` est `v` ; une copie d’un `Rc` est le même objet ; une écriture par un détenteur, un champ écrit ou une place changée à travers lui, donne d’abord une copie au détenteur, `b = structuredClone(b)`, quels que soient ceux qui le tiennent.
- **m2. Copié à chaque détenteur.** Une copie d’un `Rc`, une affectation, un champ ou un élément fait à partir de lui, est une copie de l’objet ; une écriture est sur place. Rien n’est partagé : mille lanternes tiennent mille maillages.
- **m3. Une boîte comptée, jamais libérée.** Un `Rc` est `{ v, n }` : une copie tient la même boîte et ajoute un à `n` ; une écriture par un détenteur dont la boîte a `n > 1` lui en retire un et donne au détenteur une nouvelle boîte, `{ v: structuredClone(v), n: 1 }`. Un détenteur qui finit ne retire rien : le compte ne peut qu’être trop haut, donc une copie peut être faite sans nécessité, jamais une copie manquer.

## Protocole, enregistré avant tout prototype

Chaque option est prototypée derrière `PERAS_PROTO_RCM` ; le build sans elle garde le comportement d’aujourd’hui, m0.

1. **Accord** : chaque fonction de `packages/vite-plugin-peras/e2e/fixtures/shared/shared.peras`, et P1 à P4 de la décision 0032 tels que corrigés pour r1, construits en module managed, donnent ce que donne le module owned. Une option en désaccord quelque part est écartée.
2. **Charges**, modules managed dans Node, le module de chaque option dans un même processus, l’ordre alternant, 21 tours, la médiane des rapports tour par tour et leur étendue (constat M017), contre le module owned de r1 sur TLSF pour l’échelle :
   - **W1**, le bassin de la décision 0032 où chaque lanterne tient son maillage, image 600 en 1024 × 768 avec 1 000 lanternes ;
   - **W2**, l’arbre de 1 000 000 de nœuds de la décision 0032, construit et sommé ;
   - **W3**, 1 000 000 d’écritures par un détenteur d’une valeur que deux tiennent : `a.value += i` dans une boucle, `b` la tenant aussi.
3. **Charge du processeur** : le temps CPU sur le temps réel des tours de chaque module.
4. **Mémoire et octets** : le tas JavaScript après chaque charge, après un ramassage ; les octets du module, bruts et en gzip.
5. **Facilité d’écriture** : les programmes du point 1 tels qu’écrits pour le module owned ; les erreurs que rencontre chaque option, et les lignes changées.
6. **Une option non construite** est dite telle, avec pourquoi, jamais classée sur une supposition (constat M001).

`node scripts/bench-shared-managed.mjs` les lance ; `--check` vérifie l’accord du point 1, et tourne avec chaque vérification du projet.

## Registre

_Chaque partie est consignée ici à son arrivée._
