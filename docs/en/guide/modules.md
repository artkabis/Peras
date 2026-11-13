# Modules

A program of some size is split into files. An owned module (`.peras`) imports what other owned modules export: functions, structures and enumerations ([decision 0022](../decisions/0022-owned-modules.md)).

## Exporting and importing

`export` marks what another file may import. Here `shapes.peras` exports a structure, an enumeration and two functions, and keeps `half` to itself:

```peras file shapes.peras
export struct Size {
  width: f64;
  height: f64;
}

export enum Shape {
  Circle(f64),
  Box(Size),
}

fn half(x: f64): f64 {
  return x / 2.0;
}

export fn area(s: Size): f64 {
  return s.width * s.height;
}

export fn inner(s: Size): Size {
  return Size(half(s.width), half(s.height));
}
```

Another file imports them by name, from the path of the file, relative to itself:

```peras main
import { Size, Shape, area, inner } from "./shapes.peras";

fn half(x: f64): f64 {
  return x * 0.5;
}

export fn surface(shapes: Array<Shape>): f64 {
  let total = 0.0;
  for (const shape of shapes) {
    total += match (shape) {
      Shape.Circle(r) => 3.14159 * r * r,
      Shape.Box(size) => area(inner(size)),
    };
  }
  return half(total);
}
```

Both files have a private `half`: a name not exported is the file's own.

- **The path** starts with `./`, names a `.peras` file, extension included, and stays in the folder of the importing file or below it: `./shapes.peras`, `./lib/shapes.peras`.
- **An imported structure or enumeration** is the one of the file that declares it. Imported twice, through two files, it is one type.
- **A name is taken once.** An imported name that the file declares too, or imports from another file, is refused ([E0244](../errors/E0244.md)).

## One build, one module

`perasc build main.peras` follows the imports. Every file it reaches is compiled into the same WebAssembly module, with one memory and one allocator:

- a value passed from one file to another is not copied, and ownership is checked across files as within one;
- the compiler's optimizations see every file's functions, and the WebAssembly engine may inline a call from one file to another as within one file;
- JavaScript calls only what the file built exports. Built alone, `shapes.peras` exports its own functions.

## Cycles are refused

Each file is checked against what the files it imports export, so those are checked first. A file that depends on itself, directly or through others, is refused, with the path of the cycle ([E0250](../errors/E0250.md)). Move what the files of the cycle share into a third file, which each of them imports.

## Not yet

- **Generic functions, structures and enumerations** cannot be imported yet ([E0244](../errors/E0244.md)): a generic is made again for each type it is used with, from its source. This is the next step of [decision 0022](../decisions/0022-owned-modules.md).
- **Renaming an import**, `import { a as b }`, does not exist.
- **The methods of an imported structure** are not imported with it: call functions instead.
- **A function no export reaches** is still compiled into the module: an imported file brings all its functions.

## Tooling

- `perasc build --json` lists the files the module was compiled from, under `inputs`.
- The Vite plugin watches them: changing an imported file rebuilds what imports it.
- An error in an imported file is shown in that file.
- A managed module (`.apeiron`) that imports an owned module needs nothing more: that module is built with the files it imports ([managed modules](managed.md)).
