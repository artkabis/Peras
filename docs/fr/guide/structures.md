# Structures

Une structure regroupe des champs nommés en une seule valeur.

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

## Créer une structure

On construit une structure en appelant son nom avec ses champs, dans l’ordre de leur déclaration : `Vec2(1.0, 2.0)`. Chaque champ est donné ; il n’y a ni valeur par défaut, ni structure à moitié construite.

## Champs

Les champs se lisent et s’écrivent avec un point, y compris avec les affectations composées et les incréments, sur une variable `let` :

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

Un champ qui n’existe pas est une erreur qui liste ceux qui existent ([E0215](../errors/E0215.md)).

## Des valeurs, pas des références

Une structure est une valeur : l’affecter la copie, et modifier la copie laisse l’original tel quel.

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

`copies()` renvoie `1`. Affecter une structure à elle-même fonctionne aussi : `p = Point(p.y, p.x)` échange les deux champs.

## Structures imbriquées

Un champ peut être une structure :

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

## Ce que ça coûte

Rien sur le tas. Une structure est remplacée par ses champs : une variable `Vec2` est deux variables `f64`, un paramètre `Vec2` deux paramètres, et une fonction qui renvoie un `Vec2` renvoie deux valeurs. Elle coûte ce que coûterait le même code écrit avec des nombres séparés.

## Depuis JavaScript

Une fonction exportée qui prend ou renvoie des structures prend et renvoie de simples objets, et le TypeScript généré les déclare comme interfaces :

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

> Les structures dont les champs sont des nombres, des booléens ou des structures sont compilées. Un champ tableau ou chaîne est vérifié, pas encore compilé.

Ensuite : [tableaux, chaînes et options](collections.md).
