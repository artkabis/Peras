# A3, the particle scene: runs of perasc check (runs/particles_*.json)

f0 is decision 0046's version, `bench/next/particles.peras`, its runs in `bench/next/ease/particles.md`.

## f1, an `Array<u32>`

The floor measured before the protocol was registered (decision 0047, Context): f0 with its frame one word a pixel, cleared as 64,000 words. Builds; same result as TypeScript, twice on one instance.

## f2, a `Framebuffer` imported

Written from f0, its frame a `Framebuffer`, `clear` and `setPixel`, `clamp8` gone.
run 0, 1 error: E0207 `setPixel(&self.frame, …)` inside `for (const p of self.particles)`: `self` changing while a loop goes through it, where `self.pixels[o] = …` in the same place was accepted; the fix offered, `self.copy()`, copies the scene each frame. A bug of perasc, F126, fixed: a loop keeps the path of fields it goes through.
run 1: builds; same result as TypeScript, twice on one instance.
