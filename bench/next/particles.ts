// Decision 0046, A3, a particle scene: particles born from a fountain, moving
// under gravity, colliding on a grid, dying, and drawn into a framebuffer of
// RGBA pixels each frame. The specification, written first; the Peras version
// translates it.

export interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  hue: number;
}

export class Scene {
  particles: Particle[] = [];
  pixels: Uint8ClampedArray;
  seed = 1;

  constructor(
    readonly width: number,
    readonly height: number,
  ) {
    this.pixels = new Uint8ClampedArray(width * height * 4);
  }

  random(): number {
    this.seed = (this.seed * 1103515245 + 12345) % 2147483648;
    return this.seed / 2147483648;
  }

  /** One frame: births, motion, collisions on a grid, deaths, then the drawing. */
  step(): void {
    for (let i = 0; i < 40; i++) {
      this.particles.push({
        x: this.width / 2,
        y: this.height - 1,
        vx: (this.random() - 0.5) * 4,
        vy: -2 - this.random() * 4,
        life: 60 + Math.floor(this.random() * 60),
        hue: this.random(),
      });
    }
    const cell = 8;
    const cols = Math.ceil(this.width / cell);
    const grid = new Map<number, number[]>();
    for (let i = 0; i < this.particles.length; i++) {
      const p = this.particles[i]!;
      p.vy += 0.08;
      p.x += p.vx;
      p.y += p.vy;
      if (p.x < 0 || p.x >= this.width) p.vx = -p.vx;
      if (p.y >= this.height) {
        p.y = this.height - 1;
        p.vy = -p.vy * 0.6;
      }
      p.life -= 1;
      const key = Math.floor(p.y / cell) * cols + Math.floor(p.x / cell);
      const list = grid.get(key);
      if (list) list.push(i);
      else grid.set(key, [i]);
    }
    for (const list of grid.values()) {
      if (list.length < 2) continue;
      for (const i of list) {
        const p = this.particles[i]!;
        p.vx *= 0.98;
        p.life -= 1;
      }
    }
    this.particles = this.particles.filter((p) => p.life > 0);
    this.draw();
  }

  draw(): void {
    this.pixels.fill(0);
    for (const p of this.particles) {
      const x = Math.floor(p.x);
      const y = Math.floor(p.y);
      if (x < 0 || y < 0 || x >= this.width || y >= this.height) continue;
      const o = (y * this.width + x) * 4;
      const bright = Math.min(1, p.life / 60);
      this.pixels[o] = 255 * bright;
      this.pixels[o + 1] = 160 * p.hue * bright;
      this.pixels[o + 2] = 60;
      this.pixels[o + 3] = 255;
    }
  }
}

/** The workload: frames of a scene, then a digest of its pixels and particles. */
export function run(frames: number): string {
  const scene = new Scene(320, 200);
  for (let f = 0; f < frames; f++) scene.step();
  let sum = 0;
  for (let i = 0; i < scene.pixels.length; i++) sum = (sum * 31 + scene.pixels[i]!) % 1000000007;
  return `${scene.particles.length}|${sum}`;
}
