# 0024 : Tous les types entiers compilés dans les modules owned

_Version anglaise, qui fait foi : [../../en/decisions/0024-integer-types.md](../../en/decisions/0024-integer-types.md)._

_État : proposée le 2026-11-16, avec ses critères enregistrés avant tout code ; étape 4 de la [décision 0021](0021-phases-4-and-5.md), constat F302._

## Contexte

Peras a huit types entiers : `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32` et `u64`. Le vérificateur connaît les huit, et un module managed les compile tous les huit en JavaScript. Un module owned ne compile que `i32` et `i64` ; les six autres sont refusés avec `E0900` (`integer-type`).

Ce que fait le code aujourd’hui (lu avant d’écrire ceci, après le constat M020) :

- **Le vérificateur** ne convertit jamais implicitement de l’un à l’autre. `-` sur un type non signé est refusé (`E0221`). Un décalage prend un compte du même type que la valeur. `toU8()` et les autres conversions sont des méthodes.
- **L’IR** a deux types entiers, `i32` et `i64`. Ses opérateurs sont signés : `/` et `%` tronquent vers zéro avec le signe du dividende, `>>` est arithmétique, et les comparaisons sont signées. Une conversion est `Convert(x)`, typée par son résultat, sans rien dire du signe de son opérande.
- **Les tableaux** rangent leurs éléments selon un `Elem`. `Elem::U8`, un octet lu comme un `i32` de 0 à 255, tient déjà les octets des chaînes.
- **Le chargeur** ne vérifie aucun scalaire venu de JavaScript : un nombre passé comme `i32` est réduit modulo 2³², comme le fait WebAssembly lui-même. Un `bool` est un `i32` en WebAssembly et un booléen en JavaScript : le chargeur distingue déjà deux types qui partagent une même représentation.
- **Un défaut trouvé en lisant.** `int_const` change un littéral `u64` au-dessus de `i64::MAX` en `i64::MIN`, et non en ses bits. Rien ne l’atteint aujourd’hui, puisque `u64` est refusé.
- **Un module managed** donne `i64` et `u64` comme `BigInt` et les autres comme nombres, et vérifie chaque résultat contre la plage de son type. Il sert de référence pour ce que donne chaque opération.

## Décision

1. **Pas de nouveau type dans l’IR : les opérations portent le signe.**
   - `i8`, `i16`, `u8`, `u16` et `u32` sont gardés dans un `i32`, et `u64` dans un `i64`.
   - Une valeur est toujours sous sa forme canonique : un `i8` ou un `i16` étendu par le signe, un `u8` ou un `u16` étendu par des zéros. Donc `==`, `!=` et les opérateurs bit à bit n’ont besoin de rien de nouveau.
   - L’IR gagne la division, le reste, le décalage à droite et les comparaisons non signés, et des conversions qui disent le signe de leur opérande. WebAssembly a une instruction pour chacun.
   - De nouveaux types dans l’IR auraient été l’autre voie. Chaque passe qui compare un type à `i32` ou `i64` aurait alors dû en apprendre six de plus, et une passe qui en oubliait un l’aurait maltraité. Avec des opérations qui portent le signe, une passe qui n’en sait rien voit des opérations `i32` et `i64` qu’elle traite déjà.
2. **La règle du dépassement est celle que suivent déjà `i32` et `i64`** ([décision 0011](0011-what-to-build-next.md)).
   - `+`, `-` et `*` arrêtent le programme dans un build debug quand le résultat sort de la plage de son type, et font le tour à sa largeur dans un build de production.
   - `wrappingAdd` et les autres font toujours le tour, et `checkedAdd` et les autres donnent `None`.
   - La division par zéro arrête le programme dans les deux builds.
3. **Les tableaux rangent chaque élément à sa propre largeur.**
   - `Array<u8>` utilise `Elem::U8`, qui existe déjà. `Array<i8>`, `Array<u16>` et `Array<i16>` reçoivent des éléments d’un et de deux octets, lus avec la bonne extension.
   - `Array<u32>` et `Array<u64>` partagent le rangement de `i32` et `i64`, et sont lus comme non signés à la frontière.
   - Un `PerasArray` gagne les sortes `i8`, `i16`, `u16`, `u32` et `u64` : `Int8Array`, `Int16Array`, `Uint16Array`, `Uint32Array` et `BigUint64Array`.
4. **À la frontière avec JavaScript.**
   - Tous les types sauf `i64` et `u64` sont des nombres, et ces deux-là sont des `BigInt`.
   - Une valeur donnée par JavaScript est ramenée à son type comme la range un tableau typé de ce type : 300 donné comme `u8` vaut 44, comme le tient `new Uint8Array([300])`. C’est ainsi qu’un `i32` est déjà pris.
   - Un `u32` ou un `u64` rendu n’est jamais négatif.
5. **Les tables et les ensembles** dont les clés sont de ces types, et les variables de module de ces types, suivent les mêmes règles. Une clé est élargie à 64 bits par l’extension de son type.
6. **Le défaut des littéraux est corrigé** avec le reste : un littéral `u64` garde ses bits.

## Non retenu

- **Vérifier les valeurs que donne JavaScript**, une `RangeError` pour 300 donné comme `u8`. Cela rendrait `u8` plus strict que ne l’est `i32` aujourd’hui. Si on le veut, cela devrait venir pour tous les types entiers à la fois, dans sa propre décision.
- **De nouveaux types dans l’IR,** pour la raison donnée au point 1.

## Critères, enregistrés avant tout code

L’étape n’arrive qu’avec tous ceux-ci, en plus des [critères de la décision 0021](0021-phases-4-and-5.md#critères-de-chaque-étape) :

1. **Contre un oracle qui n’est pas le compilateur.** Pour chacun des six types, un test en Rust calcule des opérations aléatoires avec l’arithmétique propre de Rust sur ce type et avec le module compilé, exécuté par `wasmi`. Il couvre 10 000 cas par type dans un build release, et moins en debug, chronométré sous une minute (constat M022).
   - Les opérations sont `+`, `-` et `*` sous leur forme qui fait le tour, `/`, `%`, `<<`, `>>`, les opérateurs bit à bit, les comparaisons, et les conversions depuis et vers chaque autre type numérique.
   - Les opérandes comprennent les cas limites : 0, 1, le minimum, le maximum et −1.
   - Les résultats doivent être égaux bit pour bit.
2. **Le dépassement.** Pour chaque type, une addition et une multiplication au-delà de son maximum arrêtent le programme dans un build debug et font le tour dans un build de production. De même pour une soustraction sous son minimum.
3. **De bout en bout, sur les deux allocateurs.**
   - Chaque type comme paramètre, résultat, champ d’une structure et élément d’un tableau, contre les mêmes appels en JavaScript.
   - Les valeurs données par JavaScript comprennent des nombres hors plage, qui sont ramenés au type.
   - Un `u32` au-dessus de 2³¹ et un `u64` au-dessus de 2⁶³ reviennent non négatifs.
   - Rien ne reste vivant.
4. **Le test différentiel** gagne des locaux `u8`, `u16`, `u32`, `u64`, `i8` et `i16` dans ses boucles, et tourne sur 2 000 graines. Deux changements faits exprès doivent chacun faire échouer un test : omettre le rétrécissement après une addition `u8`, et comparer deux `u32` comme signés.
5. **La documentation :** la référence des types, l’état et `E0900`, en anglais et en français.

## Registre

_Chaque partie est consignée ici à son arrivée._
