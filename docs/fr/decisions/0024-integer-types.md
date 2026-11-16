# 0024 : Tous les types entiers compilés dans les modules owned

_Version anglaise, qui fait foi : [../../en/decisions/0024-integer-types.md](../../en/decisions/0024-integer-types.md)._

_État : proposée le 2026-11-16, avec ses critères enregistrés avant tout code ; acceptée par le propriétaire le même jour ; construite, ses critères tenus (voir le [Registre](#registre)) ; étape 4 de la [décision 0021](0021-phases-4-and-5.md), constat F302._

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

**Tous les types entiers dans les modules owned : arrivé.**

- **Ce qui a changé.**
  - L’IR gagne `convert_u`, une conversion qui lit ou fait un entier non signé, et les non signés `/u`, `%u`, `>>u`, `<u`, `<=u`, `>u` et `>=u` ; WebAssembly a une instruction pour chacun, le backend Rust passe par `u32` ou `u64`, et l’interpréteur les calcule comme Rust. Le vérificateur de libération contrôle leurs types.
  - L’abaissement, `lower/ints.rs`, garde chaque valeur canonique : après `+`, `-`, `*`, `<<`, `-x` et `~x`, un type étroit est masqué ou étendu par le signe. Avec les contrôles de dépassement, un type étroit compare le résultat exact à son intervalle, `u32` et `u64` par une comparaison non signée ou une division ; `MIN / -1` d’un `i8` ou d’un `i16` arrête le programme dans tous les builds, comme pour `i32`. Une conversion d’un flottant vers un type étroit sature vers un `i32`, puis vers le type.
  - Les tableaux rangent chaque élément à sa largeur : `Elem` gagne `I8`, `I16`, `U16`, `U32` et `U64`, chargés avec la bonne extension. Les `arr_sort` et `arr_sort_index` du runtime prennent les sortes non signées et les éléments d’un ou deux octets, élargis pour être triés, et `buf_equal` les éléments de deux octets.
  - Le chargeur enveloppe chaque valeur que JavaScript donne à son type, dans un paramètre, un champ, un tableau, une table ou un ensemble, rend `u32` et `u64` positifs ou nuls, et un `PerasArray` a les sortes `i8`, `i16`, `u16`, `u32` et `u64`. Une clé de table ou d’ensemble est élargie par l’extension de son type, un `u32` par des zéros.
  - `u64` est écrit par `fmt_uint`, nouveau dans le runtime ; un littéral `u64` au-delà de `i64::MAX` garde ses bits.
- **Les tests, critère par critère.**
  1. `crates/perasc-driver/tests/integers.rs` : pour chacun des six types, un module de 29 fonctions, `+`, `-` et `*` qui rebouclent, `/`, `%`, `<<`, `>>`, `&`, `|`, `^`, `min`, `max`, les cinq comparaisons, les conversions vers chaque type numérique et depuis `f32` et `f64`, contre l’arithmétique de Rust sur ce type, bit à bit : 10 000 cas par type en release, 400 en debug, moins d’une seconde dans les deux. Les bornes sont tirées une fois sur quatre ; les flottants comprennent NaN, les infinis et des valeurs hors de l’intervalle du type.
  2. `overflow.rs` : pour chaque type, `+` et `*` au-delà du maximum et `-` sous le minimum arrêtent un build de debug et rebouclent en production ; `MIN / -1` arrête les deux.
  3. `integers.test.ts`, sur les deux allocateurs : chaque type comme paramètre, résultat, champ, élément de tableau, tableau `inout`, clé de table, élément d’ensemble et variable de module, contre les mêmes appels écrits en JavaScript, avec des nombres hors intervalle donnés ; `u32` au-delà de 2³¹ et `u64` au-delà de 2⁶³ reviennent positifs ; les `PerasArray` de chaque sorte se voient comme leurs tableaux typés ; rien ne reste vivant.
  4. Le test différentiel gagne des locales `u8`, `u16`, `u32`, `u64`, `i8` et `i16` et un `Array<i16>` trié court et au-delà de 64 éléments ; il passe 2 000 graines. Retirer le rétrécissement après une addition `u8` fait échouer `overflow.rs` (`u8 add wrapped`) ; comparer des `u32` comme signés fait échouer l’oracle dès ses premiers cas. L’oracle du test différentiel est l’interpréteur de l’IR : il voit une optimisation qui traite mal ces opérations, pas un abaissement qui le fait, que l’oracle voit.
  5. La référence des types, l’état, `E0900`, `E0249` et la référence des méthodes, en anglais et en français.
- **Trouvé en chemin**, chacun corrigé avec un test qui échoue sans le correctif : le vectoriseur lisait un tableau d’octets par voies de quatre (F007) ; `is_simple` laissait une division non signée sortir d’une boucle qui ne tourne pas (F008) ; `reverse()` mélangeait les octets d’éléments de deux octets (F009).
- **Écarts par rapport au texte ci-dessus.**
  - Dans un tableau de structures, un champ de ces types prend les quatre octets d’un `i32` (ou les huit d’un `i64`) : seul un `Array<T>` de l’un d’eux est rangé à sa largeur. Le chargeur enveloppe un tel champ tout de même.
  - `sort()`, `includes`, `indexOf` et les clés de `sortBy` ne prenaient que `i32`, `i64` et les flottants, dans les deux modes (F109) ; ils prennent maintenant tous les types entiers, un changement du vérificateur que le texte ci-dessus ne nommait pas.
  - Le backend Rust écrit ces opérations en passant par `u32` et `u64`, ce qu’un test unitaire vérifie dans le texte qu’il écrit (`unsigned_operations_go_through_unsigned_types`) ; un build par `--backend rust` n’a pas été exécuté de bout en bout.
