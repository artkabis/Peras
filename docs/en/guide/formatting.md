# Formatting

`perasc fmt` lays a Peras file out as Prettier lays out TypeScript, at a width of 100 ([decision 0041](../decisions/0041-formatter.md)). Every file looks the same, whoever wrote it: there is nothing to set.

A file written in haste:

```text
struct Point { x: f64, y: f64 }

// The perimeter of a path, closed or not.
export fn perimeter(points: Array<Point>, closedPath: bool, scale: f64, offset: f64, round: bool): f64 {
  let total = 0.0;
  for (let i = 1; i < points.length; i++) { total += distance(points[i - 1], points[i]); }
  if (closedPath && points.length > 2) { total += distance(points[points.length - 1], points[0]); }
  const value = total * scale + offset;
  if (round) { return Math.round(value); }
  return value;
}

fn distance(a: Point, b: Point): f64 {
    const dx = a.x-b.x;
    const dy = a.y-b.y;
    return Math.sqrt(dx*dx + dy*dy);
}
```

after `perasc fmt shapes.peras`:

```peras build
struct Point {
  x: f64,
  y: f64,
}

// The perimeter of a path, closed or not.
export fn perimeter(
  points: Array<Point>,
  closedPath: bool,
  scale: f64,
  offset: f64,
  round: bool,
): f64 {
  let total = 0.0;
  for (let i = 1; i < points.length; i++) {
    total += distance(points[i - 1], points[i]);
  }
  if (closedPath && points.length > 2) {
    total += distance(points[points.length - 1], points[0]);
  }
  const value = total * scale + offset;
  if (round) {
    return Math.round(value);
  }
  return value;
}

fn distance(a: Point, b: Point): f64 {
  const dx = a.x - b.x;
  const dy = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}
```

## What it does

- A block puts each statement on its line, indented by two spaces; a blank line between two statements is kept, a run of them made one.
- A list, the parameters of a function, the arguments of a call, the elements of an array, the arms of a `match`, stays on one line if it fits in 100 columns, and puts each element on its line otherwise, with a trailing comma. A function given last, `xs.map((x) => { … })`, stays on the line of its call.
- A chain of three calls or more, `xs.filter(…).map(…).join(…)`, breaks before each `.` if it does not fit; a long condition after each `&&` and `||`; a long sum after each `+` and `-`.
- The body of a `struct` or an `enum` puts a field on each line, and keeps the blank lines between them.
- Comments stay where they are, on their line or at the end of the line they follow. A template literal is never broken inside its `${…}`, nor is a string.
- Literals keep the text they were written with: `1_000`, `0xff`, `1.50`.

## Using it

```sh
perasc fmt src/*.peras src/*.apeiron          # formats each file in place
perasc fmt --check src/*.peras src/*.apeiron  # names the files it would change, exit status 1 if any
```

`--check` is the one for a CI. A file with a syntax error is left as it is, and named: `perasc check` shows its errors. Formatting changes nothing a program does: a file gives the same module, byte for byte, before and after, which the repository checks on each of its Peras files.
