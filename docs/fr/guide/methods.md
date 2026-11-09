# Méthodes et interfaces

Un `impl` donne ses méthodes à une structure ou à une énumération. Une interface liste des méthodes que plusieurs types fournissent, et sert de contrainte au code générique : `fn total<T: Shape>(…)` marche pour tout type qui implémente `Shape`. Ce code est compilé une fois par type, et chaque appel va droit à la méthode de ce type. <span class="status compiled">compilé</span>

## Méthodes

```peras build
struct Point {
  x: f64;
  y: f64;
}

impl Point {
  fn origin(): Point {
    return Point(0.0, 0.0);
  }

  fn norm(self): f64 {
    return self.x * self.x + self.y * self.y;
  }

  fn shift(inout self, dx: f64) {
    self.x += dx;
  }

  fn swapped(self): Self {
    return Point(self.y, self.x);
  }
}

export fn demo(): f64 {
  let p = Point.origin();
  p.shift(3.0);
  return p.norm() + p.swapped().y + Point.norm(p);
}
```

Le premier paramètre d’une méthode est `self`, la valeur avant le point. Il prend les conventions de tout paramètre :

- `self` lit la valeur ;
- `inout self` la modifie sur place : `p.shift(3.0)` demande donc que `p` soit un `let` ([E0205](../errors/E0205.md)), sans `&` à écrire ;
- `sink self` la consomme.

Une méthode sans `self` s’appelle sur son type, `Point.origin()`. Toute méthode peut s’appeler ainsi, son `self` écrit en premier : `Point.norm(p)`. Dans un `impl`, `Self` nomme son type.

Les méthodes appartiennent à une structure ou à une énumération du module ([E0234](../errors/E0234.md)). Un type générique reçoit les siennes avec `impl<T> Stack<T>` :

```peras build
struct Stack<T> {
  items: Array<T>;
}

impl<T> Stack<T> {
  fn size(self): i32 {
    return self.items.length;
  }

  fn push(inout self, value: sink T) {
    self.items.push(value);
  }

  fn pop(inout self): Option<T> {
    return self.items.pop();
  }
}

export fn top(): i32 {
  let names: Stack<String> = Stack([]);
  names.push("ada");
  names.push("grace");
  const last = names.pop().unwrapOr("");
  return last.length + names.size();
}
```

## Interfaces

```peras build
interface Shape {
  fn area(self): f64;
  fn scale(inout self, k: f64);
}

struct Circle {
  r: f64;
}

struct Rect {
  w: f64;
  h: f64;
}

impl Shape for Circle {
  fn area(self): f64 {
    return 3.14159 * self.r * self.r;
  }

  fn scale(inout self, k: f64) {
    self.r *= k;
  }
}

impl Shape for Rect {
  fn area(self): f64 {
    return self.w * self.h;
  }

  fn scale(inout self, k: f64) {
    self.w *= k;
    self.h *= k;
  }
}

fn total<T: Shape>(shapes: Array<T>): f64 {
  let sum = 0.0;
  for (const s of shapes) {
    sum += s.area();
  }
  return sum;
}

export fn areas(): f64 {
  const circles: Array<Circle> = [Circle(1.0), Circle(2.0)];
  const rects: Array<Rect> = [Rect(1.0, 2.0)];
  return total(circles) + total(rects);
}
```

Une interface déclare des méthodes, `self` en premier, `Self` représentant le type qui l’implémente ([E0236](../errors/E0236.md)). `impl Shape for Circle` fournit exactement ces méthodes, avec exactement leurs signatures ([E0235](../errors/E0235.md)). Les méthodes d’une interface sont aussi des méthodes du type : `Circle(1.0).area()` marche.

Une interface est une contrainte, seule ou avec d’autres, `T: Shape + Copy` ; un appel avec un type qui ne l’implémente pas est refusé ([E0227](../errors/E0227.md)). Ce n’est pas un type : `fn f(s: Shape)` est refusé ([E0237](../errors/E0237.md)). Le code qui marche pour toute forme est générique : il n’y a jamais de table de méthodes à consulter à l’exécution.

## En JavaScript

Les méthodes ne sont pas exportées : JavaScript voit les structures comme des objets, et appelle les fonctions exportées. Une fonction exportée peut appeler n’importe quelle méthode.
