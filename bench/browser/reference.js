// Browser benchmark: the JavaScript side. Each function is the twin of the
// one with the same name in kernels.peras, written as JavaScript is usually
// written: typed arrays for numbers, plain objects for structures, BigInt for
// 64-bit integers, the methods of strings and arrays.
//
// Where an expert writes it otherwise, for speed, the `…Expert` function does:
// a loop over the characters instead of `split`, two 32-bit halves instead of
// BigInt, columns of Float64Array instead of objects, a radix sort instead of
// the engine's. Each gives exactly the same result. The particles, the pixels,
// Mandelbrot and the calls are already written as an expert would.

/**
 * @param {Float64Array} xs
 * @param {Float64Array} vs
 * @param {number} steps
 */
export function particles(xs, vs, steps) {
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
  let total = 0;
  for (const x of xs) total += x;
  return total;
}

/** @param {Int32Array} pixels */
export function grayscale(pixels) {
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

/**
 * @param {number} n
 * @param {number} steps
 */
export function nbody(n, steps) {
  const bodies = [];
  for (let i = 0; i < n; i++) {
    bodies.push({ x: (i * 7) % 100, y: (i * 13) % 100, vx: 0, vy: 0, m: 1 + (i % 3) });
  }
  for (let s = 0; s < steps; s++) {
    for (let i = 0; i < n; i++) {
      let ax = 0;
      let ay = 0;
      const xi = bodies[i].x;
      const yi = bodies[i].y;
      for (let j = 0; j < n; j++) {
        if (i !== j) {
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
  let energy = 0;
  for (const b of bodies) energy += b.m * (b.vx * b.vx + b.vy * b.vy);
  return energy;
}

/** @param {number} n */
export function hash64(n) {
  let h = -3750763034362895579n;
  for (let i = 0; i < n; i++) {
    h = BigInt.asIntN(64, (h ^ BigInt(i)) * 1099511628211n);
  }
  return h;
}

/** @param {Float64Array} xs */
export function sorted(xs) {
  xs.sort();
  return xs[0] + xs[Math.floor(xs.length / 2)] + xs[xs.length - 1];
}

/** @param {number[]} xs */
export function sortedArray(xs) {
  xs.sort((a, b) => a - b);
  return xs[0] + xs[Math.floor(xs.length / 2)] + xs[xs.length - 1];
}

/** @param {number} size */
export function mandel(size) {
  let inside = 0;
  const scale = 3 / size;
  for (let py = 0; py < size; py++) {
    for (let px = 0; px < size; px++) {
      const cx = px * scale - 2;
      const cy = py * scale - 1.5;
      let x = 0;
      let y = 0;
      let step = 0;
      while (step < 64 && x * x + y * y <= 4) {
        const next = x * x - y * y + cx;
        y = 2 * x * y + cy;
        x = next;
        step++;
      }
      if (step === 64) inside++;
    }
  }
  return inside;
}

/** @param {string} text */
export function words(text) {
  return text.split(" ").filter((w) => w.length > 0).length;
}

/**
 * @param {number} a
 * @param {number} b
 */
export function add(a, b) {
  return a + b;
}

/**
 * `words` without `split`: one pass over the characters, no array.
 * @param {string} text
 */
export function wordsExpert(text) {
  let n = 0;
  let inWord = false;
  for (let i = 0; i < text.length; i++) {
    if (text.charCodeAt(i) === 32) inWord = false;
    else if (!inWord) {
      inWord = true;
      n++;
    }
  }
  return n;
}

/**
 * `hash64` in two 32-bit halves, no BigInt until the end. The FNV prime is
 * 2^40 + 0x1b3: the low half times 0x1b3 fits a double exactly, and the
 * 2^40 part shifts the low half into the high one.
 * @param {number} n
 */
export function hash64Expert(n) {
  let lo = 0x84222325;
  let hi = 0xcbf29ce4;
  for (let i = 0; i < n; i++) {
    lo = (lo ^ i) >>> 0;
    const product = lo * 0x1b3;
    const carry = Math.floor(product / 4294967296);
    hi = (Math.imul(hi, 0x1b3) + carry + (lo << 8)) >>> 0;
    lo = product >>> 0;
  }
  return BigInt.asIntN(64, (BigInt(hi) << 32n) | BigInt(lo));
}

/**
 * `nbody` with each field in a column, as Peras stores an array of
 * structures: the same operations in the same order, so the same result.
 * @param {number} n
 * @param {number} steps
 */
export function nbodyExpert(n, steps) {
  const x = new Float64Array(n);
  const y = new Float64Array(n);
  const vx = new Float64Array(n);
  const vy = new Float64Array(n);
  const m = new Float64Array(n);
  for (let i = 0; i < n; i++) {
    x[i] = (i * 7) % 100;
    y[i] = (i * 13) % 100;
    m[i] = 1 + (i % 3);
  }
  for (let s = 0; s < steps; s++) {
    for (let i = 0; i < n; i++) {
      let ax = 0;
      let ay = 0;
      const xi = x[i];
      const yi = y[i];
      for (let j = 0; j < n; j++) {
        if (i !== j) {
          const dx = x[j] - xi;
          const dy = y[j] - yi;
          const d2 = dx * dx + dy * dy + 0.01;
          const inv = m[j] / (d2 * Math.sqrt(d2));
          ax += dx * inv;
          ay += dy * inv;
        }
      }
      vx[i] += ax * 0.001;
      vy[i] += ay * 0.001;
    }
    for (let i = 0; i < n; i++) {
      x[i] += vx[i] * 0.001;
      y[i] += vy[i] * 0.001;
    }
  }
  let energy = 0;
  for (let i = 0; i < n; i++) energy += m[i] * (vx[i] * vx[i] + vy[i] * vy[i]);
  return energy;
}

/**
 * `sorted` with a radix sort, as Peras sorts: each number becomes an
 * unsigned 64-bit key in the same order, in two 32-bit halves, sorted 16
 * bits at a time, a pass skipped when every key has the same digit, then
 * turned back. Typed arrays are little-endian on every platform browsers run
 * on: the low half comes first.
 * @param {Float64Array} xs
 */
export function sortedExpert(xs) {
  const n = xs.length;
  const words = new Uint32Array(xs.buffer, xs.byteOffset, n * 2);
  let lo = new Uint32Array(n);
  let hi = new Uint32Array(n);
  for (let i = 0; i < n; i++) {
    const l = words[2 * i];
    const h = words[2 * i + 1];
    if (h & 0x80000000) {
      lo[i] = ~l >>> 0;
      hi[i] = ~h >>> 0;
    } else {
      lo[i] = l;
      hi[i] = (h | 0x80000000) >>> 0;
    }
  }
  let lo2 = new Uint32Array(n);
  let hi2 = new Uint32Array(n);
  const count = new Uint32Array(65536);
  for (let pass = 0; pass < 4; pass++) {
    const half = pass < 2 ? lo : hi;
    const shift = pass % 2 === 0 ? 0 : 16;
    count.fill(0);
    for (let i = 0; i < n; i++) count[(half[i] >>> shift) & 0xffff]++;
    if (count[(half[0] >>> shift) & 0xffff] === n) continue;
    let sum = 0;
    for (let d = 0; d < 65536; d++) {
      const c = count[d];
      count[d] = sum;
      sum += c;
    }
    for (let i = 0; i < n; i++) {
      const at = count[(half[i] >>> shift) & 0xffff]++;
      lo2[at] = lo[i];
      hi2[at] = hi[i];
    }
    [lo, lo2] = [lo2, lo];
    [hi, hi2] = [hi2, hi];
  }
  for (let i = 0; i < n; i++) {
    const h = hi[i];
    if (h & 0x80000000) {
      words[2 * i] = lo[i];
      words[2 * i + 1] = h & 0x7fffffff;
    } else {
      words[2 * i] = ~lo[i] >>> 0;
      words[2 * i + 1] = ~h >>> 0;
    }
  }
  return xs[0] + xs[Math.floor(n / 2)] + xs[n - 1];
}
