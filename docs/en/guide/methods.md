# Methods and interfaces

An `impl` gives a structure or an enumeration its methods. An interface lists methods that several types provide, and is a constraint for generic code: `fn total<T: Shape>(…)` works for every type that implements `Shape`, compiled once per type, with each call going straight to that type's method. <span class="status compiled">compiled</span>

## Methods

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

A method's first parameter is `self`, the value before the dot. It takes the conventions of any parameter:

- `self` reads the value;
- `inout self` changes it in place, so `p.shift(3.0)` needs `p` to be a `let` ([E0205](../errors/E0205.md)), with no `&` to write;
- `sink self` consumes it.

A method without `self` is called on its type, `Point.origin()`. Any method can be called that way, with its `self` written first: `Point.norm(p)`. Inside an `impl`, `Self` names its type.

Methods belong to a structure or an enumeration of the module ([E0234](../errors/E0234.md)). A generic type gets its methods with `impl<T> Stack<T>`:

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

An interface declares methods, `self` first, `Self` standing for the type that implements it ([E0236](../errors/E0236.md)). `impl Shape for Circle` provides exactly those methods, with exactly their signatures ([E0235](../errors/E0235.md)). The methods of an interface are methods of the type too: `Circle(1.0).area()` works.

An interface is a constraint, alone or with others, `T: Shape + Copy`; a call with a type that does not implement it is refused ([E0227](../errors/E0227.md)). It is not a type: `fn f(s: Shape)` is refused ([E0237](../errors/E0237.md)). Code that works for every shape is generic, so there is never a table of methods to look up at run time.

## In JavaScript

Methods are not exported: JavaScript sees structures as objects, and calls exported functions. An exported function can call any method.
