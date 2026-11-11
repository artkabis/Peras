// The demo in TypeScript, as it is often written: each particle an object,
// each frame a new array of new particles, as immutable state is in React,
// Redux or Vue. Correct, typed strictly, and allocating at every frame.
interface Particle {
  readonly x: number;
  readonly y: number;
  readonly vx: number;
  readonly vy: number;
}

export function start(
  xs: Float32Array,
  ys: Float32Array,
  vxs: Float32Array,
  vys: Float32Array,
): readonly Particle[] {
  return Array.from(xs, (x, i) => ({ x, y: ys[i] ?? 0, vx: vxs[i] ?? 0, vy: vys[i] ?? 0 }));
}

export function frame(
  particles: readonly Particle[],
  pixels: Uint32Array,
  width: number,
  height: number,
): readonly Particle[] {
  const cx = width / 2;
  const cy = height / 2;
  const next = particles.map((p) => {
    const dx = cx - p.x;
    const dy = cy - p.y;
    const f = 0.6 / (dx * dx + dy * dy + 30);
    const vx = p.vx + dx * f;
    const vy = p.vy + dy * f;
    return { x: p.x + vx, y: p.y + vy, vx, vy };
  });
  const counts = new Uint32Array(width * height);
  for (const p of next) {
    if (p.x >= 0 && p.y >= 0 && p.x < width && p.y < height) {
      const at = Math.trunc(p.y) * width + Math.trunc(p.x);
      counts[at] = (counts[at] ?? 0) + 1;
    }
  }
  counts.forEach((count, at) => {
    const lum = Math.min(count * 5, 255);
    pixels[at] = 0xff000000 + lum * 65536 + Math.trunc((lum * 3) / 4) * 256 + Math.trunc(lum / 3);
  });
  return next;
}
