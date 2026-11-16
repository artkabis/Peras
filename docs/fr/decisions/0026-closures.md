# 0026 : Les fermetures dans les modules owned

_Version anglaise, qui fait foi : [../../en/decisions/0026-closures.md](../../en/decisions/0026-closures.md)._

_État : acceptée par le propriétaire le 2026-11-16, avec ses critères enregistrés avant tout code ; elle suit la [décision 0025](0025-moved-fields-and-function-values.md), dont elle lève le refus `closure`. Une première version, proposée le même jour, faisait de l’environnement une valeur possédée qui se déplace ; le propriétaire a demandé si ses régressions n’étaient pas trop élevées, et cette version les retire._

## Contexte

Ce que fait le code aujourd’hui, lu avant d’écrire ceci (constat M020), et ce que dit le compilateur.

```peras
fn apply(f: (i32) => i32, x: i32): i32 {
  return f(x);
}

export fn shifted(n: i32, k: i32): i32 {
  return apply((x) => x + k, n); // E0900 : fonctions fléchées qui capturent une variable, utilisées comme valeurs
}
```

- **Une valeur fonction est un `i32`** depuis la décision 0025 : sa position dans la table du module, appelée par `call_indirect`. Rien d’autre ne voyage avec elle, si bien qu’une fonction qui a besoin d’un `k` capturé n’a nulle part où le trouver.
- **Une valeur fonction se copie librement** : `const g = f;` garde `f` utilisable, et rien n’est libéré.
- **Une fonction fléchée donnée sur place** à `map` et aux autres est abaissée dans la boucle. Elle lit directement les locales qui l’entourent, et ne capture rien.
- **Une fonction fléchée utilisée comme valeur** devient une fonction à elle, abaissée après les autres dans un état à elle. Un nom de la fonction englobante y est inconnu, et c’est ainsi que le refus `closure` est trouvé.
- **Un module managed capture par référence**, comme JavaScript. Ci-dessous, `later(5)` donne 105, pas 6 :

  ```apeiron
  export fn later(n: i32): i32 {
    let k = 1;
    const f = (x: i32): i32 => x + k;
    k = 100;
    return apply(f, n);
  }
  ```

- **Le runtime refuse de libérer l’adresse 0** (`INVALID_RELEASE`), si bien qu’une valeur possédée tient toujours un vrai bloc.
- **L’en-tête d’un tampon** garde, à son décalage 12, la taille d’un élément et ses drapeaux (`DEEP`, `INLINE`, `MAP`, `STRKEY`). Sa capacité est au décalage 8.
- **L’IR n’a pas de réinterprétation des bits** entre flottants et entiers. WebAssembly en a une : `i64.reinterpret_f64` et les autres.
- **Le backend Rust ne compile aucun tampon** : une opération sur l’un d’eux y est `unreachable!()`. Il compile l’`i32` de la décision 0025.

## Décision

1. **Une valeur fonction porte un environnement.** Elle fait deux scalaires : sa position dans la table, un `i32`, et son environnement, une poignée `i32`.
2. **Un environnement est figé et partagé.** C’est un bloc d’emplacements `i64`, un par valeur capturée, écrits quand la fermeture est faite et jamais après.
   - Son en-tête compte qui le tient. Une copie de la valeur en ajoute un ; quitter une portée en retire un, et le dernier libère le bloc.
   - Une valeur fonction reste donc une valeur qui se copie librement, comme dans la décision 0025 : `const g = f;` garde `f`, et rien ne se déplace.
   - Rien de ce qu’il tient ne peut le retenir, puisqu’il ne tient que des nombres : aucun cycle ne peut le garder vivant.
3. **Une valeur qui ne capture rien a un environnement vide statique**, écrit une fois au démarrage du module sous le tas : une fonction nommée donnée comme valeur, ou une fonction fléchée qui ne capture rien. Ajouter à son compte ou en retirer ne fait rien, et il n’est jamais libéré, si bien qu’une telle valeur coûte ce qu’elle coûte dans la décision 0025 : aucune allocation.
4. **Ce qu’une fermeture capture, dans un module owned, ne peut pas changer tant qu’elle vit.**
   - Elle capture un `const`, ou un paramètre qui n’est pas `inout`.
   - Un `let` qu’elle capture est refusé, avec un nouveau code. Une copie faite quand la fermeture est faite est alors ce que lirait la référence de JavaScript, si bien que les deux modes donnent la même chose.
5. **Ce qu’elle capture est copié dans son environnement quand elle est faite.**
   - Nombres et booléens sont rangés sur 64 bits : un entier étendu selon son type, un flottant par ses bits. Pour cela, l’IR gagne les réinterprétations de WebAssembly.
   - Une capture d’une chaîne, d’un tableau, d’une table, d’un ensemble, d’une structure qui en tient, ou d’une valeur fonction est refusée pour l’instant (`E0900`, `closure-heap`). La tenir ferait posséder un tampon à l’environnement : c’est l’étape suivante.
6. **Chaque fonction de la table prend son environnement en dernier**, prêté, après ses arguments : les arguments sont évalués avant la valeur appelée, si bien que l’appel lit la position de sa fonction dans l’environnement sans une seconde locale par appel imbriqué.
   - Une fermeture lit chaque capture dans son emplacement.
   - Une fonction du module utilisée comme valeur reçoit un adaptateur dans la table, qui prend l’environnement et appelle la fonction. Ses appels directs ne changent pas.
   - `xs.map(twice)` reste un appel direct : la valeur n’est jamais faite.
7. **Un module managed ne change pas.** Ses fermetures sont celles de JavaScript.
8. **Le backend Rust ne perd rien.** Une valeur qui ne capture rien y compile comme dans la décision 0025 : son environnement est une constante, ajouter au compte et en retirer ne font rien. Une fermeture qui capture y est refusée, comme toute autre valeur sur le tas, puisque ce backend ne compile aucun tampon.

## Non retenu

- **Un environnement possédé par une seule valeur, qui se déplace**, la première version de cette décision : `const g = f;` aurait déplacé `f`, une allocation aurait été faite pour chaque fonction nommée donnée comme valeur, et le backend Rust aurait perdu toute valeur fonction. Trois régressions pour un gain : aucun compte à tenir.
- **Garder l’`i32` et mettre les environnements ailleurs** : dans une table du module, ou jamais libérés. C’est soit un second propriétaire à tenir en accord, soit une fuite.
- **Capturer par référence, comme JavaScript** : une variable mise en boîte, partagée par la fonction et la fermeture, qui peut la changer. C’est `Rc<RefCell<T>>`, le constat F310, encore à concevoir ; un environnement figé n’a besoin que du compte.
- **Copier implicitement les chaînes capturées** : Peras rend les copies explicites (`copy()`), et une fermeture qui copierait en secret serait le seul endroit où ce ne serait pas le cas.

## Critères, enregistrés avant tout code

L’étape n’arrive qu’avec tous ceux-ci, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape) :

1. **Tests UI du vérificateur :**
   - refusés :
     - un `let` capturé, avec le nouveau code ;
     - une chaîne, un tableau, une structure qui en tient, et une valeur fonction capturés (`E0900`, `closure-heap`) ;
   - acceptés : un `const` et un paramètre capturés ; `const g = f;` puis `f(1)` et `g(1)`.
2. **De bout en bout, sur les deux allocateurs,** contre le même source construit comme module managed, qui est du JavaScript. Les appels :
   - `makeAdder(k)` qui rend une fermeture ;
   - un tableau de fermetures sur un `const` de boucle, appelées après la boucle ;
   - une fermeture dans une structure et dans une option ;
   - une fermeture donnée à `apply`, et copiée avant d’être appelée ;
   - des fermetures sur `i8`, `u32`, `i64`, `u64`, `f32`, `f64` et `bool`, les bornes de chacun.

   Les résultats sont les mêmes, et rien ne reste vivant.

3. **Le compte tient.** Une fermeture copiée 100 fois dans un tableau, le tableau ensuite libéré, ne laisse rien de vivant ; une copie qui n’ajoute pas au compte, faite exprès dans l’abaissement, est prise par un test (un bloc libéré deux fois, ou libéré pendant qu’il est lu).
4. **Le test différentiel** gagne une fermeture faite dans une boucle sur les valeurs du tour et appelée à travers un tableau de fonctions, et passe 2 000 graines. Lire exprès le mauvais emplacement de l’environnement doit le faire échouer.
5. **Le vérificateur de libération** prouve chaque environnement libéré sur chaque chemin : fait et abandonné, copié dans un tableau, rendu, remplacé. Un environnement non libéré, exprès, est refusé.
6. **Les réinterprétations** sont testées contre `to_bits` et `from_bits` de Rust sur chaque borne de `f32` et `f64`, NaN et −0 compris, dans l’interpréteur et en WebAssembly.
7. **Aucune régression du backend Rust :** un module dont les valeurs fonctions ne capturent rien compile avec `--backend rust` et donne ce que donne le module WebAssembly, tenu par un test du texte produit.
8. **Mesures, rapportées et pas des conditions :**
   - un appel à travers une fermeture contre un appel direct, dans un même processus (constat M017) ;
   - l’appel à travers une fonction nommée donnée comme valeur, contre son temps de la décision 0025, dans un même processus ;
   - le tableau de bord, qui ne fait aucune valeur fonction, inchangé selon `ab-data.mjs` ;
   - la taille d’un module avec une fermeture.
9. **Docs,** en anglais et en français :
   - la page du guide sur les fonctions ;
   - l’état, avec les fermetures compilées pour les nombres et les booléens ;
   - `E0900` et le nouveau code ;
   - la référence de la CLI, pour le backend Rust (`--backend rust`) ;
   - la suite du constat F308.

## Registre

_Chaque partie est consignée ici à son arrivée._

- **2026-11-16, l’étape, en un lot.**
  - L’IR gagne `ExprKind::Env` (`Static`, `New`, `Zero`, `Load`) et `UnOp::Bits` ; un type fonction s’abaisse en une feuille `owned`, comme une chaîne. Chaque fonction de la table prend son environnement en dernier : les arguments sont évalués avant la valeur appelée, si bien que l’appel y lit la position de sa fonction avec une seule locale de travail.
  - Le runtime marque un environnement `SHARED` : `buf_clone` ajoute un à son compte, `buf_release` en retire un et libère le bloc à zéro ; un environnement statique, écrit sous le tas par un segment de données, a un compte de 0, que rien ne change. L’environnement vide d’un `None` est à l’adresse fixe `ZERO_ENV`, et les littéraux commencent après.
  - Un module dont tous les environnements sont statiques (`Module::static_envs_only`) n’a pas besoin du tas : un environnement y est la position de sa fonction, une copie la même position, et une libération ne fait rien. C’est ainsi que le backend Rust continue de compiler les valeurs de la décision 0025.
  - Le vérificateur note ce que capture chaque fonction fléchée, et refuse, dans un module owned, un `let` ou un paramètre `inout` capturé par une fonction utilisée comme valeur (`E0251`).
  - Critères :
    1. `E0251-closure-capture.peras` et `.apeiron`, `E0900-closure.peras` (`closure-heap` : une chaîne, un tableau, une structure qui en tient, une valeur fonction), et `closures.peras`, dont l’IR est gardée.
    2. `closures.test.ts`, sur les deux allocateurs, contre le même source construit comme module managed : `makeAdder`, des fermetures sur un `const` de boucle, une fermeture dans une structure et dans une option, donnée à `apply` et copiée, des fermetures sur les bornes de `i8`, `u32`, `i64`, `u64`, `f32`, `f64` et `bool`. Les résultats sont les mêmes, et rien ne reste vivant.
    3. `an_environment_copied_is_freed_once` : 100 copies dans un tableau, puis libérées, ne laissent rien de vivant dans l’interpréteur ni en WebAssembly. Une copie qui ne compte pas, exprès dans le `buf_clone` du runtime, fait arrêter l’appel par l’allocateur de débogage ; dans l’interpréteur, un `DoubleRelease`.
    4. Le test différentiel gagne des fermetures faites dans une boucle et appelées à travers un tableau ; il passe 2 000 graines. Lire l’emplacement 0 pour chaque emplacement, exprès dans le générateur WebAssembly, fait échouer les deux tests différentiels (la graine 28 entre autres). La même mutation dans l’abaissement passait : constat M026.
    5. Le vérificateur de libération refuse un environnement fait et jamais libéré, exprès dans l’abaissement : `%3 still holds a block when the function returns: it leaks`.
    6. `reinterpretations_match_rusts_bits` et `captured_numbers_come_back_bit_for_bit` : chaque borne de `f32` et `f64`, NaN avec une charge, NaN signalant et −0 compris, dans l’interpréteur et en WebAssembly.
    7. `static_environments_are_positions` tient le Rust produit ; et un module de valeurs qui ne capturent rien, construit avec `--backend rust` et avec le backend direct, a donné les mêmes 16 résultats dans Node. `bench-backends.mjs --check` passe.
    8. Mesures, après le commit de l’étape (`3a750e8`), sur un Intel Xeon à 2,10 GHz, 4 cœurs, TLSF, 21 tours de 20 000 000 d’appels dans un même processus, l’ordre alterné (`calls.mjs`, gardé hors du dépôt) :
       - à travers une fermeture, 77,9 ms contre 81,8 ms directement, un rapport de 0,941 en médiane, de 0,874 à 1,128 ;
       - une fonction nommée à travers une valeur, 75,7 ms contre 75,5 ms avec le compilateur de la décision 0025 (`1daaf30`), un rapport de 1,007, de 0,733 à 1,083 : aucun coût trouvé pour l’environnement statique ;
       - le module avec une fonction nommée comme valeur fait 224 octets contre 169 dans la décision 0025 : 55 octets pour l’adaptateur et le paramètre de l’environnement. Avec une fermeture, 3 444 octets : un environnement compté lie l’allocateur, comme toute valeur du tas ;
       - le tableau de bord, qui ne fait aucune valeur fonction, `node scripts/ab-data.mjs 1daaf30`, 1 000 000 de lignes, 20 tours : 344,8 ms avant, 353,1 ms après, un rapport avant / après de 1,011 en médiane, de 0,775 à 1,314, le même résumé.

       Rapportées, pas des conditions.

    9. La page du guide sur les fonctions, l’état, `E0900`, le nouveau `E0251` et l’index des erreurs (auquel manquait `E0250` aussi), la référence de la CLI, les constats F308, F011 et M026, la feuille de route et le journal des changements, en anglais et en français.
- **Écarts au texte ci-dessus.**
  - L’environnement est le dernier paramètre, pas le premier.
  - Une fermeture qui capture est refusée par le pilote du backend Rust comme toute valeur du tas, pas avec `E0900`.
  - La mutation du critère 4 est faite dans le générateur, pas dans l’abaissement (constat M026).
  - Le bogue F011 a été trouvé et corrigé en chemin : deux fonctions fléchées utilisées comme valeurs avaient le même nom.
