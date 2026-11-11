// The demo in TypeScript, as an expert writes it for speed: typed arrays,
// plain loops, nothing allocated after the start.
export function frame(
  xs: Float32Array,
  ys: Float32Array,
  vxs: Float32Array,
  vys: Float32Array,
  counts: Int32Array,
  pixels: Uint32Array,
  width: number,
  height: number,
): number {
  const cx = width / 2;
  const cy = height / 2;
  counts.fill(0);
  let drawn = 0;
  for (let i = 0; i < xs.length; i++) {
    const dx = cx - (xs[i] ?? 0);
    const dy = cy - (ys[i] ?? 0);
    const f = 0.6 / (dx * dx + dy * dy + 30);
    const vx = (vxs[i] ?? 0) + dx * f;
    const vy = (vys[i] ?? 0) + dy * f;
    const x = (xs[i] ?? 0) + vx;
    const y = (ys[i] ?? 0) + vy;
    xs[i] = x;
    ys[i] = y;
    vxs[i] = vx;
    vys[i] = vy;
    if (x >= 0 && y >= 0 && x < width && y < height) {
      const at = Math.trunc(y) * width + Math.trunc(x);
      counts[at] = (counts[at] ?? 0) + 1;
      drawn += 1;
    }
  }
  for (let i = 0; i < pixels.length; i++) {
    const lum = Math.min((counts[i] ?? 0) * 5, 255);
    pixels[i] = 0xff000000 + lum * 65536 + Math.trunc((lum * 3) / 4) * 256 + Math.trunc(lum / 3);
  }
  return drawn;
}
