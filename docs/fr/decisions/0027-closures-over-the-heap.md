# 0027 : Les fermetures qui capturent des chaînes, des tableaux et des valeurs fonctions

_Version anglaise, qui fait foi : [../../en/decisions/0027-closures-over-the-heap.md](../../en/decisions/0027-closures-over-the-heap.md)._

_État : proposée le 2026-11-16, avec ses options et son protocole enregistrés avant qu’aucun prototype ne soit construit ni mesuré ; la première décision choisie en mesurant ses options, comme le propriétaire l’a demandé le même jour. Elle suit la [décision 0026](0026-closures.md), dont elle lève le refus `closure-heap`._

## Contexte

Ce que fait le code aujourd’hui, lu avant d’écrire ceci (constat M020).

```peras
fn label(prefix: String): (String) => String {
  return (s: String): String => `${prefix}${s}`; // E0900 : closure-heap
}
```

- **Un environnement tient des emplacements `i64`**, des nombres seulement, écrits quand la fermeture est faite et jamais après. Il est compté : une copie de la valeur fonction en ajoute un, la dernière libération le libère ([décision 0026](0026-closures.md)).
- **Un `const` ne peut pas changer, en profondeur** : `const xs = []; xs.push(1);` est `E0205`. Une chaîne ou un tableau `const` capturé ne peut pas changer tant que la fermeture vit, sauf s’il est déplacé ailleurs d’abord.
- **Les chaînes, tableaux, tables et ensembles ont un seul propriétaire** et se déplacent ([décision 0003](0003-runtime-proved-at-compile-time.md)) ; une copie s’écrit `copy()`. Rien n’est partagé sur le tas aujourd’hui, sauf les environnements.
- **La passe de propriété lit deux fois le corps d’une fonction fléchée**, comme le corps d’une boucle, et y refuse un déplacement.
- **Un module managed capture par référence**, comme JavaScript.

## Options

Chacune est construite en prototype quand c’est possible, et mesurée selon le protocole ci-dessous avant qu’une soit choisie.

- **A. Déplacer dans l’environnement.** Faire la fermeture y déplace ce qu’elle capture : la variable ne peut plus servir ensuite (`E0204`), sauf si le programme écrit `s.copy()` lui-même. L’environnement possède ses tampons et les libère avec son dernier détenteur. Aucune copie n’est faite en secret.
- **B. Copier dans l’environnement.** Faire la fermeture copie ce qu’elle capture, comme JavaScript le lirait, puisqu’un `const` ne peut pas changer : la variable reste utilisable, et le coût est une copie par fermeture faite.
- **C. Partager par compte.** Le tampon capturé est compté, comme un environnement : la fermeture et la variable tiennent le même. Il faut un compte sur chaque tampon, `Rc<T>`, le constat F310.
- **D. Prêter.** La fermeture emprunte ce qu’elle capture ; elle peut être appelée et passée plus bas, mais pas rendue, rangée, ni gardée au-delà de la portée de ce qu’elle emprunte. Ni copie ni compte ; une fermeture qui s’échappe est refusée.
- **E. Prêter, sinon déplacer.** D là où la fermeture ne s’échappe pas, A là où elle s’échappe.

## Protocole, enregistré avant tout prototype

Quatre grandeurs, comme le propriétaire l’a demandé : la vitesse, la charge du processeur, la mémoire et les octets, et la facilité d’écriture.

1. **Charges**, dans un module owned construit sur TLSF, exécuté dans Node, le module de chaque option construit du même source là où l’option l’accepte :
   - **W1, faite puis appelée une fois** : une fermeture qui capture une chaîne de 16 octets, 1 Kio et 64 Kio, faite, appelée une fois et abandonnée, 1 000 000, 100 000 et 2 000 fois.
   - **W2, appelée souvent** : une fermeture qui capture une chaîne de 1 Kio, appelée 20 000 000 de fois, chaque appel lisant un octet de celle-ci.
   - **W3, qui s’échappe** : `label(prefix)` rendue par une fonction, puis donnée à `map` sur 100 000 chaînes de 10 octets.
   - Le même source construit comme module managed, qui est du JavaScript, sert de référence.
2. **Vitesse** : le temps de chaque charge, le module de chaque option dans un même processus, l’ordre alterné à chaque tour, 21 tours, comparés par la médiane des rapports à ceux de l’option B, et leur étendue (constat M017).
3. **Charge du processeur** : le temps CPU du processus (`process.cpuUsage`, utilisateur et système) sur les 21 tours de chaque module, divisé par le temps écoulé : les cœurs gardés occupés, comme la décision 0012 mesure une page.
4. **Mémoire et octets** : le pic de la mémoire du module (`memory.buffer.byteLength`) et le plus de blocs vivants (`__perasHeap`) pendant chaque charge ; les octets du module, bruts et compressés par gzip.
5. **Facilité d’écriture** : cinq programmes, écrits pour chaque option comme un utilisateur les écrirait, en partant du JavaScript qu’il écrirait, puis corrigés jusqu’à ce qu’ils se construisent :
   - un fabricant d’étiquettes, `label(prefix)` rendue ;
   - un filtre par un tableau de mots capturé ;
   - une fermeture gardée dans une structure, avec un nom capturé ;
   - des rappels rangés dans un tableau, chacun capturant sa propre chaîne ;
   - une fermeture qui capture une valeur que la fonction utilise encore après.

   Pour chacun : les lignes changées par rapport au JavaScript, les notions qu’un utilisateur doit apprendre et que JavaScript ne demande pas, et les erreurs rencontrées avant que le programme se construise, avec leurs codes.

6. **Une option non construite** est rapportée comme telle, avec la raison, et classée sur rien de mesuré (constat M001).
7. **L’exactitude d’abord** : chaque prototype passe les tests de bout en bout de la décision 0026 et les résultats des nouvelles charges contre le module managed, sans rien laisser de vivant, avant que ses temps comptent.

## Critères de l’étape qui suit le choix

Fixés une fois une option choisie, avec le propriétaire : les tests UI, de bout en bout sur les deux allocateurs, le test différentiel, le vérificateur de libération, les docs, comme pour la décision 0026.

## Registre

_Chaque partie est consignée ici à son arrivée._
