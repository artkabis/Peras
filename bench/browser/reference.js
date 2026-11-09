// Browser benchmark: the JavaScript side, written as a developer who cares
// about speed would write it: typed arrays for numbers, plain objects for
// structures, BigInt for 64-bit integers. Each function is the twin of the one
// with the same name in kernels.peras.

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
