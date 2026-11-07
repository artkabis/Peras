# Génériques

Une fonction, une structure ou une énumération générique s’écrit une fois pour de nombreux types : `larger` marche pour tout type comparable, `Pair` contient deux valeurs quelconques. Chaque paramètre de type dit ce que ses types doivent savoir faire, et le compilateur vérifie les deux côtés : le code générique n’utilise que ce que les contraintes permettent, et chaque utilisation donne des types qui les respectent. <span class="status compiled">compilé</span>

## Fonctions génériques

```peras build
fn larger<T: Ord>(a: T, b: T): T {
  if (a > b) {
    return a;
  }
  return b;
}

export fn biggest(a: i32, b: i32, c: f64): f64 {
  return larger(a, b).toF64() + larger(c, 0.5);
}
```

Les paramètres de type suivent le nom, entre `<` et `>`. Ici, `larger` est appelée avec `i32`, puis avec `f64` : les arguments de type viennent des arguments.

## Contraintes

Un paramètre de type sans contrainte peut être n’importe quel type : son corps ne peut que transmettre ses valeurs. Une contrainte, écrite `T: Ord`, ou `T: Numeric + Copy` pour plusieurs, en permet davantage :

| Contrainte | Permet                                                | Respectée par                                  |
| ---------- | ----------------------------------------------------- | ---------------------------------------------- |
| `Eq`       | `==`, `!=`                                            | tous les types sauf `void`                     |
| `Ord`      | `<`, `<=`, `>`, `>=`, et ce que permet `Eq`           | les nombres                                    |
| `Numeric`  | l’arithmétique, `+=`, `++`, les conversions, et `Ord` | les nombres                                    |
| `Integer`  | `&`, `\|`, `^`, `~`, les décalages, et `Numeric`      | les entiers                                    |
| `Float`    | le `-` unaire, et `Numeric`                           | `f32`, `f64`                                   |
| `Copy`     | utiliser une valeur sans la déplacer                  | les types sans chaîne ni tableau à l’intérieur |

`Ord` et `Numeric` impliquent `Copy` : les nombres sont copiés. Un corps qui utilise une opération que ses contraintes ne permettent pas est refusé avec [E0228](../errors/E0228.md), dont la correction ajoute la contrainte ; un appel avec un type qui n’en respecte pas une est refusé avec [E0227](../errors/E0227.md).

```peras build
fn sum<T: Numeric>(xs: Array<T>, start: T): T {
  let total = start;
  for (const x of xs) {
    total += x;
  }
  return total;
}

fn mean<T: Numeric>(xs: Array<T>, zero: T): f64 {
  return sum(xs, zero).toF64() / xs.length.toF64();
}

export fn average(xs: Array<i32>): f64 {
  return mean(xs, 0);
}
```

## Inférence et arguments de type

Les arguments de type viennent des arguments, puis du type que l’appel doit avoir. Quand rien ne les donne, écrivez-les après le nom, ou donnez un type au résultat ([E0229](../errors/E0229.md)) :

```peras build
fn empty<T>(): Array<T> {
  return [];
}

fn larger<T: Ord>(a: T, b: T): T {
  if (a > b) {
    return a;
  }
  return b;
}

export fn sizes(): i64 {
  const words: Array<String> = empty();
  const counts = empty<i64>();
  return larger<i64>(1, 2) + counts.length.toI64() + words.length.toI64();
}
```

Peras refuse les comparaisons enchaînées comme `a < b > c` : `nom<Type>(…)` est donc toujours un appel avec des arguments de type.

## Structures et énumérations génériques

```peras build
struct Pair<A, B> {
  first: A;
  second: B;
}

enum Tree<T> {
  Leaf(T),
  Node(T, T),
  Empty,
}

fn swap<A: Copy, B: Copy>(p: Pair<A, B>): Pair<B, A> {
  return Pair(p.second, p.first);
}

fn size<T>(t: Tree<T>): i32 {
  return match (t) {
    Tree.Leaf(_) => 1,
    Tree.Node(_, _) => 2,
    Tree.Empty => 0,
  };
}

export fn demo(n: i32): f64 {
  const p = swap(Pair(n, 2.5));
  const t = Tree.Node("a", "b");
  const e: Tree<f64> = Tree.Empty;
  return p.first + (p.second + size(t) + size(e)).toF64();
}
```

`Pair(n, 2.5)` est une `Pair<i32, f64>` : les arguments de type d’un constructeur ou d’une variante viennent de ses données, ou du type attendu, comme pour `Tree.Empty` ci-dessus. Un motif nomme l’énumération sans ses arguments : `Tree.Leaf(x)`.

## Possession

Une valeur d’un paramètre de type se déplace comme une chaîne ou un tableau, sauf si son paramètre est `Copy` : le code générique doit être correct pour tous les types, y compris ceux qui contiennent un tampon. On la transmet comme toute valeur, par `sink`, `inout`, ou une copie faite par l’appelant :

```peras build
fn last<T>(xs: inout Array<T>): Option<T> {
  return xs.pop();
}

fn pushBoth<T>(xs: inout Array<T>, a: sink T, b: sink T) {
  xs.push(a);
  xs.push(b);
}

export fn words(): i32 {
  let names: Array<String> = ["ada"];
  pushBoth(&names, "grace", "alan");
  return last(&names).unwrapOr("").length;
}
```

## Compilation

Chaque jeu d’arguments de type utilisé par le module donne sa propre copie du code générique, spécialisée pour ces types : `larger<i32>` et `larger<f64>` sont deux fonctions, chacune aussi rapide qu’écrite à la main, sans rien décider à l’exécution. Une fonction générique jamais appelée est vérifiée, et pas compilée.

Deux conséquences :

- une fonction exportée ne peut pas être générique ([E0230](../errors/E0230.md)) : JavaScript passe n’importe quelle valeur, il n’y a donc pas de copie unique à exporter ; exportez une fonction qui appelle la fonction générique avec les types voulus ;
- du code générique qui demanderait une copie pour des types toujours plus grands, comme quand `f<T>` appelle `f<Array<T>>`, est refusé ([E0231](../errors/E0231.md)).

Les interfaces, qui permettront d’écrire vos propres contraintes, arrivent ensuite : voir la [feuille de route](../reference/roadmap.md).
