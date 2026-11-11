// The page of the demo (scripts/demo.mjs): the same loop for every version,
// one frame per animation frame, drawn into a canvas, with what it costs
// shown on it: frames per second, and the memory the page holds, the
// JavaScript heap plus, for Peras, the module's memory. The version is the
// `v` of the address: `immutable`, `typed` or `peras`; `n` particles.
const params = new URLSearchParams(location.search);
const version = params.get("v") ?? "peras";
const n = Number(params.get("n") ?? "200000");
const label = {
  immutable: "TypeScript, immutable objects",
  typed: "TypeScript, typed arrays by hand",
  peras: "Peras, WebAssembly",
}[version];
const width = 480;
const height = 360;
const canvas = document.querySelector("canvas");
canvas.width = width;
canvas.height = height;
const ctx = canvas.getContext("2d");
let seed = 11;
const random = () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 4294967296;
};
// A disk around the centre, each particle at the speed of a circular orbit,
// a little off: the same for every version.
const startX = new Float32Array(n);
const startY = new Float32Array(n);
const startVx = new Float32Array(n);
const startVy = new Float32Array(n);
for (let i = 0; i < n; i++) {
  const r = 8 + 150 * Math.sqrt(random());
  const a = random() * 2 * Math.PI;
  const v = Math.sqrt(0.6) * (0.9 + 0.2 * random());
  startX[i] = width / 2 + r * Math.cos(a);
  startY[i] = height / 2 + r * Math.sin(a);
  startVx[i] = -Math.sin(a) * v;
  startVy[i] = Math.cos(a) * v;
}

let step;
let wasmBytes = () => 0;
let image;
if (version === "peras") {
  const m = await import("./peras/particles.js");
  const heap = m.__perasHeap;
  // Made in the module's memory, then filled through a view: no copy in
  // between, so the memory grows only by what the particles need.
  const [xs, ys, vxs, vys] = [startX, startY, startVx, startVy].map((a) => {
    const array = heap.array("f32", n);
    array.view().set(a);
    return array;
  });
  const counts = heap.array("i32", width * height);
  const pixels = heap.array("i32", width * height);
  // The module's memory: all of it, whatever it holds.
  wasmBytes = () => pixels.view().buffer.byteLength;
  step = () => {
    m.frame(xs, ys, vxs, vys, counts, pixels, width, height);
    const view = pixels.view();
    image = new ImageData(
      new Uint8ClampedArray(view.buffer, view.byteOffset, view.length * 4),
      width,
      height,
    );
  };
} else if (version === "typed") {
  const m = await import("./ts/particles-typed.js");
  const [xs, ys, vxs, vys] = [startX, startY, startVx, startVy].map((a) => a.slice());
  const counts = new Int32Array(width * height);
  const pixels = new Uint32Array(width * height);
  image = new ImageData(new Uint8ClampedArray(pixels.buffer), width, height);
  step = () => m.frame(xs, ys, vxs, vys, counts, pixels, width, height);
} else {
  const m = await import("./ts/particles-immutable.js");
  let particles = m.start(startX, startY, startVx, startVy);
  const pixels = new Uint32Array(width * height);
  image = new ImageData(new Uint8ClampedArray(pixels.buffer), width, height);
  step = () => {
    particles = m.frame(particles, pixels, width, height);
  };
}

const heapBytes = () => performance.memory?.usedJSHeapSize ?? 0;
const frames = [];
const memory = [];
let last = performance.now();
const draw = () => {
  ctx.putImageData(image, 0, 0);
  // Frames per second over the last second, and the memory, in MiB.
  const now = performance.now();
  const recent = frames.filter((f) => f.at > now - 1000);
  const fps = recent.length;
  ctx.fillStyle = "rgba(0, 0, 0, 0.7)";
  ctx.fillRect(0, 0, width, 46);
  ctx.fillRect(0, height - 70, width, 70);
  ctx.fillStyle = "#fff";
  ctx.font = "bold 15px sans-serif";
  ctx.fillText(label, 10, 19);
  ctx.font = "13px sans-serif";
  const mib = (memory.at(-1) ?? 0) / 1048576;
  const spent = frames.at(-1)?.spent ?? 0;
  ctx.fillText(
    `${String(fps)} frames/s   ${spent.toFixed(1)} ms a frame   ${mib.toFixed(0)} MiB held`,
    10,
    38,
  );
  // The memory over the last 10 s, on the same scale for every version:
  // `mem` MiB at the top, from the address.
  const top = Number(params.get("mem") ?? "1500") * 1048576;
  ctx.strokeStyle = "#7fd17f";
  ctx.lineWidth = 2;
  ctx.beginPath();
  let moved = false;
  for (let i = 0; i < memory.length; i++) {
    const age = now - (frames[i]?.at ?? 0);
    if (age > 10000) continue;
    const x = width - 10 - (age / 10000) * (width - 20);
    const y = height - 8 - Math.min((memory[i] ?? 0) / top, 1) * 56;
    if (moved) ctx.lineTo(x, y);
    else ctx.moveTo(x, y);
    moved = true;
  }
  ctx.fillText(`${String(Math.round(top / 1048576))} MiB`, width - 70, height - 56);
  ctx.stroke();
};
const loop = () => {
  const t0 = performance.now();
  step();
  const spent = performance.now() - t0;
  frames.push({ at: t0, gap: t0 - last, spent });
  last = t0;
  memory.push(heapBytes() + wasmBytes());
  draw();
  requestAnimationFrame(loop);
};
globalThis.demo = { frames, memory };
requestAnimationFrame(loop);
