# 0025 : Un champ déplacé hors d’une structure, et les fonctions comme valeurs

_Version anglaise, qui fait foi : [../../en/decisions/0025-moved-fields-and-function-values.md](../../en/decisions/0025-moved-fields-and-function-values.md)._

_État : proposée le 2026-11-16, avec ses critères enregistrés avant tout code ; acceptée par le propriétaire le même jour, la partie B en vraies valeurs fonctions, les deux parties à livrer en un seul lot ; construite, ses critères tenus (voir le [Registre](#registre)) ; dernière partie de l’étape 4 de la [décision 0021](0021-phases-4-and-5.md), constats F214 et F308._

## Contexte

Ce que fait le code aujourd’hui (lu avant d’écrire ceci, après le constat M020), et ce que dit le compilateur, par les programmes ci-dessous.

**Un champ d’une structure que la fonction possède ne peut pas être déplacé (F214).**

```peras
struct Row {
  country: String;
  amount: i32;
}

export fn take(text: String): String {
  const row = Row(text.copy(), 3);
  const country = row.country; // error[E0209]: cannot move a field out of its structure
  return country;
}
```

- **La passe de propriété** (`perasc-check/src/owner.rs`) suit chaque chemin d’une fonction avec les variables déplacées sur ce chemin, `Path.moved`, d’une variable vers l’endroit où elle a été déplacée. Les chemins sont réunis après un `if`, un `match` et une boucle ; une variable déplacée dans un tour de boucle est déplacée au début du suivant. Un champ ou un élément consommé est refusé (`E0209`) quoi qui le tienne : un paramètre qui ne fait que le prêter, l’élément d’un `for…of`, ou une locale que la fonction possède.
- **L’abaissement** garde déjà chaque scalaire d’une structure dans une locale à lui, et suit chaque locale qui possède un tampon sur chaque chemin (`live`). Une structure est libérée feuille par feuille, à la fin de sa portée, avant un `return`, et là où les chemins se rejoignent. Un champ consommé est lu aujourd’hui en empruntant sa structure, puis en choisissant ses feuilles.
- Déplacer un champ demande donc au vérificateur de suivre une partie de variable. Pour l’abaissement, cela revient à déplacer quelques-unes des locales de la structure, ce qu’il sait déjà faire pour des variables entières.
- Le coût du constat : le tableau de bord copie le pays une seconde fois là où il pourrait le prendre, 7 ms sur 322 par 1 000 000 de lignes.

**Une fonction ne peut pas servir de valeur (F308).**

```peras
fn twice(x: i32): i32 {
  return x * 2;
}

export fn a(xs: Array<i32>): Array<i32> {
  return xs.map(twice); // owned : E0238, cette méthode prend une fonction fléchée
}                       // managed : E0210, `(i32) => void` attendu, `fn twice` trouvé

export fn b(n: i32): i32 {
  const g = twice;
  return g(n); // E0221 dans les deux modes : `()` ne s’applique pas à `fn twice`
}

fn apply(f: (i32) => i32, x: i32): i32 { // owned : E0242, un type fonction appartient aux modules managed
  return f(x);
}

export fn c(n: i32): i32 {
  return apply((x) => x + 1, n); // owned : E0238, une fonction fléchée n’est une valeur que dans un module managed
}
```

- **Le vérificateur** donne à une fonction nommée le type `fn nom` (`Ty::Fn`), que rien ne peut appeler. Un type fonction `(A) => R` (`Ty::Callback`) ne s’écrit que dans un module managed, ou dans un bloc `extern`. Une valeur de ce type s’appelle dans un module managed.
- **Dans un module managed**, `apply` et `c` sont acceptés. `xs.map(twice)` est refusé parce que le type du résultat de `map` est tiré du corps d’une fonction fléchée, et `twice` n’en a pas ; c’est un bogue du vérificateur, trouvé en écrivant ceci.
- **Dans un module owned**, une fonction fléchée n’existe que comme argument d’une méthode comme `map`. Elle est abaissée sur place, à l’appel (`lower/functional.rs`), ses paramètres liés à chaque élément et ses captures lues dans les locales qui l’entourent. Aucune valeur fonction n’existe à l’exécution.
- **Une fonction exportée** qui prend ou rend une fonction est refusée dans les deux modes (`arrow_export`) : JavaScript l’appellerait avec des fonctions dont les types ne seraient pas vérifiés.

## Décision

### Partie A : un champ déplacé hors d’une structure (F214)

1. **Ce qui peut être déplacé.** Un champ d’une structure tenue par une variable que la fonction possède peut être déplacé : un `let` ou un `const` déclaré dans la fonction, ou un paramètre `sink`. Il est déplacé là où une valeur est consommée : `const country = row.country;`, un argument d’un paramètre `sink` ou d’un constructeur, un `return`, `push`.
   - Le chemin ne passe que par des champs : `row.place.city` se déplace.
   - À travers un élément, une option, une liaison de `match`, l’élément d’un `for…of`, un paramètre qui ne fait que prêter, ou une variable de module, c’est toujours refusé (`E0209`). Ceux-là n’appartiennent pas à la fonction.
2. **Le vérificateur suit les places déplacées comme il suit les variables déplacées.** Une place est une variable et un chemin de champs. Elle est suivie sur chaque chemin, réunie après `if`, `match` et les boucles, avec la règle des boucles inchangée.
3. **Après le déplacement :**
   - Le champ déplacé, ou un champ en lui, lu ou déplacé, est signalé (`E0204`). Le message nomme le champ et l’endroit où il a été déplacé.
   - La variable entière, lue, déplacée, copiée, passée, rendue ou filtrée par `match`, est signalée aussi (`E0204`) : elle est en partie déplacée, et le message nomme le champ.
   - Ses autres champs restent utilisables.
   - Affecter le champ déplacé (`row.country = …`) le rend ; affecter la variable la rend tout entière.
4. **Une fonction fléchée** ne peut pas déplacer un champ d’une variable qu’elle capture, comme elle ne peut pas déplacer la variable.
5. **L’abaissement déplace les locales du champ**, qui quittent l’ensemble des locales vivantes. Le reste de la structure est libéré comme aujourd’hui, feuille par feuille. L’IR ne gagne rien ; le vérificateur de libération prouve chaque tampon libéré une fois sur chaque chemin.
6. **Un module managed** n’a pas de déplacements : rien n’y change.

### Partie B : les fonctions comme valeurs (F308)

1. **Une fonction nommée est une valeur de type `(A, B) => R`** partout où sa signature convient, dans les deux modes :
   - passée à `map`, `filter` et aux autres méthodes qui prennent une fonction : `xs.map(twice)` ;
   - liée à une variable, passée en argument, rendue ;
   - tenue dans un champ, un tableau ou une option.
2. **Un type fonction s’écrit aussi dans un module owned :** `(i32) => i32` comme paramètre, résultat, variable, champ, élément ou option. `E0242` ne s’y applique plus.
   - Une fonction exportée qui en prend ou en rend un reste refusée dans les deux modes.
   - L’égalité entre fonctions reste refusée.
3. **Une valeur d’un type fonction s’appelle :** `f(x)` quand `f` est une variable ou un paramètre, et `(s.handler)(x)` à travers toute expression.
4. **Une fonction fléchée est une valeur dans un module owned quand elle ne capture rien.** Une qui capture une variable est refusée (`E0900`, `closure`) ; les fermetures sont une étape à part, toujours prévue. Un module managed garde ses fonctions fléchées telles qu’elles sont.
5. **Ce qui ne peut pas être une valeur :**
   - une fonction générique, sauf si ses arguments de type sont donnés par le type attendu ;
   - une fonction avec un paramètre `sink` ou `inout`.

   Un type fonction n’a pas de conventions : ses paramètres sont prêtés et son résultat appartient à l’appelant. Chaque refus est signalé là où la fonction sert de valeur.

6. **En WebAssembly, une valeur fonction est un `i32`**, son indice dans la table des fonctions du module (`funcref`). Un appel est un `call_indirect` avec le type de sa signature, que WebAssembly vérifie.
   - Une fonction n’entre dans la table que si elle sert de valeur.
   - L’IR gagne `FnRef(FnId)`, un `i32`, et un appel indirect.
   - Le backend Rust écrit l’appel comme un `match` sur l’indice parmi les fonctions de cette signature, sans `unsafe` ; l’interpréteur appelle la fonction que l’indice nomme.
7. **Ce que voient les optimisations.** `xs.map(twice)`, avec `twice` nommée là où elle est passée, est abaissé comme `(x) => twice(x)` : un appel direct, aussi rapide qu’aujourd’hui. Un appel à travers une valeur est opaque pour les optimisations, comme tout appel.

### Ordre

La partie A arrive d’abord, puis la partie B, chacune avec ses critères et son registre. Une partie qui ne tient pas ses critères n’arrive pas, et l’autre n’est pas retenue par elle.

## Non retenu

- **F308 par résolution statique seulement.** `const g = twice` et `xs.map(twice)` seraient résolus à la compilation, sans valeur fonction à l’exécution. C’est plus petit, mais cela laisse `fn apply(f: (i32) => i32, …)`, l’usage qui fait des fonctions des valeurs, aux modules managed.
- **Les fermetures**, des fonctions fléchées qui capturent et survivent à leur appel. Elles demandent un environnement alloué, possédé et libéré, et leur propre décision.
- **Une fonction qui passe à JavaScript.** Il faudrait que le chargeur fasse d’une fonction JavaScript un indice, et d’un indice une fonction JavaScript, avec ses types vérifiés à chaque appel.
- **Pour F214, une copie implicite, ou un mot à lui comme `take(row.country)`.** Une copie est le coût que le constat nomme. Un déplacement se lit comme se lit déjà celui d’une variable, en Peras comme en Rust.

## Critères, enregistrés avant tout code

Chaque partie n’arrive qu’avec tous ses critères, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape).

### Partie A

1. **Tests UI du vérificateur.**
   - Refusés (`E0204`) :
     - un champ déplacé, puis lu ;
     - un champ déplacé, puis la variable utilisée entière ;
     - un champ déplacé sur une seule branche, puis lu après le `if` ;
     - un champ d’une variable déclarée hors d’une boucle, déplacé dedans ;
     - un champ dans un champ déplacé, lu.
   - Acceptés :
     - un autre champ de la variable, lu après le déplacement ;
     - le champ déplacé affecté, puis lu ;
     - un champ à deux structures de profondeur déplacé.
   - Toujours `E0209` : le champ d’un paramètre prêté, d’un élément de `for…of`, à travers un élément, d’une variable de module, et dans le corps d’une fonction fléchée.
2. **De bout en bout, sur les deux allocateurs.** Le champ déplacé et le reste de la structure sont chacun libérés une fois, sur chaque chemin, et rien ne reste vivant. Les chemins :
   - déplacé sur une seule branche ;
   - déplacé dans le corps d’une boucle, d’une structure faite dans le tour ;
   - un `return` anticipé ;
   - `?` ;
   - `break`.

   Le vérificateur de libération accepte chacun. L’abaissement, changé exprès pour libérer aussi le champ déplacé, est refusé par le vérificateur.

3. **Le test différentiel** gagne une instruction qui déplace un champ hors d’une structure faite dans le tour, sur une branche, et passe 2 000 graines. La même mutation qu’au critère 2 doit le faire échouer (constat M015).
4. **Le tableau de bord.**
   - `bench/data/dashboard.peras` prend le pays là où il le copiait une seconde fois, et donne le même résumé octet pour octet (`node scripts/bench-data.mjs --check`).
   - Son A/B par `node scripts/ab-data.mjs` est rapporté, pas une condition.

### Partie B

5. **Tests UI du vérificateur, dans les deux modes.**
   - Acceptés : chaque place où une valeur fonction peut aller.
   - Refusés :
     - une signature qui ne convient pas (`E0210`) ;
     - une fonction générique sans arguments de type ;
     - une fonction avec un paramètre `sink` ou `inout` ;
     - une fonction fléchée qui capture, comme valeur dans un module owned (`E0900`) ;
     - une fonction exportée avec un type fonction ;
     - `==` entre fonctions.
   - Le bogue du vérificateur : `xs.map(twice)` est accepté dans un module managed.
6. **De bout en bout, sur les deux allocateurs,** contre le même module construit comme un module managed, qui est du JavaScript. Les appels :
   - `apply(twice, x)` ;
   - une table de fonctions dans un tableau, appelées dans une boucle ;
   - une fonction dans un champ et dans une option ;
   - une fonction rendue par une fonction qui la choisit selon un argument ;
   - `xs.map(twice)` et `xs.sortBy(key)`.

   Les fonctions prennent et rendent des chaînes et des structures : les arguments sont prêtés et le résultat pris à travers un appel indirect. Rien ne reste vivant.

7. **Le test différentiel** gagne un aiguillage à travers un tableau de fonctions et passe 2 000 graines. Appeler exprès la fonction au mauvais indice doit le faire échouer.
8. **Le backend Rust et le vérificateur.** Un test unitaire tient le Rust écrit pour un appel indirect. Le vérificateur de libération prend le résultat d’un appel indirect comme possédé et ses arguments comme prêtés ; une mutation qui libère un argument à l’intérieur est refusée.
9. **Taille et coût.** Deux chiffres sont rapportés, pas des conditions :
   - la taille d’un module avec et sans une fonction utilisée comme valeur ;
   - le coût d’un appel à travers une valeur contre un appel direct, mesuré dans un même processus (constat M017).
10. **Docs**, en anglais et en français : la page du guide sur les fonctions, l’état (fermetures toujours prévues), `E0242`, `E0238`, `E0221` et `E0900`, et les constats F214 et F308.

## Registre

_Chaque partie est consignée ici à son arrivée._

**Les deux parties : arrivées ensemble**, comme le propriétaire l’a demandé, la partie B en vraies valeurs fonctions.

- **Ce qui a changé, partie A.**
  - La passe de propriété suit les places déplacées, une variable et les champs du chemin, comme elle suit les variables déplacées.
    - Un champ d’une locale, ou d’un paramètre `sink`, se déplace.
    - Le champ, ou ce qui le tient, utilisé ensuite est signalé (`E0204`), comme la structure utilisée en entier, avec un nouveau message pour une structure en partie déplacée.
    - Affecter le champ le rend.
    - L’élément d’un `for…of`, qu’une boucle prête, est distingué d’une locale à elle.
  - L’abaissement déplace les locales du champ, qui quittent l’ensemble des vivantes ; le vérificateur de libération accepte chaque chemin.
  - Le tableau de bord prend son pays au lieu de le copier une seconde fois.
- **Ce qui a changé, partie B.**
  - Le vérificateur :
    - écrit des types fonctions partout où un type s’écrit, dans les deux modes ;
    - type une fonction du module, non appelée, comme l’un d’eux, sauf si elle est générique ou prend un paramètre `sink` ou `inout` ;
    - donne une fonction par sa valeur à `map` et aux autres (F010 corrigé) ;
    - refuse `==` entre fonctions, et une fonction exportée qui en prend ou en rend une, même au fond d’une structure.
  - L’IR gagne trois formes :
    - `FnRef`, une fonction comme `i32` ;
    - `CallIndirect` ;
    - `CallManyIndirect`.

    Chaque passe qui distinguait un appel a été lue et les a reçues ; le compilateur a listé chaque `match` qui les manquait.

  - Les backends et le runtime :
    - WebAssembly a une table `funcref`, remplie une fois par un segment d’éléments, et `call_indirect` avec un type par signature ;
    - l’éditeur de liens garde la table du module et son segment, renumérotés ;
    - le backend Rust écrit un `match` sur la position ;
    - l’interpréteur appelle par position.
  - Une fonction fléchée utilisée comme valeur devient une fonction à elle, abaissée après les autres, ses paramètres prêtés. Un nom qu’elle ne déclare pas est une capture, refusée comme `closure`.
  - `xs.map(twice)` appelle `twice` directement : la valeur tient lieu de corps de la boucle.
- **Les tests, critère par critère.**
  1. `E0204-moved-field.peras` et `E0209-field-not-owned.peras` : chaque refus et chaque cas permis que le critère nomme.
  2. `functions.test.ts`, sur les deux allocateurs : un champ déplacé sur une branche, dans une boucle avec `break` et `continue`, avant un `return` anticipé ; rien ne reste vivant. Déplacer le champ sans quitter l’ensemble des vivantes, exprès, est refusé par le vérificateur de libération : `holds a block on one path and not on another`.
  3. Le test différentiel gagne un champ déplacé sur une branche ; il passe 2 000 graines, et la même mutation le fait échouer à la graine 0.
  4. Le tableau de bord donne le même résumé (`bench-data.mjs --check`). Son A/B, `node scripts/ab-data.mjs HEAD --before-source <le tableau de bord qui copie>`, 1 000 000 de lignes, 20 tours : 310,3 ms avant, 301,6 ms après, un rapport de 1,025 en médiane, de 0,788 à 1,446. Rapporté, pas une condition.
  5. `E0210-function-values.peras` et `.apeiron`, `E0238-function-export.peras`, `E0900-closure.peras`, et `function-values.peras`, dont l’IR est gardée. Le bogue du vérificateur est corrigé : `xs.map(twice)` est accepté dans un module managed.
  6. `functions.test.ts`, sur les deux allocateurs, contre le même source construit comme module managed :
     - `apply(twice, x)` ;
     - un tableau de fonctions appelées dans une boucle ;
     - une fonction dans un champ et dans une option ;
     - une fonction choisie par une fonction ;
     - `map`, `filter`, `reduce` et `sortBy` auxquels des fonctions sont données par leur nom ;
     - des chaînes et des structures données et reçues, une chaîne possédée prêtée.

     Les résultats sont les mêmes, et rien ne reste vivant.

  7. Le test différentiel gagne un aiguillage à travers un tableau de fonctions et une fonction choisie par un appel ; il passe 2 000 graines. Appeler exprès la fonction suivante de la table le fait échouer à la graine 11.
  8. `a_call_through_a_value_matches_on_the_table` tient le Rust écrit. `a_call_through_a_function_value_only_lends` tient le vérificateur de libération : un argument déplacé est refusé, et le test échoue sans cette règle.
  9. Une boucle qui appelle une fonction 20 000 000 de fois, dans un même processus, l’ordre alterné :
     - le module fait 143 octets, ou 173 avec la fonction comme valeur : 30 octets pour la table, son segment et un type ;
     - à travers la valeur, 77,7 ms contre 76,5 ms directement, un rapport de 1,025 en médiane, de 0,871 à 1,219.

     Rapportés, pas des conditions.

  10. Les pages du guide sur les fonctions et la mémoire, l’état, `E0204`, `E0209`, `E0238` et `E0900`, et les constats F214, F308 et F010, en anglais et en français.
- **Écarts par rapport au texte ci-dessus.**
  - Les deux parties arrivent en un lot, comme le propriétaire l’a demandé, là où le texte ci-dessus mettait A avant B.
  - Une fonction générique n’est jamais une valeur, même quand le type attendu donnerait ses arguments de type : les inférer là est laissé pour plus tard.
  - Une fonction avec un paramètre `sink` ou `inout`, utilisée comme valeur, est signalée comme une incompatibilité (`E0210`), pas avec un message à elle.
  - Appeler à travers un champ s’écrit `(s.handler)(x)` : `s.handler(x)` reste un appel de méthode.
  - Au critère 8, la mutation a été faite dans le test du vérificateur plutôt que dans l’abaissement. Le prêt est décidé par la passe de propriété : un abaissement changé seul ne déplace rien ; changé avec la passe de propriété, le vérificateur signale le déplacement d’abord.
  - L’A/B du critère 4 compare deux versions du tableau de bord avec un même compilateur. `ab-data.mjs` a gagné `--before-source` pour cela.
