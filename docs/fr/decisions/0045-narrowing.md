# 0045 : Restreindre une option à travers `&&`, `||` et `!`

_Version anglaise : [../../en/decisions/0045-narrowing.md](../../en/decisions/0045-narrowing.md)._

_Statut : protocole enregistré le 2026-11-24, avant tout prototype et toute mesure, comme le demande la règle du projet. Demandée par le propriétaire le 2026-11-24, après que la [décision 0044](0044-pools.md) a rencontré le constat F118 ; elle vient avant un amendement de la 0044 qui mesure un pool abaissé par le compilateur lui-même. Selon la règle du propriétaire du 2026-11-17, l’option choisie doit être une amélioration, jamais une régression ; si aucune ne l’est, la décision le dit et rien n’est intégré._

## Contexte

Ce que fait le vérificateur aujourd’hui, lu avant d’écrire ceci (constat M020) :

- **Une option est restreinte par deux formes de `if`** (décision 0028, option 2e ; constat F115), dans `crates/perasc-check/src/stmt.rs` : dans le bloc de `if (x.isSome())`, `x` est sa valeur (`some_test`) ; après `if (x.isNone()) { … }` dont le bloc sort, `x` est sa valeur jusqu’à la fin du bloc qui l’entoure (`narrowing`). `x` est une `const`, un paramètre non `inout`, ou un champ de l’un d’eux, `child.parent` ; un `let` n’est pas restreint.
- **Une lecture restreinte est marquée, pas récrite** : le vérificateur note chaque nom ou champ lu comme sa valeur (`Analysis::narrowed`), et l’abaissement possédé y lit la partie valeur de l’option (`crates/perasc-ir/src/lower/values.rs`, `lower.rs`) ; dans un module managé, une option est sa valeur ou `undefined`, et une lecture restreinte est le même JavaScript.
- **`&&` et `||` typent chaque côté comme un `bool`, et ne restreignent rien** (`binary` dans `crates/perasc-check/src/expr.rs`) ; `!` non plus, ni un `else`.
- **Ce que cela coûte, constat F118** : écrits comme en TypeScript, les cinq programmes de ce protocole rencontrent neuf erreurs aujourd’hui (`perasc check`, au commit `b16e79c`) : `E0210` et `E0221`, une option là où sa valeur est attendue. Le correctif est de séparer la condition en `if` imbriqués, ou de la retourner.

Ce que font les autres : TypeScript restreint par le flot de contrôle, à travers `&&`, `||`, `!`, `else`, les retours anticipés et `while` ; les « smart casts » de Kotlin à travers `&&` et `||` ; Swift demande `if let a, let b` et `guard let` ; Rust `if let Some(p) = p && …`, les chaînes de `let`, stables depuis 2024.

## Options

- **n0. Comme aujourd’hui** : les deux formes de `if`. La référence.
- **n1. Les chaînes** : une condition qui est une chaîne de `&&` restreint chaque option testée `x.isSome()` par l’un de ses opérandes, dans les opérandes qui le suivent et dans le bloc qu’elle garde ; une chaîne de `||` restreint chaque option testée `x.isNone()`, dans les opérandes qui le suivent, et après un `if` sans `else` dont le bloc sort. Ces deux formes seulement, lues sur l’arbre.
- **n2. Les faits**, comme TypeScript : chaque condition prouve des faits quand elle est vraie et quand elle est fausse ; `x.isSome()` prouve `x` quand elle est vraie, `x.isNone()` quand elle est fausse, `!c` échange les deux, `a && b` est vraie quand les deux le sont et fausse quand les faits des deux faussetés s’accordent, `a || b` l’inverse. L’opérande droit de `&&` est vérifié avec les faits vrais du gauche, celui de `||` avec ses faits faux ; le bloc d’un `if` avec ses faits vrais, son `else` avec ses faits faux ; après un `if` dont le bloc sort, ses faits faux, et après un `if` dont le `else` sort, ses faits vrais ; le corps d’un `while` avec ses faits vrais.

Les deux gardent ce qui est restreint aujourd’hui, les `const`, les paramètres non `inout` et leurs champs, et la même marque pour l’abaissement : pas de nouvel abaissement, pas de nouveau runtime.

## Protocole, enregistré avant tout prototype

n1 et n2 sont prototypées derrière `PERAS_PROTO_NARROW=n1|n2` ; sans elle, le compilateur est celui d’aujourd’hui, n0. Chaque module mesuré est construit pour la production, `--allocator tlsf` (constat M029), par une construction release de `perasc`.

### Avant toute mesure : les définitions testées

Les cinq programmes sont écrits comme le ferait un utilisateur venant de TypeScript (`bench/narrow/*.peras`), puis comme n0 le demande (`bench/narrow/n0/*.peras`), la condition séparée ou retournée et rien d’autre de changé ; chaque forme n0 se construit, donne la même chose deux fois sur une instance, et la même chose que son module managé (testé le 2026-11-24 au commit `b16e79c` : N1 343772, N2 -372340, N3 40781981, N4 339126400, N5 3012980, chacun pour 100 000).

### Les programmes

- **N1, les parents** (`links.peras`) : une remontée des parents arrêtée par `if (p.isNone() || !alive[p]) { break; }`.
- **N2, l’attraction** (`pull.peras`) : `if (p.isSome() && alive[p]) { sum += xs[p] - xs[i]; }`.
- **N3, les deux présentes** (`both.peras`) : `if (a.isNone() || b.isNone()) { return -1; } return a + b;`.
- **N4, le décompte** (`tally.peras`) : `if (x.isNone()) { … } else { total += x; }`, et `if (!x.isSome()) { continue; }`.
- **N5, un champ** (`weigh.peras`) : `if (c.parent.isSome() && alive[c.parent]) { … }`.

### Les grandeurs

1. **Vitesse** : le module construit depuis la forme TypeScript sous n1 et sous n2 contre le module construit depuis la forme n0 sous n0, dans un seul processus, l’ordre alterné à chaque tour, 21 tours, la médiane des rapports et leur étendue (constat M017) ; chacun appelé deux fois sur une instance et sa seconde réponse vérifiée (constat M035). Un programme qu’une option refuse n’est pas chronométré sous elle.
2. **Charge CPU** : le temps pendant lequel un cœur est occupé, `process.cpuUsage`, sur les mêmes tours.
3. **Mémoire et octets** : la mémoire de chaque module à la fin et sa taille, brute et gzip, un module identique à celui de la référence compté à 1,000 (constat M037) ; `check-regression` contre le commit de ce protocole, le temps du compilateur et sa taille.
4. **Facilité d’écriture** : les erreurs que le compilateur rapporte pour chaque forme TypeScript sous chaque option, comptées par `perasc check --json`, et les lignes, lignes vides et commentaires à part, de la forme qui se construit : la forme TypeScript là où elle se construit, la forme n0 sinon.
5. **Sûreté** : des programmes qui doivent rester refusés sous toute option, chacun un test de `crates/perasc-check` : `x` lu dans le bloc de `if (x.isSome() || c)`, après `if (x.isNone() && c) { return; }`, dans le `else` de `if (x.isSome() && c)`, dans l’opérande droit de `x.isSome() || …` ; un `let` ou un `inout` testé dans une chaîne ; les formes que n2 accepte, `if (!(x.isSome() && c)) { return; }` parmi elles, vérifiées pour donner ce que donne la forme n0.

### Justesse, avant que tout chiffre compte

Chaque option passe `cargo test --workspace` avec sa variable posée, les tests de bout en bout, et le test différentiel avec `PERAS_DIFFERENTIAL_SEEDS=2000`, son générateur étendu avec des conditions qui testent une option à travers `&&`, `||` et `!` et la lisent là où l’option la restreint ; puis une fois cassée exprès, l’opérande droit de `||` recevant les faits vrais du gauche, pour voir un test échouer (constats M015, M024).

### Critères, enregistrés maintenant

- Une option qui change un module d’un programme qui ne l’utilise pas, échoue à `check-regression`, ou échoue à un contrôle de justesse ou de sûreté ci-dessus, est écartée.
- Une option n’est une amélioration que si elle rapporte **moins d’erreurs** que n0 sur N1 à N5, ses formes qui se construisent en **pas plus de lignes** que celles de n0, chaque module au plus à **1,05** fois le temps du module n0, son étendue prise en compte, sa mémoire au plus **1,10 fois ou une page de 64 Kio de plus** et ses octets pas plus que ceux du module n0, et le compilateur au plus à 1,05 fois son temps.
- Parmi les améliorations, celle qui laisse le moins d’erreurs sur N1 à N5 ; puis celle qui fait le moins grandir le compilateur, en octets.
- Si aucune option n’en est une, n0 reste, les prototypes sont retirés, leurs chiffres gardés ici, et le constat F118 dit pourquoi.

## Journal

_À venir : les prototypes, la mesure, la proposition._
