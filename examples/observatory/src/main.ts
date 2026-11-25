// The observatory of Peras (docs/en/observatory.md): decision 0047's full
// frame drawn live by TypeScript and by Peras side by side, the visitor's
// measure, and the figures the repository has recorded, read from its
// results files when the page is built.
import { Plasma } from "../../../bench/frame/plasma.ts";
import { __perasHeap, draw } from "../../../bench/frame/canvas-f3.peras";
import nextResults from "../../../bench/next/RESULTS.md?raw";
import frameResults from "../../../bench/frame/RESULTS.md?raw";
import bugResults from "../../../bench/web-bugs/RESULTS.md?raw";
import streamResults from "../../../bench/stream/RESULTS.md?raw";
import candleResults from "../../../bench/candle/RESULTS.md?raw";
import demoResults from "../../../bench/demo/RESULTS.md?raw";
import browserResults from "../../../bench/browser/RESULTS.md?raw";
import { measure, summary, type Rounds } from "./measure.ts";
import {
  applications,
  bugs,
  candle,
  demo,
  frames,
  kernels,
  machine,
  stream,
  tally,
  type Measured,
  type Stop,
} from "./recorded.ts";

const W = 640;
const H = 400;
const fr =
  new URLSearchParams(location.search).get("lang") === "fr" ||
  (!location.search.includes("lang=") && navigator.language.startsWith("fr"));
const T = fr
  ? {
      title: "L’observatoire de Peras",
      lead: "La même image, dessinée en ce moment par votre navigateur deux fois : à gauche par TypeScript, à droite par Peras compilé en WebAssembly. Mêmes pixels, vérifiés pendant qu’elle tourne. Sous chacune, une bougie brûle comme le temps de calcul dépensé. Puis mesurez sur votre machine, et voyez tout ce que le dépôt a mesuré, le bon comme le moins bon.",
      pause: "Pause",
      play: "Reprendre",
      same: (n: number) => `mêmes pixels, ${String(n)} vérifications`,
      differ: "les pixels diffèrent",
      perFrame: (ms: number) => `${ms.toFixed(2)} ms par image`,
      spent: (s: number) => `${s.toFixed(1)} s de calcul`,
      sceneNote:
        "Les temps de la scène comprennent le dessin des deux canvas par la page : ce sont une image, pas une mesure. La mesure est ci-dessous.",
      measureTitle: "Mesurer sur votre machine",
      measureLead:
        "La scène s’arrête ; TypeScript, Peras, et TypeScript de nouveau (le contrôle A/A, qui mesure le bruit) dessinent 21 tours de 20 images sans canvas, l’ordre alterné à chaque tour.",
      measure: "Mesurer",
      save: "Enregistrer en JSON",
      round: (n: number) => `tour ${String(n)} sur 21`,
      version: "Version",
      ratio: "Temps / TypeScript, médiane (étendue)",
      ms: "ms par image, médiane",
      noisy: "contrôle hors de 0,95 à 1,05 : machine trop bruitée, le rapport ne décide rien",
      quiet: "contrôle dans 0,95 à 1,05",
      againName: "TypeScript, de nouveau (A/A)",
      mapTitle: "Tout ce qui a été mesuré, d’un seul regard",
      againstUsual: "Face au TypeScript tel qu’on l’écrit d’habitude",
      againstTuned: "Face au TypeScript optimisé à la main, ou écrit par un expert",
      mapLead:
        "Le temps de Peras sur celui de TypeScript, échelle logarithmique. À gauche de 1, Peras est plus rapide ; à droite, plus lent. Lu dans les fichiers de résultats du dépôt quand cette page a été construite.",
      faster: "Peras plus rapide",
      equal: "égal, dans le bruit (0,95 à 1,05)",
      slower: "Peras plus lent",
      before: "avant la décision 0047",
      band: "bande du contrôle A/A",
      measuredOn: "Mesuré sur",
      bugsTitle: "Les bogues arrêtés",
      bugsLead:
        "Dix-sept bogues d’applications web, écrits comme on les écrit naturellement, et le premier moment où chaque chaîne d’outils les arrête : TypeScript à son plus strict avec typescript-eslint, face à perasc.",
      stops: {
        compile: "compilation",
        run: "exécution",
        silent: "en silence",
        none: "pas un bogue",
      },
      tally: (ts: Record<Stop, number>, p: Record<Stop, number>) =>
        `Arrêtés à la compilation : TypeScript ${String(ts.compile)}, Peras ${String(p.compile)} ; à l’exécution : ${String(ts.run)} et ${String(p.run)} ; en silence : ${String(ts.silent)} et ${String(p.silent)}.`,
      footer:
        "Chaque chiffre a son protocole, enregistré avant la mesure, et ses scripts dans le dépôt. Votre machine n’est pas la nôtre : vos rapports sont les vôtres.",
    }
  : {
      title: "The observatory of Peras",
      lead: "The same frame, drawn right now by your browser twice: on the left by TypeScript, on the right by Peras compiled to WebAssembly. The same pixels, checked as it runs. Under each, a candle burns as the time it spent computing. Then measure on your machine, and see everything the repository has measured, the good and the less good.",
      pause: "Pause",
      play: "Resume",
      same: (n: number) => `same pixels, ${String(n)} checks`,
      differ: "the pixels differ",
      perFrame: (ms: number) => `${ms.toFixed(2)} ms a frame`,
      spent: (s: number) => `${s.toFixed(1)} s computing`,
      sceneNote:
        "The scene's times include the page drawing both canvases: they are a picture, not a measure. The measure is below.",
      measureTitle: "Measure on your machine",
      measureLead:
        "The scene stops; TypeScript, Peras, and TypeScript again (the A/A control, which measures the noise) draw 21 rounds of 20 frames with no canvas, the order alternating round by round.",
      measure: "Measure",
      save: "Save as JSON",
      round: (n: number) => `round ${String(n)} of 21`,
      version: "Version",
      ratio: "Time / TypeScript, median (range)",
      ms: "ms a frame, median",
      noisy: "control outside 0.95 to 1.05: the machine was too noisy, the ratio decides nothing",
      quiet: "control within 0.95 to 1.05",
      againName: "TypeScript, again (A/A)",
      mapTitle: "Everything measured, at one glance",
      againstUsual: "Against TypeScript as it is usually written",
      againstTuned: "Against TypeScript tuned by hand, or written by an expert",
      mapLead:
        "Peras's time over TypeScript's, on a scale of logarithms. Left of 1, Peras is faster; right of it, slower. Read from the repository's results files when this page was built.",
      faster: "Peras faster",
      equal: "equal, within the noise (0.95 to 1.05)",
      slower: "Peras slower",
      before: "before decision 0047",
      band: "the A/A control's band",
      measuredOn: "Measured on",
      bugsTitle: "Bugs stopped",
      bugsLead:
        "Seventeen bugs of web applications, written as they are naturally written, and the earliest point where each toolchain stops them: TypeScript at its strictest with typescript-eslint, against perasc.",
      stops: { compile: "compile", run: "run", silent: "silent", none: "no bug" },
      tally: (ts: Record<Stop, number>, p: Record<Stop, number>) =>
        `Stopped at compile time: TypeScript ${String(ts.compile)}, Peras ${String(p.compile)}; at run time: ${String(ts.run)} and ${String(p.run)}; in silence: ${String(ts.silent)} and ${String(p.silent)}.`,
      footer:
        "Every figure has its protocol, registered before the measure, and its scripts in the repository. Your machine is not ours: your ratios are yours.",
    };
document.documentElement.lang = fr ? "fr" : "en";

const $ = (id: string): HTMLElement => {
  const e = document.getElementById(id);
  if (e === null) throw new Error(`#${id} is missing`);
  return e;
};
const text = (id: string, s: string) => {
  $(id).textContent = s;
};
text("title", T.title);
text("lead", T.lead);
text("scene-note", T.sceneNote);
text("measure-title", T.measureTitle);
text("measure-lead", T.measureLead);
text("measure", T.measure);
text("save", T.save);
text("map-title", T.mapTitle);
text("map-lead", T.mapLead);
text("bugs-title", T.bugsTitle);
text("bugs-lead", T.bugsLead);
text("footer", T.footer);

// --- The scene --------------------------------------------------------------

const tsCtx = ($("ts") as HTMLCanvasElement).getContext("2d");
const perasCtx = ($("peras") as HTMLCanvasElement).getContext("2d");
if (tsCtx === null || perasCtx === null) throw new Error("no 2d canvas");
const plasma = new Plasma(W, H);
const lent = __perasHeap.array("u32", W * H);
const perasBytes = () => {
  const v = lent.view();
  // A view of the module's memory, an ArrayBuffer (no threads here).
  return new Uint8ClampedArray(v.buffer as ArrayBuffer, v.byteOffset, v.byteLength);
};

const recent = { ts: [] as number[], peras: [] as number[] };
const spent = { ts: 0, peras: 0 };
let frameNo = 0;
let checks = 0;
let differ = false;
let running = true;

const drawTs = (f: number) => {
  const t0 = performance.now();
  plasma.draw(f * 0.05);
  const ms = performance.now() - t0;
  tsCtx.putImageData(new ImageData(plasma.pixels as Uint8ClampedArray<ArrayBuffer>, W, H), 0, 0);
  return ms;
};
const drawPeras = (f: number) => {
  const t0 = performance.now();
  draw(lent, W, H, f * 0.05);
  const ms = performance.now() - t0;
  perasCtx.putImageData(new ImageData(perasBytes(), W, H), 0, 0);
  return ms;
};
const keep = (xs: number[], x: number) => {
  xs.push(x);
  if (xs.length > 120) xs.shift();
};

function tick() {
  if (!running) return;
  const f = frameNo++;
  // The one that starts alternates frame by frame.
  let a: number;
  let b: number;
  if (f % 2 === 0) {
    a = drawTs(f);
    b = drawPeras(f);
  } else {
    b = drawPeras(f);
    a = drawTs(f);
  }
  keep(recent.ts, a);
  keep(recent.peras, b);
  spent.ts += a;
  spent.peras += b;
  if (f % 30 === 0) {
    const p = perasBytes();
    const t = plasma.pixels;
    let same = p.length === t.length;
    for (let i = 0; same && i < t.length; i++) if (p[i] !== t[i]) same = false;
    if (!same) differ = true;
    checks++;
  }
  if (f % 10 === 0) show();
  requestAnimationFrame(tick);
}

function show() {
  text("ts-stats", T.perFrame(summary(recent.ts).median));
  text("peras-stats", T.perFrame(summary(recent.peras).median));
  text("ts-spent", T.spent(spent.ts / 1000));
  text("peras-spent", T.spent(spent.peras / 1000));
  const most = Math.max(spent.ts, spent.peras, 1);
  // The wax left: all of it for no time spent, a fifth for the larger time.
  for (const who of ["ts", "peras"] as const) {
    const wax = Math.max(6, 100 - (80 * spent[who]) / most);
    $(`${who}-wax`).style.height = `${String(wax)}%`;
    $(`${who}-flame`).style.bottom = `${String(wax)}%`;
  }
  const same = $("same");
  same.textContent = differ ? T.differ : T.same(checks);
  same.className = `same ${differ ? "ko" : "ok"}`;
  document.body.dataset.frames = String(frameNo);
  document.body.dataset.same = String(!differ && checks > 0);
}

const pause = $("pause") as HTMLButtonElement;
pause.textContent = T.pause;
pause.addEventListener("click", () => {
  running = !running;
  pause.textContent = running ? T.pause : T.play;
  if (running) requestAnimationFrame(tick);
});

// --- The visitor's measure --------------------------------------------------

let last: Rounds | undefined;
const button = $("measure") as HTMLButtonElement;
const save = $("save") as HTMLButtonElement;
button.addEventListener("click", () => {
  void (async () => {
    const wasRunning = running;
    running = false;
    button.disabled = true;
    const again = new Plasma(W, H);
    last = await measure(
      [
        {
          name: "TypeScript",
          draw: (f) => {
            plasma.draw(f * 0.05);
          },
        },
        {
          name: "Peras",
          draw: (f) => {
            draw(lent, W, H, f * 0.05);
          },
        },
        {
          name: T.againName,
          draw: (f) => {
            again.draw(f * 0.05);
          },
        },
      ],
      21,
      20,
      () => performance.now(),
      async (n) => {
        text("progress", T.round(n));
        await new Promise((r) => setTimeout(r, 0));
      },
    );
    const table = $("measured") as HTMLTableElement;
    table.hidden = false;
    table.replaceChildren();
    const head = table.insertRow();
    for (const h of [T.version, T.ratio, T.ms]) {
      const th = document.createElement("th");
      th.textContent = h;
      head.append(th);
    }
    for (const [name, ratios] of Object.entries(last.ratios)) {
      const r = summary(ratios);
      const ms = summary((last.times[name] ?? []).map((t) => t / 20));
      const row = table.insertRow();
      row.insertCell().textContent = name;
      const c = row.insertCell();
      c.className = "n";
      c.textContent = `${r.median.toFixed(3)} (${r.low.toFixed(3)} – ${r.high.toFixed(3)})`;
      const d = row.insertCell();
      d.className = "n";
      d.textContent = ms.median.toFixed(2);
    }
    const control = summary(last.ratios[T.againName] ?? []).median;
    text(
      "progress",
      `${/(Chrome|Firefox|Safari)\/[\d.]+/.exec(navigator.userAgent)?.[0] ?? ""}, ${String(navigator.hardwareConcurrency)} · ${control >= 0.95 && control <= 1.05 ? T.quiet : T.noisy}`,
    );
    document.body.dataset.measured = "true";
    button.disabled = false;
    save.disabled = false;
    running = wasRunning;
    if (running) requestAnimationFrame(tick);
  })();
});
save.addEventListener("click", () => {
  if (last === undefined) return;
  const blob = new Blob(
    [
      JSON.stringify(
        {
          protocol: "docs/en/observatory.md",
          browser: navigator.userAgent,
          processors: navigator.hardwareConcurrency,
          rounds: 21,
          frames: 20,
          ...last,
        },
        null,
        2,
      ),
    ],
    { type: "application/json" },
  );
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = "peras-observatory.json";
  a.click();
  URL.revokeObjectURL(a.href);
});

// --- The recorded figures ---------------------------------------------------

// Each figure against the TypeScript it was measured against: as it is
// usually written, then tuned by hand, since they are not the same rival.
const recorded: Measured[] = [
  ...stream(streamResults).map((m) => ({ ...m, label: `${m.label} · 0015` })),
  ...candle(candleResults).map((m) => ({ ...m, label: `${m.label} · 0013` })),
  ...demo(demoResults),
  ...frames(frameResults).map((m) => ({ ...m, label: `${m.label} · 0047` })),
  // A3 is decision 0047's row, its figure before 0047 drawn on it.
  ...applications(nextResults)
    .filter((m) => !m.label.startsWith("A3"))
    .map((m) => ({ ...m, label: `${m.label} · 0046` })),
  ...kernels(browserResults),
];
type Row = Measured | { heading: string };
const rows: Row[] = [
  { heading: T.againstUsual },
  ...recorded.filter((m) => (m.against ?? "usual") === "usual"),
  { heading: T.againstTuned },
  ...recorded.filter((m) => m.against === "tuned"),
];
const svgNs = "http://www.w3.org/2000/svg";
function mapChart() {
  const svg = $("map") as unknown as SVGSVGElement;
  const rowH = 34;
  const left = 260;
  const width = 860;
  const height = 40 + rows.length * rowH + 30;
  svg.setAttribute("viewBox", `0 0 ${String(width)} ${String(height)}`);
  svg.setAttribute("width", String(width));
  svg.setAttribute("aria-label", T.mapTitle);
  const lo = Math.log(0.0625);
  const hi = Math.log(8);
  const x = (v: number) => left + ((Math.log(v) - lo) / (hi - lo)) * (width - left - 20);
  const el = (name: string, attrs: Record<string, string | number>, body?: string) => {
    const e = document.createElementNS(svgNs, name);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
    if (body !== undefined) e.textContent = body;
    svg.append(e);
    return e;
  };
  for (const v of [0.0625, 0.125, 0.25, 0.5, 1, 2, 4, 8]) {
    el("line", { x1: x(v), x2: x(v), y1: 24, y2: height - 24, class: v === 1 ? "one" : "axis" });
    const tick = v < 0.2 ? `1/${String(1 / v)}` : String(v);
    el("text", { x: x(v), y: 16, "text-anchor": "middle" }, tick);
  }
  el("text", { x: x(0.0625), y: height - 6 }, `← ${T.faster}`);
  el("text", { x: x(8), y: height - 6, "text-anchor": "end" }, `${T.slower} →`);
  rows.forEach((row, i) => {
    const y = 40 + i * rowH;
    if ("heading" in row) {
      el("text", { x: 0, y: y + 4, "font-weight": 600 }, row.heading);
      return;
    }
    const m = row;
    el("text", { x: 0, y: y + 4 }, m.label);
    const c = m.control;
    if (c !== undefined) {
      const a = Math.min(c.low ?? c.median, c.median);
      const b = Math.max(c.high ?? c.median, c.median);
      el("rect", {
        x: x(Math.max(a, 0.0625)),
        y: y - 9,
        width: Math.max(2, x(Math.min(b, 8)) - x(Math.max(a, 0.0625))),
        height: 18,
        rx: 4,
        fill: "var(--line)",
        opacity: 0.6,
      });
    }
    const p = m.peras;
    if (p.low !== undefined && p.high !== undefined) {
      el("line", {
        x1: x(Math.max(p.low, 0.0625)),
        x2: x(Math.min(p.high, 8)),
        y1: y,
        y2: y,
        stroke: "var(--peras)",
        "stroke-width": 2,
      });
    }
    if (m.before !== undefined) {
      el("circle", { cx: x(m.before.median), cy: y, r: 5, fill: "none", stroke: "var(--muted)" });
      el("line", {
        x1: x(m.before.median),
        x2: x(p.median),
        y1: y,
        y2: y,
        stroke: "var(--muted)",
        "stroke-dasharray": "2 3",
      });
    }
    el("circle", {
      cx: x(p.median),
      cy: y,
      r: 7,
      // Within 0.95 to 1.05, the noise an A/A control measures (M042): equal.
      fill: p.median < 0.95 ? "var(--good)" : p.median > 1.05 ? "var(--bad)" : "var(--muted)",
    });
    // The figure to the right of everything drawn on the row.
    const right = Math.max(p.median, p.high ?? 0, Math.min(m.before?.median ?? 0, 8));
    el("text", { x: Math.min(x(right) + 12, width - 40), y: y + 4 }, p.median.toFixed(2));
  });
  $("map-legend").innerHTML = "";
  for (const [color, label] of [
    ["var(--good)", T.faster],
    ["var(--bad)", T.slower],
    ["var(--muted)", T.equal],
    ["var(--muted)", T.before],
    ["var(--line)", T.band],
  ] as const) {
    const s = document.createElement("span");
    const d = document.createElement("span");
    d.className = "dot";
    d.style.background = color;
    s.append(d, label);
    $("map-legend").append(s);
  }
  text(
    "map-source",
    `${T.measuredOn} ${machine(frameResults)} — bench/frame/RESULTS.md, bench/next/RESULTS.md.`,
  );
}

function bugMatrix() {
  const list = bugs(bugResults);
  const grid = $("bugs");
  grid.replaceChildren();
  for (const h of ["", "TypeScript", "Peras"]) {
    const s = document.createElement("strong");
    s.textContent = h;
    grid.append(s);
  }
  const cells: HTMLElement[] = [];
  for (const b of list) {
    const name = document.createElement("span");
    name.textContent = `${b.name} (${b.kind})`;
    grid.append(name);
    for (const who of [b.typescript, b.peras]) {
      const c = document.createElement("span");
      c.className = `cell ${who}`;
      c.textContent = T.stops[who];
      grid.append(c);
      cells.push(c);
    }
  }
  // The cells come in one by one when the matrix is seen.
  const reveal = () => {
    cells.forEach((c, i) =>
      setTimeout(() => {
        c.classList.add("shown");
      }, i * 40),
    );
  };
  if ("IntersectionObserver" in window) {
    const seen = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        reveal();
        seen.disconnect();
      }
    });
    seen.observe(grid);
  } else reveal();
  text("bugs-tally", T.tally(tally(list, "typescript"), tally(list, "peras")));
}

mapChart();
bugMatrix();
show();
document.body.dataset.ready = "true";
requestAnimationFrame(tick);
