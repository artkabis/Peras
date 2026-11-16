# 0026 : Les fermetures dans les modules owned

_Version anglaise, qui fait foi : [../../en/decisions/0026-closures.md](../../en/decisions/0026-closures.md)._

_État : proposée le 2026-11-16, avec ses critères enregistrés avant tout code ; elle suit la [décision 0025](0025-moved-fields-and-function-values.md), dont elle lève le refus `closure`._

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

- **Le runtime refuse de libérer l’adresse 0** (`INVALID_RELEASE`), si bien qu’une valeur possédée tient toujours un vrai bloc. Une `Option<String>` vide tient un tampon vide, fait pour elle.
- **L’IR n’a pas de réinterprétation des bits** entre flottants et entiers. WebAssembly en a une : `i64.reinterpret_f64` et les autres.
- **Le backend Rust ne compile aucun tampon** : une opération sur l’un d’eux y est `unreachable!()`.

## Décision

1. **Une valeur fonction porte un environnement.** Elle fait deux scalaires : sa position dans la table, un `i32`, et son environnement, un tampon possédé de `i64`, un emplacement par valeur capturée.
   - Une fonction du module, ou une fonction fléchée qui ne capture rien, a un environnement vide, fait là où la valeur est faite.
   - Dans un module owned, une valeur d’un type fonction se déplace donc comme une chaîne :
     - `const g = f;` déplace `f` ;
     - un argument est prêté ;
     - `f.copy()` copie l’environnement ;
     - quitter sa portée le libère.
2. **Ce qu’une fermeture capture, dans un module owned, ne peut pas changer tant qu’elle vit.**
   - Elle capture un `const`, ou un paramètre qui n’est pas `inout`.
   - Un `let` qu’elle capture est refusé, avec un nouveau code. Une copie faite quand la fermeture est faite est alors ce que lirait la référence de JavaScript, si bien que les deux modes donnent la même chose.
3. **Ce qu’elle capture est copié dans son environnement quand elle est faite.**
   - Nombres et booléens sont rangés sur 64 bits : un entier étendu selon son type, un flottant par ses bits. Pour cela, l’IR gagne les réinterprétations de WebAssembly.
   - Une capture d’une chaîne, d’un tableau, d’une table, d’un ensemble ou d’une structure qui en tient est refusée pour l’instant (`E0900`, `closure-heap`). C’est l’étape suivante : la déplacer dans l’environnement, qui posséderait alors des tampons.
   - Une valeur fonction capturée est refusée aussi, pour la même raison : elle tient un tampon.
4. **Chaque fonction de la table prend son environnement en premier**, prêté.
   - Une fermeture lit chaque capture dans son emplacement.
   - Une fonction du module utilisée comme valeur reçoit un adaptateur dans la table, qui prend l’environnement et appelle la fonction. Ses appels directs ne changent pas.
   - `xs.map(twice)` reste un appel direct : la valeur n’est jamais faite.
5. **Un module managed ne change pas.** Ses fermetures sont celles de JavaScript.
6. **Le backend Rust** refuse une valeur fonction avec `E0900`, puisqu’il ne compile aucun tampon, là où il compilait l’`i32` de la décision 0025. C’est écrit comme une régression de ce backend, pas caché.

## Non retenu

- **Garder l’`i32` et mettre les environnements ailleurs** : dans une table du module, ou jamais libérés. C’est soit un second propriétaire à tenir en accord, soit une fuite.
- **Capturer par référence, comme JavaScript** : une variable mise en boîte, partagée par la fonction et la fermeture. Partager demande un comptage de références, c’est-à-dire `Rc<T>`, le constat F310, encore à concevoir.
- **Un environnement vide statique pour les fonctions qui ne capturent rien** : une valeur possédée doit être un bloc que le runtime peut libérer. Un bloc prêté ferait deux sortes de valeurs fonctions, qu’un même type ne peut pas distinguer.
- **Copier implicitement les chaînes capturées** : Peras rend les copies explicites (`copy()`), et une fermeture qui copierait en secret serait le seul endroit où ce ne serait pas le cas.

## Critères, enregistrés avant tout code

L’étape n’arrive qu’avec tous ceux-ci, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape) :

1. **Tests UI du vérificateur :**
   - refusés :
     - un `let` capturé, avec le nouveau code ;
     - une chaîne, un tableau, une structure qui en tient, et une valeur fonction capturés (`E0900`, `closure-heap`) ;
   - acceptés : un `const` et un paramètre capturés ;
   - dans un module owned, une valeur fonction se déplace :
     - `const g = f;` puis `f(1)` est `E0204` ;
     - `f.copy()` garde les deux.
2. **De bout en bout, sur les deux allocateurs,** contre le même source construit comme module managed, qui est du JavaScript. Les appels :
   - `makeAdder(k)` qui rend une fermeture ;
   - un tableau de fermetures sur un `const` de boucle, appelées après la boucle ;
   - une fermeture dans une structure et dans une option ;
   - une fermeture donnée à `apply` ;
   - `f.copy()` ;
   - des fermetures sur `i8`, `u32`, `i64`, `u64`, `f32`, `f64` et `bool`, les bornes de chacun.

   Les résultats sont les mêmes, et rien ne reste vivant.

3. **Le test différentiel** gagne une fermeture faite dans une boucle sur les valeurs du tour et appelée à travers un tableau de fonctions, et passe 2 000 graines. Lire exprès le mauvais emplacement de l’environnement doit le faire échouer.
4. **Le vérificateur de libération** prouve chaque environnement libéré une fois : fait et abandonné, déplacé dans un tableau, rendu, remplacé. Un environnement non libéré, exprès, est refusé.
5. **Les réinterprétations** sont testées contre `to_bits` et `from_bits` de Rust sur chaque borne de `f32` et `f64`, NaN et −0 compris, dans l’interpréteur et en WebAssembly.
6. **Mesures, rapportées et pas des conditions :**
   - un appel à travers une fermeture contre un appel direct, dans un même processus (constat M017) ;
   - le tableau de bord, qui ne fait aucune valeur fonction, inchangé selon `ab-data.mjs` ;
   - la taille d’un module avec une fermeture.

   **Un chiffre est une condition :** le test de la décision 0025 qui appelle une fonction nommée à travers une valeur 20 000 000 de fois doit rester sous 1,5 fois son temps de la décision 0025, mesuré dans un même processus. Chaque valeur fonction fait désormais un environnement.

7. **Docs,** en anglais et en français :
   - la page du guide sur les fonctions ;
   - l’état, avec les fermetures compilées pour les nombres et les booléens ;
   - `E0900` et le nouveau code ;
   - la référence de la CLI, pour le backend Rust (`--backend rust`) ;
   - la suite du constat F308.

## Registre

_Chaque partie est consignée ici à son arrivée._
