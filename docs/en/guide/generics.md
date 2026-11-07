# Generics

A generic function, structure or enumeration is written once for many types: `larger` works for every type that can be compared, `Pair` holds any two values. Each type parameter says what its types must be able to do, and the compiler checks both sides: the generic code uses only what the constraints allow, and each use gives types that meet them. <span class="status compiled">compiled</span>

## Generic functions

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

The type parameters follow the name, between `<` and `>`. Here `larger` is called with `i32`, then with `f64`: the type arguments come from the arguments.

## Constraints

A type parameter without a constraint can be any type, so its body can only pass its values along. A constraint, written `T: Ord`, or `T: Numeric + Copy` for several, allows more:

| Constraint | Allows                                         | Met by                             |
| ---------- | ---------------------------------------------- | ---------------------------------- |
| `Eq`       | `==`, `!=`                                     | every type but `void`              |
| `Ord`      | `<`, `<=`, `>`, `>=`, and what `Eq` allows     | numbers                            |
| `Numeric`  | arithmetic, `+=`, `++`, conversions, and `Ord` | numbers                            |
| `Integer`  | `&`, `\|`, `^`, `~`, shifts, and `Numeric`     | integers                           |
| `Float`    | unary `-`, and `Numeric`                       | `f32`, `f64`                       |
| `Copy`     | using a value without moving it                | types that hold no string or array |

`Ord` and `Numeric` imply `Copy`: numbers are copied. A body that uses an operation its constraints do not allow is refused with [E0228](../errors/E0228.md), whose fix adds the constraint; a call with a type that does not meet one is refused with [E0227](../errors/E0227.md).

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

## Inference and type arguments

The type arguments come from the arguments, then from the type the call must have. When neither says, write them after the name, or give the result a type ([E0229](../errors/E0229.md)):

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

Peras refuses chained comparisons such as `a < b > c`, so `name<Type>(…)` is always a call with type arguments.

## Generic structures and enumerations

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

`Pair(n, 2.5)` is a `Pair<i32, f64>`: the type arguments of a constructor or a variant come from its data, or from the type expected, as for `Tree.Empty` above. A pattern names the enumeration without its arguments: `Tree.Leaf(x)`.

## Ownership

A value of a type parameter moves like a string or an array, unless its parameter is `Copy`: the generic code must be correct for every type, including those that hold a buffer. Passing it on is done as for any value, by `sink`, `inout`, or a copy the caller makes:

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

## How it compiles

Each set of type arguments the module uses gives its own copy of the generic code, specialized for those types: `larger<i32>` and `larger<f64>` are two functions, each as fast as if written by hand, with nothing decided at run time. A generic function that is never called is checked, and not compiled.

Two consequences:

- an exported function cannot be generic ([E0230](../errors/E0230.md)): JavaScript passes any value, so there is no single copy to export; export a function that calls the generic one with the types needed;
- generic code that would need a copy for ever larger types, as when `f<T>` calls `f<Array<T>>`, is refused ([E0231](../errors/E0231.md)).

Interfaces, which will let you write your own constraints, come next: see the [roadmap](../reference/roadmap.md).
