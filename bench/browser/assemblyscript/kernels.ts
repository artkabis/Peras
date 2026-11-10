// The kernels of bench/browser/kernels.peras, written as an AssemblyScript
// developer writes them: a class per body, `split` and `filter`, the standard
// library's sort, the garbage collector AssemblyScript uses by default.
// Arrays of numbers stay in the module's memory, as a `PerasArray` does:
// JavaScript reads them at the address `dataF64` or `dataI32` gives, with no
// copy. `scripts/bench-browser.mjs --contenders` builds it.

// An array of n f64, kept alive until `release`.
export function newF64(n: i32): usize {
  const a = new Float64Array(n);
  return __pin(changetype<usize>(a));
}

// An array of n i32, kept alive until `release`.
export function newI32(n: i32): usize {
  const a = new Int32Array(n);
  return __pin(changetype<usize>(a));
}

// Where the elements of an array start.
export function dataF64(a: usize): usize {
  return changetype<Float64Array>(a).dataStart;
}

export function dataI32(a: usize): usize {
  return changetype<Int32Array>(a).dataStart;
}

// Lets the collector take the array back.
export function release(a: usize): void {
  __unpin(a);
}

// Particles moving in a box, bouncing off its walls.
export function particles(xsRef: usize, vsRef: usize, steps: i32): f64 {
  const xs = changetype<Float64Array>(xsRef);
  const vs = changetype<Float64Array>(vsRef);
  const dt = 0.01;
  for (let s = 0; s < steps; s++) {
    for (let i = 0; i < xs.length; i++) {
      const v = vs[i];
      let x = xs[i] + v * dt;
      if (x < 0 || x > 1) {
        vs[i] = -v;
        x = xs[i] - v * dt;
      }
      xs[i] = x;
    }
  }
  let total = 0.0;
  for (let i = 0; i < xs.length; i++) total += xs[i];
  return total;
}

// Pixels packed as 0xAARRGGBB turned to grey in place.
export function grayscale(pixelsRef: usize): i32 {
  const pixels = changetype<Int32Array>(pixelsRef);
  let checksum = 0;
  for (let i = 0; i < pixels.length; i++) {
    const p = pixels[i];
    const r = (p >> 16) & 255;
    const g = (p >> 8) & 255;
    const b = p & 255;
    const y = (r * 77 + g * 150 + b * 29) >> 8;
    const q = (p & -16777216) | (y << 16) | (y << 8) | y;
    pixels[i] = q;
    checksum ^= q;
  }
  return checksum;
}

class Body {
  constructor(
    public x: f64,
    public y: f64,
    public vx: f64,
    public vy: f64,
    public m: f64,
  ) {}
}

// Gravity between n bodies, each pair at each step.
export function nbody(n: i32, steps: i32): f64 {
  const bodies = new Array<Body>();
  for (let i = 0; i < n; i++) {
    bodies.push(new Body(<f64>((i * 7) % 100), <f64>((i * 13) % 100), 0, 0, 1 + <f64>(i % 3)));
  }
  for (let s = 0; s < steps; s++) {
    for (let i = 0; i < n; i++) {
      let ax = 0.0;
      let ay = 0.0;
      const xi = bodies[i].x;
      const yi = bodies[i].y;
      for (let j = 0; j < n; j++) {
        if (i != j) {
          const dx = bodies[j].x - xi;
          const dy = bodies[j].y - yi;
          const d2 = dx * dx + dy * dy + 0.01;
          const inv = bodies[j].m / (d2 * Math.sqrt(d2));
          ax += dx * inv;
          ay += dy * inv;
        }
      }
      bodies[i].vx += ax * 0.001;
      bodies[i].vy += ay * 0.001;
    }
    for (let i = 0; i < n; i++) {
      bodies[i].x += bodies[i].vx * 0.001;
      bodies[i].y += bodies[i].vy * 0.001;
    }
  }
  let energy = 0.0;
  for (let i = 0; i < n; i++) {
    const b = bodies[i];
    energy += b.m * (b.vx * b.vx + b.vy * b.vy);
  }
  return energy;
}

// FNV-1a over n integers, on 64 bits.
export function hash64(n: i32): i64 {
  let h: i64 = -3750763034362895579;
  for (let i = 0; i < n; i++) {
    h = (h ^ (<i64>i)) * 1099511628211;
  }
  return h;
}

// Sorting numbers in place.
export function sorted(xsRef: usize): f64 {
  const xs = changetype<Float64Array>(xsRef);
  xs.sort();
  return xs[0] + xs[xs.length / 2] + xs[xs.length - 1];
}

// Points inside the Mandelbrot set.
export function mandel(size: i32): i32 {
  let inside = 0;
  const scale = 3.0 / <f64>size;
  for (let py = 0; py < size; py++) {
    for (let px = 0; px < size; px++) {
      const cx = <f64>px * scale - 2.0;
      const cy = <f64>py * scale - 1.5;
      let x = 0.0;
      let y = 0.0;
      let step = 0;
      while (step < 64 && x * x + y * y <= 4.0) {
        const next = x * x - y * y + cx;
        y = 2.0 * x * y + cy;
        x = next;
        step++;
      }
      if (step == 64) inside++;
    }
  }
  return inside;
}

// Words of a text: split, filter, count. AssemblyScript strings are UTF-16,
// as JavaScript's: the text is copied in as it is, with no conversion.
export function words(text: string): i32 {
  return text.split(" ").filter((w) => w.length > 0).length;
}

// The smallest function, to measure what a call from JavaScript costs.
export function add(a: i32, b: i32): i32 {
  return a + b;
}
