# 0005 : Les tables et les ensembles : dans l’ordre d’insertion, hachés avec une clé secrète

_Version anglaise, qui fait foi : [../../en/decisions/0005-maps-and-sets.md](../../en/decisions/0005-maps-and-sets.md)._

## Contexte

La phase 3 de la [feuille de route](../reference/roadmap.md#phase-3--une-bibliothèque-standard) apporte `Map<K, V>` et `Set<T>`. Les programmes web comptent des mots, indexent des enregistrements par leur identifiant, et retiennent ce qu’ils ont déjà vu. En JavaScript, `Map` et `Set` rendent leurs entrées dans l’ordre où elles ont été insérées, et un développeur TypeScript s’y attend.

Une table de hachage répond à trois questions, et chacune a un coût :

- **Dans quel ordre rend-elle ses entrées ?** L’ordre d’insertion, ou celui des cases, qui dépend du hachage.
- **Avec quel hachage ?** Un rapide, ou un qui résiste à des clés choisies pour entrer en collision.
- **Que tient-elle, et combien de temps ?** Peras a des propriétaires, des prêts et des déplacements ; une table doit suivre ces règles.

## Ce que d’autres ont fait

- **L’ordre d’insertion.** Le dictionnaire compact de CPython (Raymond Hettinger, 2012 ; CPython 3.6, garanti depuis 3.7) range ses entrées dans un tableau dense, dans l’ordre d’insertion, et les retrouve par un index creux de petits entiers. `Map` et `Set` en JavaScript rendent aussi l’ordre d’insertion, par spécification.
- **Les tables suisses.** Le `flat_hash_map` d’Abseil (Benzaquen, Evlogimenos, Kulukundis et Perepelitsa, CppCon 2017) et le `hashbrown` de Rust gardent un octet de contrôle par case, comparés seize à la fois avec le SIMD, et les entrées dans les cases : aucune indirection, et aucun ordre.
- **L’inondation de hachage.** Crosby et Wallach (_Denial of Service via Algorithmic Complexity Attacks_, USENIX Security 2003) ont montré que des clés choisies pour entrer en collision changent chaque opération en parcours. Klink et Wälde (28C3, 2011) l’ont fait aux cadriciels web de l’époque avec les paramètres d’une seule requête (CVE-2011-4885 pour PHP, entre autres). Aumasson et Bernstein (_SipHash: a fast short-input PRF_, INDOCRYPT 2012 ; _Hash-flooding DoS reloaded_, 29C3, 2012) ont trouvé dans MurmurHash et CityHash des collisions qui ne dépendent pas de leur graine : une graine secrète ne les protégeait pas. SipHash est une fonction à clé avec une preuve de sécurité. Python l’utilise depuis la version 3.4 (PEP 456), et Rust utilise SipHash-1-3 avec des clés tirées au hasard.

## Mesures

Avant de décider, deux dispositions et trois hachages ont été écrits en Rust, compilés en WebAssembly avec le SIMD, les instructions qu’utiliserait le runtime de Peras, et exécutés dans Chromium à côté de la `Map` de JavaScript. Chaque variante a donné exactement les mêmes résultats que `Map`. Les prototypes ne sont pas versionnés : ils ont été écrits pour décider.

- **Ordonnée** est la disposition de CPython : un index d’entiers de 4 octets rempli aux deux tiers au plus, avec sondage linéaire, et un tableau dense d’entrées (hachage, clé, valeur) dans l’ordre d’insertion.
- **Suisse** est celle de hashbrown : un octet de contrôle par case, 16 comparés d’un coup, et les entrées dans les cases.
- **SipHash-1-3** a une clé de 128 bits. **Multiplicatif** a une clé aussi, fait de multiplications et de décalages, comme les hachages rapides. **FxHash** n’a pas de clé, comme dans le compilateur Rust.

**1 million de clés entières** (insérer, retrouver chacune, chercher un million qui n’y sont pas, en supprimer la moitié), médiane de 7 exécutions, en ms :

| Disposition et hachage  | Clés aléatoires | Identifiants, pas de 4096 |
| ----------------------- | --------------: | ------------------------: |
| `Map` de JavaScript     |       673 – 840 |                       601 |
| Ordonnée, SipHash-1-3   |       249 – 266 |                       271 |
| Ordonnée, multiplicatif |       165 – 191 |                       181 |
| Ordonnée, FxHash        |       159 – 180 |                **18 182** |
| Suisse, SipHash-1-3     |       181 – 190 |                       182 |
| Suisse, multiplicatif   |       112 – 124 |                       119 |
| Suisse, FxHash          |         86 – 90 |                       197 |

**Compter les mots d’un texte de 1 Mo** (19 627 mots différents), en ms : `Map` et `split` de JavaScript, 21,6 à 29,1 ; ordonnée avec SipHash-1-3, 8,3 à 11,9 ; suisse avec SipHash-1-3, 7,5 à 7,6 ; suisse, multiplicatif, 6,2 à 6,3. Les chiffres du module ne comptent pas l’entrée du texte, 0,7 ms par million de caractères depuis la [décision 0004](0004-strings-without-allocation.md).

**La mémoire** tenue pour des clés et des valeurs `i32`, les capacités des tableaux : ordonnée, 25 à 31 octets par clé ; suisse, 12 à 19.

Quatre faits ressortent de ces mesures :

1. **Un hachage sans clé s’effondre sur des données réelles.** FxHash avec sondage linéaire a mis 18 secondes sur des identifiants de 4096 en 4096, cent fois plus longtemps, car ses bits de poids faible ne variaient pas. Une première série de ces mesures, avec des clés qu’un générateur défectueux avait rendues répétitives, avait montré le même effondrement.
2. **L’ordre d’insertion coûte 1,4 à 1,5 fois sur un million d’entiers**, et 10 à 20 % sur les mots, qui tiennent dans le cache. Ce coût est celui de l’index : une clé trouvée se lit à deux endroits. Garder le hachage dans l’index, pour ne jamais suivre une case qui ne correspond pas, a été mesuré aussi : aucun gain (263 contre 266 ms).
3. **SipHash-1-3 coûte environ 40 % de plus qu’un hachage multiplicatif.**
4. **Chaque variante est plus rapide que la `Map` de JavaScript** : la plus lente, ordonnée avec SipHash-1-3, est 2,2 à 3,2 fois plus rapide sur les entiers et 2,4 à 2,6 fois sur les mots. C’est la `Map` de JavaScript, telle qu’on l’utilise habituellement, pas une table écrite à la main sur des tableaux typés, comme le ferait un expert pour des entiers : le [banc d’essai dans le navigateur](../../../bench/README.fr.md#banc-dessai-dans-le-navigateur-benchbrowser) a montré que de tels jumeaux réduisent chaque écart ; ces rapports sont donc un plafond, à mesurer de nouveau face à la table d’un expert avant d’être cités.

## Décision

1. **L’ordre d’insertion, dans la disposition de CPython.** Une `Map` ou un `Set` rend ses entrées dans l’ordre où elles ont été insérées, comme en JavaScript. Supprimer une entrée laisse un trou dans les entrées, retiré quand la table grandit. Parcourir lit le tableau dense et saute les trous.
2. **SipHash-1-3, avec une clé secrète tirée au chargement du module.** Le chargeur tire 128 bits avec `crypto.getRandomValues` pour chaque instance du module. Comme l’ordre des entrées ne dépend pas du hachage, la clé ne se voit jamais dans ce qu’un programme calcule ou affiche : un même programme donne le même résultat à chaque exécution, et des clés choisies pour entrer en collision ne coûtent pas plus que les autres. Chaque entrée garde 32 bits de son hachage, comparés avant la clé, et réutilisés quand la table grandit.
3. **Les clés : entiers, booléens et `String` d’abord.** Un flottant n’est pas encore une clé : JavaScript compare les clés avec SameValueZero, où `NaN` est égal à lui-même et `-0` à `+0`, et cette règle sera décidée avec un usage qui la demande. Les structures et les énumérations viendront ensuite, avec une interface qui dit comment les hacher et quand deux sont égales.
4. **La table possède ce qu’elle tient.** `m.set(k, v)` prend sa clé et sa valeur, comme `push`. `m.get(k)` prête sa clé et donne une `Option<V>`, lue comme on lit un élément de tableau. `m.has(k)`, `m.delete(k)` (qui dit si la clé y était), `m.size` et `m.clear()` la complètent, sous les noms de JavaScript. `for (const entry of m)` prête chaque entrée comme un tuple `(clé, valeur)` ; changer la table dans la boucle est refusé, comme pour un tableau. `Set<T>` est la même table sans valeurs : `add`, `has`, `delete`, `size`, `for…of`. Une table se crée avec `Map.new()`, ses types écrits sur sa variable.
5. **À la frontière, une copie.** Une fonction exportée prend et donne une `Map` ou un `Set` de JavaScript, copiés dans l’ordre d’insertion, dans les deux sens ; TypeScript voit `Map<string, number>`. Une table qui reste dans la mémoire du module, comme [`PerasArray`](../guide/javascript.md) pour les tableaux, attend une mesure qui la demande.
6. **Vérifié comme le reste.** Les tables sont des fonctions du runtime, écrites une fois pour les deux allocateurs, avec un modèle en Rust pour l’interpréteur. Des tests exécutent des suites aléatoires de `set`, `delete`, `get` et de parcours des deux côtés, face à la `Map` de JavaScript, entrée par entrée et dans l’ordre, et sont cassés exprès pour montrer qu’ils détectent une table fausse.

```peras planned
export fn counts(text: String): Map<String, i32> {
  let seen: Map<String, i32> = Map.new();
  for (const word of text.split(" ")) {
    if (word.length > 0) {
      seen.set(word.copy(), seen.get(word).unwrapOr(0) + 1);
    }
  }
  return seen;
}
```

## Écarté pour l’instant

- **Les tables suisses.** Elles sont 1,4 à 1,5 fois plus rapides sur une grande table et utilisent moitié moins de mémoire pour de petites entrées, mais leur ordre dépend du hachage, donc de la clé secrète : un programme rendrait ses entrées dans un ordre différent à chaque exécution. Le compilateur pourra plus tard donner la disposition suisse à une table jamais parcourue ni passée à JavaScript, où l’ordre ne peut pas se voir. Il faut pour cela savoir où va chaque table, et le gain mesuré dira quand cela vaut la peine.
- **Un hachage multiplicatif par défaut.** Il est 40 % plus rapide, mais de tels hachages ont eu des collisions qu’aucune graine n’empêche ; les clés viennent du réseau aussi souvent que du programme. Un hachage plus rapide pour les clés dont le compilateur prouve qu’elles viennent du programme lui-même pourra suivre, mesuré.
- **FxHash, et tout hachage sans clé.** Mesuré : cent fois plus lent sur des identifiants ordinaires.

## Statut

**La première partie est faite.** `Map<K, V>` et `Set<T>` sont compilés pour des clés entières, booléennes et chaînes, et des valeurs nombres et booléens :

- La table est celle du runtime, en WebAssembly, écrite une fois pour les deux allocateurs : un en-tête de 32 octets, des entrées de 24 octets dans l’ordre d’insertion, et un index de cases qui vaut 0 pour vide, −1 pour une entrée retirée, `e + 1` sinon. SipHash-1-3 est vérifié sur le vecteur de son article.
- Le chargeur tire la clé de 128 bits avec `crypto.getRandomValues` pour un module qui a des tables, et seulement alors.
- Les tests font les mêmes opérations aléatoires de trois façons, contre `HashMap` et `HashSet` de Rust : compilées en WebAssembly sur l’allocateur de débogage, dans l’interpréteur de l’IR, et dans Chromium contre `Map` et `Set` de JavaScript sur les deux allocateurs, sans bloc laissé vivant. Retirer l’écriture d’une entrée retirée, ou mal élargir une clé `bool`, les fait échouer.

Restent à venir, dans cet ordre : `for…of` dans l’ordre d’insertion, les valeurs qui possèdent des tampons, les tables dans les structures, puis la copie à la frontière JavaScript.

## Conséquences

- La ligne des collections de la [feuille de route](../reference/roadmap.md#phase-3--une-bibliothèque-standard) renvoie ici ; `Map` et `Set` viennent ensuite, pour les clés entières, booléennes et chaînes, puis `Deque`.
- Le chargeur tire la clé une fois par instance : un appel à `crypto.getRandomValues`, disponible dans tous les navigateurs, dans Node et dans les workers.
