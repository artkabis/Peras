# Structures

A structure groups named fields into one value.

```peras build
export struct Vec2 {
  x: f64;
  y: f64;
}

fn add(a: Vec2, b: Vec2): Vec2 {
  return Vec2(a.x + b.x, a.y + b.y);
}

export fn middle(a: Vec2, b: Vec2): Vec2 {
  const sum = add(a, b);
  return Vec2(sum.x / 2.0, sum.y / 2.0);
}
```

## Creating a structure

A structure is built by calling its name with its fields, in the order they are declared: `Vec2(1.0, 2.0)`. Every field is given; there is no default value and no partly built structure.

## Fields

Fields are read and written with a dot, including with compound assignments and increments, on a `let` variable:

```peras build
struct Counter {
  hits: i32;
  misses: i32;
}

export fn tally(n: i32): i32 {
  let c = Counter(0, 0);
  for (let i = 0; i < n; i++) {
    if (i % 3 == 0) {
      c.misses++;
    } else {
      c.hits += 2;
    }
  }
  return c.hits - c.misses;
}
```

A field that does not exist is an error that lists the ones that do ([E0215](../errors/E0215.md)).

## Values, not references

A structure is a value: assigning it copies it, and changing the copy leaves the original as it was.

```peras build
struct Point {
  x: i32;
  y: i32;
}

export fn copies(): i32 {
  let a = Point(1, 2);
  let b = a;
  b.x = 100;
  return a.x;
}
```

`copies()` returns `1`. Assigning a structure to itself works too: `p = Point(p.y, p.x)` swaps the two fields.

## Nested structures

A field can be a structure:

```peras build
struct Vec2 {
  x: f64;
  y: f64;
}

export struct Segment {
  from: Vec2;
  to: Vec2;
  closed: bool;
}

export fn flip(s: Segment): Segment {
  let t = s;
  t.from = s.to;
  t.to = s.from;
  t.closed = !s.closed;
  return t;
}
```

## What it costs

Nothing on the heap. A structure is replaced by its fields: a `Vec2` variable is two `f64` variables, a `Vec2` parameter two parameters, and a function returning a `Vec2` returns two values. It costs what the same code written with separate numbers would.

## From JavaScript

An exported function taking or returning structures takes and returns plain objects, and the generated TypeScript declares them as interfaces:

```ts
import { flip } from "./geometry.peras";

flip({ from: { x: 1, y: 2 }, to: { x: 5, y: 10 }, closed: false });
// { from: { x: 5, y: 10 }, to: { x: 1, y: 2 }, closed: true }
```

```ts
export interface Vec2 {
  x: number;
  y: number;
}
export interface Segment {
  from: Vec2;
  to: Vec2;
  closed: boolean;
}
export declare function flip(s: Segment): Segment;
```

> Structures whose fields are numbers, booleans or structures compile. A field that is an array or a string is checked, not compiled yet.

Next: [arrays, strings and options](collections.md).
