// Decision 0047, P2, a full frame: every pixel of 640 by 400 computed each
// frame, a plasma drawn from a sine per column and per row, a palette of 256
// colours, and a shade that rounds as a Uint8ClampedArray rounds. The
// specification, written first; the Peras versions translate it.

export class Plasma {
  readonly pixels: Uint8ClampedArray;
  readonly palette: Uint8ClampedArray;
  readonly columns: Float64Array;
  readonly rows: Float64Array;

  constructor(
    readonly width: number,
    readonly height: number,
  ) {
    this.pixels = new Uint8ClampedArray(width * height * 4);
    this.palette = new Uint8ClampedArray(256 * 3);
    for (let i = 0; i < 256; i++) {
      const a = (i / 256) * Math.PI * 2;
      this.palette[i * 3] = 128 + 127 * Math.sin(a);
      this.palette[i * 3 + 1] = 128 + 127 * Math.sin(a + 2.094);
      this.palette[i * 3 + 2] = 128 + 127 * Math.sin(a + 4.188);
    }
    this.columns = new Float64Array(width);
    this.rows = new Float64Array(height);
  }

  draw(t: number): void {
    const { width, height, pixels, palette, columns, rows } = this;
    for (let x = 0; x < width; x++)
      columns[x] = Math.sin(x * 0.031 + t) + Math.sin(x * 0.013 - t * 0.5);
    for (let y = 0; y < height; y++)
      rows[y] = Math.sin(y * 0.047 - t * 0.7) + Math.cos(y * 0.019 + t * 0.3);
    for (let y = 0; y < height; y++) {
      const row = rows[y]!;
      const shade = 0.75 + 0.25 * Math.sin(y * 0.1 + t);
      for (let x = 0; x < width; x++) {
        const v = columns[x]! + row;
        const c = Math.floor(v * 32 + 512) & 255;
        const o = (y * width + x) * 4;
        pixels[o] = palette[c * 3]! * shade;
        pixels[o + 1] = palette[c * 3 + 1]! * shade;
        pixels[o + 2] = palette[c * 3 + 2]! * shade;
        pixels[o + 3] = 255;
      }
    }
  }
}

export function run(frames: number): string {
  const plasma = new Plasma(640, 400);
  for (let f = 0; f < frames; f++) plasma.draw(f * 0.05);
  let sum = 0;
  for (let i = 0; i < plasma.pixels.length; i++) sum = (sum * 31 + plasma.pixels[i]!) % 1000000007;
  return `${frames}|${sum}`;
}
