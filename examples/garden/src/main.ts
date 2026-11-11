// The playground: the same window of the garden twice, as the stored image
// shows it and as the program draws it. Every figure comes from the measured
// results (bench/garden/results.json), none is typed here.
import results from "../../../bench/garden/results.json";
import storedUrl from "../../../docs/assets/garden-stored.avif?url";
import type { Frame, Request } from "./render.worker.ts";
import {
  clamp,
  fromHash,
  home,
  level,
  pan,
  placed,
  toHash,
  window,
  zoomAt,
  type View,
} from "./view.ts";

// English first, then French, as everywhere in Peras.
const texts = {
  en: {
    title: "The garden of Apeiron",
    intro:
      "The same garden twice. On the left, the smallest image that keeps it faithful at zoom 1, enlarged as you zoom. On the right, a program smaller than that image, drawing every frame again. Drag to move, scroll or pinch to zoom.",
    storedTitle: "The stored image",
    storedMeta: (bytes: string, crf: number) => `AVIF · ${bytes} bytes · CRF ${String(crf)}`,
    drawnTitle: "The program",
    drawnMeta: (bytes: string) => `Peras → WebAssembly · ${bytes} bytes`,
    home: "Whole garden",
    visit: "Zoom ×64 into the meadow",
    zoomIn: "Zoom in",
    zoomOut: "Zoom out",
    readout: (zoom: string, octaves: number, ms: string, side: number) =>
      `×${zoom} · ${String(octaves)} octaves · ${ms} ms for ${String(side)}² px`,
    drawing: (zoom: string, octaves: number) => `×${zoom} · ${String(octaves)} octaves · drawing…`,
    notes: [
      "Both sizes are measured: the program with its loader, minified and gzip, and the AVIF as the benchmark chose it, the smallest at SSIM 0.95 against the render.",
      "The program draws the same pixels, bit for bit, as the TypeScript and Rust versions of the benchmark. It runs in a Web Worker; a quarter-resolution frame comes first, then the full one.",
      "The image on the left is enlarged by the browser's smoothing; the benchmark's figures used Lanczos. Either way, the image holds no more detail than it stored.",
      "What this does not say: TypeScript ships the same program five times smaller, and for a scene nobody zooms into, the AVIF is the better choice.",
    ],
    more: "Read the study",
    moreHref: "https://github.com/artkabis/Peras/blob/main/docs/en/garden.md",
  },
  fr: {
    title: "Le jardin d’Apeiron",
    intro:
      "Le même jardin deux fois. À gauche, la plus petite image qui le garde fidèle au zoom 1, agrandie à mesure qu’on zoome. À droite, un programme plus petit que cette image, qui redessine chaque vue. Glissez pour vous déplacer, molette ou pincement pour zoomer.",
    storedTitle: "L’image stockée",
    storedMeta: (bytes: string, crf: number) => `AVIF · ${bytes} octets · CRF ${String(crf)}`,
    drawnTitle: "Le programme",
    drawnMeta: (bytes: string) => `Peras → WebAssembly · ${bytes} octets`,
    home: "Tout le jardin",
    visit: "Zoomer ×64 dans la prairie",
    zoomIn: "Zoomer",
    zoomOut: "Dézoomer",
    readout: (zoom: string, octaves: number, ms: string, side: number) =>
      `×${zoom} · ${String(octaves)} octaves · ${ms} ms pour ${String(side)}² px`,
    drawing: (zoom: string, octaves: number) => `×${zoom} · ${String(octaves)} octaves · dessin…`,
    notes: [
      "Les deux tailles sont mesurées : le programme avec son chargeur, minifié et gzip, et l’AVIF tel que le banc l’a choisi, le plus petit à un SSIM de 0,95 face au rendu.",
      "Le programme dessine les mêmes pixels, bit pour bit, que les versions TypeScript et Rust du banc. Il tourne dans un Web Worker ; une vue au quart de la résolution vient d’abord, puis la vue entière.",
      "L’image de gauche est agrandie par le lissage du navigateur ; les chiffres du banc ont utilisé Lanczos. Dans les deux cas, l’image ne contient pas plus de détail qu’elle n’en a stocké.",
      "Ce que cela ne dit pas : TypeScript livre le même programme cinq fois plus petit, et pour une scène que personne ne zoome, l’AVIF reste le meilleur choix.",
    ],
    more: "Lire l’étude",
    moreHref: "https://github.com/artkabis/Peras/blob/main/docs/fr/garden.md",
  },
};
const asked = new URLSearchParams(location.search).get("lang");
const lang =
  asked === "fr" || asked === "en" ? asked : navigator.language.startsWith("fr") ? "fr" : "en";
const t = texts[lang];
const numbers = new Intl.NumberFormat(lang);
document.documentElement.lang = lang;
document.title = t.title;

function element<T extends HTMLElement>(id: string, kind: new () => T): T {
  const found = document.getElementById(id);
  if (!(found instanceof kind)) throw new Error(`missing #${id}`);
  return found;
}

// The figures, from the benchmark's results.
const avif = results.h1Avif.reduce((a, b) => (b.bytes < a.bytes ? b : a));
element("title", HTMLElement).textContent = t.title;
element("intro", HTMLElement).textContent = t.intro;
element("stored-title", HTMLElement).textContent = t.storedTitle;
element("stored-meta", HTMLElement).textContent = t.storedMeta(
  numbers.format(avif.bytes),
  avif.crf,
);
element("drawn-title", HTMLElement).textContent = t.drawnTitle;
element("drawn-meta", HTMLElement).textContent = t.drawnMeta(numbers.format(results.shipped.peras));
element("home", HTMLButtonElement).textContent = t.home;
element("visit", HTMLButtonElement).textContent = t.visit;
element("in", HTMLButtonElement).setAttribute("aria-label", t.zoomIn);
element("out", HTMLButtonElement).setAttribute("aria-label", t.zoomOut);
const notes = element("notes", HTMLUListElement);
for (const note of t.notes) {
  const item = document.createElement("li");
  item.textContent = note;
  notes.append(item);
}
const more = document.createElement("a");
more.href = t.moreHref;
more.textContent = t.more;
const last = document.createElement("li");
last.append(more);
notes.append(last);

const storedCanvas = element("stored", HTMLCanvasElement);
const drawnCanvas = element("drawn", HTMLCanvasElement);
const readout = element("readout", HTMLOutputElement);
function context(canvas: HTMLCanvasElement): CanvasRenderingContext2D {
  const ctx = canvas.getContext("2d");
  if (ctx === null) throw new Error("no 2D canvas");
  return ctx;
}
const storedCtx = context(storedCanvas);
const drawnCtx = context(drawnCanvas);

const stored = new Image();
stored.src = storedUrl;
stored.addEventListener("load", () => {
  paint();
});

// The state: the view asked for, its number, and the best frame drawn so far.
let view: View = fromHash(location.hash);
let id = 0;
let side = 0;
const frame = document.createElement("canvas");
let shown: { id: number; side: number; view: View; ms: number } | undefined;

/** A worker that draws one frame at a time; a request made while it draws replaces the waiting one. */
interface Slot {
  worker: Worker;
  busy: boolean;
  waiting: Request | undefined;
}
function slot(): Slot {
  const worker = new Worker(new URL("./render.worker.ts", import.meta.url), { type: "module" });
  const s: Slot = { worker, busy: false, waiting: undefined };
  worker.addEventListener("message", (event: MessageEvent<Frame>) => {
    s.busy = false;
    const next = s.waiting;
    s.waiting = undefined;
    if (next !== undefined) ask(s, next);
    receive(event.data);
  });
  return s;
}
function ask(s: Slot, request: Request): void {
  if (s.busy) {
    s.waiting = request;
    return;
  }
  s.busy = true;
  s.worker.postMessage(request);
}
// A quick worker for a quarter-resolution frame, a fine one for the full frame.
const quick = slot();
const fine = slot();

function receive(f: Frame): void {
  if (shown !== undefined && (f.id < shown.id || (f.id === shown.id && f.side <= shown.side)))
    return;
  frame.width = f.side;
  frame.height = f.side;
  const ctx = frame.getContext("2d");
  if (ctx === null) return;
  ctx.putImageData(new ImageData(new Uint8ClampedArray(f.pixels), f.side, f.side), 0, 0);
  shown = { id: f.id, side: f.side, view: { zoom: f.zoom, cx: f.cx, cy: f.cy }, ms: f.ms };
  paint();
  if (f.id === id && f.side === side) {
    // The full frame of the view asked for: a test reads exactly what it shows.
    document.body.dataset.frame = JSON.stringify({ side: f.side, ...view });
    document.body.dataset.ready = "true";
    history.replaceState(null, "", location.search + toHash(view));
  }
}

let painting = false;
function paint(): void {
  if (painting) return;
  painting = true;
  requestAnimationFrame(() => {
    painting = false;
    // The stored image: the window of it the view shows, enlarged.
    storedCtx.imageSmoothingEnabled = true;
    storedCtx.imageSmoothingQuality = "high";
    if (stored.complete && stored.naturalWidth > 0) {
      const w = window(view, stored.naturalWidth);
      storedCtx.drawImage(stored, w.x, w.y, w.size, w.size, 0, 0, side, side);
    }
    // The program: its best frame, moved where the view now puts it.
    if (shown !== undefined) {
      const exact = shown.id === id && shown.side === side;
      drawnCtx.imageSmoothingEnabled = !exact;
      if (!exact) drawnCtx.clearRect(0, 0, side, side);
      const p = placed(shown.view, view, side);
      drawnCtx.drawImage(frame, p.x, p.y, p.size, p.size);
    }
    const zoom = numbers.format(Math.round(view.zoom * 10) / 10);
    const octaves = 6 + level(view.zoom);
    readout.textContent =
      shown?.id === id && shown.side === side
        ? t.readout(zoom, octaves, shown.ms.toFixed(0), side)
        : t.drawing(zoom, octaves);
  });
}

/** Shows `next`: the old frame moved at once, then new frames asked for. */
function go(next: View): void {
  view = clamp(next);
  id += 1;
  paint();
  const request = { id, zoom: view.zoom, cx: view.cx, cy: view.cy };
  ask(quick, { ...request, side: Math.max(64, Math.round(side / 4)) });
  ask(fine, { ...request, side });
}

// The panels' side in device pixels, at most 1024: the size of the measured renders.
function resize(): void {
  const css = drawnCanvas.getBoundingClientRect().width;
  const next = Math.min(1024, Math.max(64, Math.round(css * devicePixelRatio)));
  if (next === side) return;
  side = next;
  for (const canvas of [storedCanvas, drawnCanvas]) {
    canvas.width = side;
    canvas.height = side;
  }
  go(view);
}
new ResizeObserver(resize).observe(drawnCanvas);

// Moving: a drag pans, two fingers pinch, the wheel zooms at the cursor.
const pointers = new Map<number, { x: number; y: number }>();
function fraction(canvas: HTMLCanvasElement, x: number, y: number): [number, number] {
  const box = canvas.getBoundingClientRect();
  return [(x - box.left) / box.width, (y - box.top) / box.height];
}
for (const canvas of [storedCanvas, drawnCanvas]) {
  canvas.addEventListener("pointerdown", (event) => {
    canvas.setPointerCapture(event.pointerId);
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
  });
  canvas.addEventListener("pointermove", (event) => {
    const before = pointers.get(event.pointerId);
    if (before === undefined) return;
    const box = canvas.getBoundingClientRect();
    const others = [...pointers].filter(([key]) => key !== event.pointerId).map(([, p]) => p);
    const other = others[0];
    let next: View;
    if (other === undefined) {
      next = pan(
        view,
        (event.clientX - before.x) / box.width,
        (event.clientY - before.y) / box.height,
      );
    } else {
      // Pinch: the distance between the fingers scales, their middle moves.
      const d0 = Math.hypot(before.x - other.x, before.y - other.y);
      const d1 = Math.hypot(event.clientX - other.x, event.clientY - other.y);
      const [u, v] = fraction(canvas, (event.clientX + other.x) / 2, (event.clientY + other.y) / 2);
      next = pan(
        zoomAt(view, d0 > 0 ? d1 / d0 : 1, u, v),
        (event.clientX - before.x) / 2 / box.width,
        (event.clientY - before.y) / 2 / box.height,
      );
    }
    pointers.set(event.pointerId, { x: event.clientX, y: event.clientY });
    go(next);
  });
  const up = (event: PointerEvent): void => {
    pointers.delete(event.pointerId);
  };
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", up);
  canvas.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const [u, v] = fraction(canvas, event.clientX, event.clientY);
      const step = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? 40 : 1;
      go(zoomAt(view, 2 ** ((-event.deltaY * step) / 400), u, v));
    },
    { passive: false },
  );
  canvas.addEventListener("dblclick", (event) => {
    const [u, v] = fraction(canvas, event.clientX, event.clientY);
    go(zoomAt(view, event.shiftKey ? 0.5 : 2, u, v));
  });
  canvas.addEventListener("keydown", (event) => {
    const moves: Record<string, () => View> = {
      "+": () => zoomAt(view, 2, 0.5, 0.5),
      "=": () => zoomAt(view, 2, 0.5, 0.5),
      "-": () => zoomAt(view, 0.5, 0.5, 0.5),
      ArrowLeft: () => pan(view, 0.1, 0),
      ArrowRight: () => pan(view, -0.1, 0),
      ArrowUp: () => pan(view, 0, 0.1),
      ArrowDown: () => pan(view, 0, -0.1),
      "0": () => home,
    };
    const move = moves[event.key];
    if (move === undefined) return;
    event.preventDefault();
    go(move());
  });
}
element("in", HTMLButtonElement).addEventListener("click", () => {
  go(zoomAt(view, 2, 0.5, 0.5));
});
element("out", HTMLButtonElement).addEventListener("click", () => {
  go(zoomAt(view, 0.5, 0.5, 0.5));
});
element("home", HTMLButtonElement).addEventListener("click", () => {
  go(home);
});
// The window of the before-and-after image: the meadow at (0.37, 0.83), ×64.
element("visit", HTMLButtonElement).addEventListener("click", () => {
  go({ zoom: 64, cx: 0.37, cy: 0.83 });
});
addEventListener("hashchange", () => {
  go(fromHash(location.hash));
});
