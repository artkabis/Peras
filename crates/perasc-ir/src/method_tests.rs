//! Methods, interfaces, type aliases, tuples and module constants, run in
//! the interpreter: a method is a function whose first argument is the value
//! before the dot, an interface call reaches the method of each instance's
//! type, a tuple is a structure, a constant is its value where it is used.

use crate::interp::{Value, run_with_heap};
use crate::{FnId, Module, lower, verify};

fn compile(source: &str) -> Module {
    let parsed = perasc_syntax::parse(source);
    assert_eq!(parsed.diagnostics, [], "syntax errors");
    let analysis = perasc_check::analyze(&parsed.ast, source);
    assert_eq!(analysis.diagnostics, [], "check errors");
    let module = lower(&parsed.ast, &analysis).unwrap_or_else(|e| unreachable!("{e:#?}"));
    assert_eq!(verify(&module), Ok(()), "{module}");
    module
}

#[test]
fn methods_interfaces_tuples_and_constants_run_without_a_leak() {
    let module = compile(SOURCE);
    for (n, expected) in [(5, 68_246.5), (-3, 67_246.5)] {
        let (value, heap) = run_with_heap(&module, FnId(0), &[Value::I32(n)]).unwrap_or_else(|t| {
            unreachable!(
                "{t:?}
{module}"
            )
        });
        assert_eq!(
            value,
            Value::F64(expected),
            "{n}
{module}"
        );
        assert_eq!(
            heap.live,
            Vec::<u32>::new(),
            "leak on {n}
{module}"
        );
    }
    let names: Vec<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
    for name in [
        "Point_norm",
        "Point_shift",
        "Circle_area",
        "Rect_area",
        "Stack_push__String",
        "total__Circle",
        "total__Rect",
        "grown__Circle",
    ] {
        assert!(names.contains(&name), "{name} missing from {names:?}");
    }
}

const SOURCE: &str = r#"export fn run(n: i32): f64 {
  let p = Point.new(3.0, 4.0);
  p.shift(1.0);
  const q = p.swapped();
  const circles: Array<Circle> = [Circle(1.0), Circle(2.0)];
  const rects: Array<Rect> = [Rect(1.0, 2.0)];
  let st: Stack<String> = Stack([]);
  st.push("a");
  st.push("bcd");
  const top = st.pop().unwrapOr("");
  const m = minmax([n, 7, -2]);
  const nested: (i32, (f64, bool)) = (1, (2.5, true));
  const g: Grid<i32> = [[1, 2], [3]];
  const label = GREETING + "!";
  return p.norm() + Point.norm(q) + total(circles) + total(rects) + grown(Circle(1.0), 2.0)
    + (st.size() * 100 + top.length * 10 + AREA + m.0 * 1000 + m.1 * 10000 + g.length + label.length).toF64()
    + nested.1.0 + ORIGIN.x;
}

type Meters = f64;
type Grid<T> = Array<Array<T>>;

const SIDE: i32 = 4;
const AREA: i32 = SIDE * SIDE;
const GREETING: String = "hi";
const ORIGIN: Point = Point(0.0, 0.0);

struct Point {
  x: Meters;
  y: Meters;
}

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

impl Point {
  fn new(x: f64, y: f64): Point {
    return Point(x, y);
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

impl Shape for Circle {
  fn area(self): f64 {
    return 3.0 * self.r * self.r;
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

fn total<T: Shape>(shapes: Array<T>): f64 {
  let sum = 0.0;
  for (const s of shapes) {
    sum += s.area();
  }
  return sum;
}

fn grown<T: Shape>(s: sink T, k: f64): f64 {
  let t = s;
  t.scale(k);
  return t.area();
}

fn minmax(xs: Array<i32>): (i32, i32) {
  let lo = xs[0];
  let hi = xs[0];
  for (const x of xs) {
    if (x < lo) {
      lo = x;
    }
    if (x > hi) {
      hi = x;
    }
  }
  return (lo, hi);
}

"#;
