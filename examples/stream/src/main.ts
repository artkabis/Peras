// The page: it asks the worker for one batch after another, merges each
// digest into the totals, draws, and keeps every measure for export.
import { bins } from "./digest.ts";
import {
  emptyTotals,
  merge,
  summarize,
  toCsv,
  toJson,
  type Sample,
  type Totals,
} from "./metrics.ts";
import type { Reply, Request } from "./stream.worker.ts";

const texts = {
  en: {
    title: "A stream of sensors",
    intro:
      "480 sensors in twelve zones of a building send a reading every five seconds: time, sensor, zone, temperature, humidity and power, as CSV text, about one line in 400 broken. A Web Worker reads each batch with Peras, compiled to WebAssembly, and, if asked, with the same digest written in TypeScript, the two in turn. Every batch is timed and checked: both must give the same numbers. The TypeScript is written plainly, as it often is; optimized by hand, it would go faster.",
    start: "Start",
    pause: "Pause",
    rows: "Rows per batch",
    compare: "Compare with TypeScript",
    reset: "Reset",
    json: "Export JSON",
    csv: "Export CSV",
    processed: "Rows read",
    perasRate: "Peras",
    tsRate: "TypeScript",
    perSecond: "rows/s",
    speedup: "Peras is faster by",
    latency: "Peras per batch",
    agreement: "Same numbers",
    leaks: "Peras blocks left",
    broken: "Broken lines",
    throughput: "Text read by Peras",
    batches: "batches",
    colon: ":",
    notCompared: "not compared",
    timesTitle: "Time per batch, last 120 (ms)",
    histogramTitle: "Temperatures read (°C)",
    zonesTitle: "Zones",
    zone: "Zone",
    readings: "Readings",
    mean: "Mean °C",
    min: "Min °C",
    max: "Max °C",
    humidity: "Humidity %",
    energy: "Energy (Wh)",
    alerts: "Alerts",
    batch: "batch",
    readingsOf: "readings",
    dev: "Development build: the module is built for debug, every memory access and integer overflow checked. Build for production (pnpm build, then pnpm preview) to measure.",
    prod: "Production build: the module uses the TLSF allocator, integers wrap.",
    hottest: "Hottest reading",
    sensor: "sensor",
  },
  fr: {
    title: "Un flux de capteurs",
    intro:
      "480 capteurs dans douze zones d’un bâtiment envoient une mesure toutes les cinq secondes : heure, capteur, zone, température, humidité et puissance, en texte CSV, environ une ligne sur 400 cassée. Un Web Worker lit chaque lot avec Peras, compilé en WebAssembly, et, si on le demande, avec le même calcul écrit en TypeScript, l’un puis l’autre à tour de rôle. Chaque lot est chronométré et vérifié : les deux doivent donner les mêmes nombres. Le TypeScript est écrit simplement, comme souvent ; optimisé à la main, il irait plus vite.",
    start: "Démarrer",
    pause: "Pause",
    rows: "Lignes par lot",
    compare: "Comparer avec TypeScript",
    reset: "Recommencer",
    json: "Exporter en JSON",
    csv: "Exporter en CSV",
    processed: "Lignes lues",
    perasRate: "Peras",
    tsRate: "TypeScript",
    perSecond: "lignes/s",
    speedup: "Peras est plus rapide de",
    latency: "Peras par lot",
    agreement: "Mêmes nombres",
    leaks: "Blocs Peras restants",
    broken: "Lignes cassées",
    throughput: "Texte lu par Peras",
    batches: "lots",
    colon: " :",
    notCompared: "non comparé",
    timesTitle: "Temps par lot, 120 derniers (ms)",
    histogramTitle: "Températures lues (°C)",
    zonesTitle: "Zones",
    zone: "Zone",
    readings: "Mesures",
    mean: "Moy. °C",
    min: "Min °C",
    max: "Max °C",
    humidity: "Humidité %",
    energy: "Énergie (Wh)",
    alerts: "Alertes",
    batch: "lot",
    readingsOf: "mesures",
    dev: "Build de développement : le module est compilé en debug, chaque accès mémoire et chaque débordement d’entier vérifiés. Pour mesurer, construisez pour la production (pnpm build, puis pnpm preview).",
    prod: "Build de production : le module utilise l’allocateur TLSF, les entiers bouclent.",
    hottest: "Mesure la plus chaude",
    sensor: "capteur",
  },
} as const;

const asked = new URLSearchParams(location.search).get("lang");
const lang =
  asked === "fr" || asked === "en" ? asked : navigator.language.startsWith("fr") ? "fr" : "en";
const t = texts[lang];
const numbers = new Intl.NumberFormat(lang);
const fixed = (digits: number) =>
  new Intl.NumberFormat(lang, { minimumFractionDigits: digits, maximumFractionDigits: digits });
const one = fixed(1);
const two = fixed(2);
document.documentElement.lang = lang;

function $(id: string): HTMLElement {
  const e = document.getElementById(id);
  if (e === null) throw new Error(`#${id} is missing`);
  return e;
}

function svgOf(id: string): SVGSVGElement {
  const e = document.getElementById(id);
  if (!(e instanceof SVGSVGElement)) throw new Error(`#${id} is no <svg>`);
  return e;
}

const runButton = $("run") as HTMLButtonElement;
const rowsSelect = $("rows") as HTMLSelectElement;
const compareBox = $("compare") as HTMLInputElement;
const jsonButton = $("json") as HTMLButtonElement;
const csvButton = $("csv") as HTMLButtonElement;

document.title = t.title;
$("title").textContent = t.title;
$("intro").textContent = t.intro;
runButton.textContent = t.start;
$("rows-label").textContent = t.rows;
$("compare-label").textContent = t.compare;
$("reset").textContent = t.reset;
jsonButton.textContent = t.json;
csvButton.textContent = t.csv;
$("times-title").textContent = t.timesTitle;
$("histogram-title").textContent = t.histogramTitle;
$("zones-title").textContent = t.zonesTitle;
$("footer").textContent = import.meta.env.DEV ? t.dev : t.prod;
$("times-legend").innerHTML =
  `<span><span class="swatch" style="background:var(--peras)"></span>Peras</span>` +
  `<span><span class="swatch" style="background:var(--typescript)"></span>TypeScript</span>`;

// ---- State ----------------------------------------------------------------

const seed = 7;
let samples: Sample[] = [];
let totals: Totals = emptyTotals(bins);
let running = false;
let waiting = false;
let next = 0;
let started = performance.timeOrigin + performance.now();
let drawn = false;

const worker = new Worker(new URL("./stream.worker.ts", import.meta.url), { type: "module" });

function ask(): void {
  if (!running || waiting) return;
  waiting = true;
  const request: Request = {
    batch: next,
    rows: Number(rowsSelect.value),
    seed,
    compare: compareBox.checked,
    started,
  };
  next += 1;
  worker.postMessage(request);
}

worker.onmessage = (event: MessageEvent<Reply | "ready">) => {
  if (event.data === "ready") {
    runButton.disabled = false;
    document.body.dataset.ready = "true";
    return;
  }
  waiting = false;
  const { sample, digest } = event.data;
  samples.push(sample);
  merge(totals, digest);
  document.body.dataset.batches = String(samples.length);
  if (!drawn) {
    drawn = true;
    requestAnimationFrame(draw);
  }
  ask();
};

runButton.addEventListener("click", () => {
  running = !running;
  runButton.textContent = running ? t.pause : t.start;
  // A batch's size and the comparison are kept while it runs, so that every
  // sample of an export was taken under the settings it names.
  rowsSelect.disabled = running || samples.length > 0;
  compareBox.disabled = running || samples.length > 0;
  ask();
});

$("reset").addEventListener("click", () => {
  samples = [];
  totals = emptyTotals(bins);
  next = 0;
  started = performance.timeOrigin + performance.now();
  rowsSelect.disabled = running;
  compareBox.disabled = running;
  document.body.dataset.batches = "0";
  requestAnimationFrame(draw);
});

// ---- Export ---------------------------------------------------------------

function save(name: string, type: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 1000);
}

interface NavigatorMemory {
  deviceMemory?: number;
}

/** The report the JSON export holds; also read by the end-to-end test. */
function report(): string {
  return toJson({
    environment: {
      userAgent: navigator.userAgent,
      cores: navigator.hardwareConcurrency,
      deviceMemoryGiB: (navigator as Navigator & NavigatorMemory).deviceMemory ?? null,
      build: import.meta.env.DEV ? "development (debug allocator)" : "production (TLSF allocator)",
      crossOriginIsolated: self.crossOriginIsolated,
      date: new Date().toISOString(),
    },
    settings: { rowsPerBatch: Number(rowsSelect.value), compare: compareBox.checked, seed },
    summary: summarize(samples),
    samples,
  });
}
Object.assign(window, { __streamReport: report });

const stamp = () => new Date().toISOString().replaceAll(":", "-").slice(0, 19);
jsonButton.addEventListener("click", () => {
  save(`peras-stream-${stamp()}.json`, "application/json", report());
});
csvButton.addEventListener("click", () => {
  save(`peras-stream-${stamp()}.csv`, "text/csv", toCsv(samples));
});

// ---- Drawing --------------------------------------------------------------

function tile(label: string, value: string, note = "", tone = ""): string {
  return `<div class="tile"><div class="label">${label}</div><div class="value ${tone}">${value}</div><div class="note">${note}</div></div>`;
}

const rate = (r: number | null): string => (r === null ? "–" : numbers.format(Math.round(r)));

function drawTiles(): void {
  const s = summarize(samples);
  const last = samples.at(-1);
  const agreeTone = s.compared === 0 ? "" : s.disagreed === 0 ? "good" : "critical";
  const agree =
    s.compared === 0
      ? t.notCompared
      : `${numbers.format(s.compared - s.disagreed)} / ${numbers.format(s.compared)}`;
  $("tiles").innerHTML = [
    tile(
      t.processed,
      numbers.format(totals.lines),
      `${numbers.format(s.batches)} × ${numbers.format(Number(rowsSelect.value))}`,
    ),
    tile(t.perasRate, rate(samples.length > 0 ? s.perasRowsPerSecond : null), t.perSecond),
    tile(t.tsRate, rate(s.typescriptRowsPerSecond), s.compared > 0 ? t.perSecond : t.notCompared),
    tile(
      t.speedup,
      s.speedup === null ? "–" : `× ${two.format(s.speedup)}`,
      s.compared > 0 ? `${numbers.format(s.compared)} ${t.batches}` : "",
    ),
    tile(
      t.latency,
      samples.length > 0 ? `${one.format(s.perasP50)} ms` : "–",
      samples.length > 0 ? `p95 ${one.format(s.perasP95)} · p99 ${one.format(s.perasP99)}` : "",
    ),
    tile(t.agreement, agree, "", agreeTone),
    tile(
      t.leaks,
      numbers.format(last?.live ?? 0),
      `max ${numbers.format(s.maxLive)}`,
      s.maxLive === 0 ? "good" : "critical",
    ),
    tile(
      t.broken,
      numbers.format(totals.broken),
      totals.lines > 0 ? `${two.format((totals.broken / totals.lines) * 100)} %` : "",
    ),
    tile(
      t.throughput,
      s.perasMs > 0 ? `${one.format(s.bytes / 1048.576 / s.perasMs)} MiB/s` : "–",
      s.bytes > 0 ? `${one.format(s.bytes / 1048576)} MiB` : "",
    ),
  ].join("");
}

/** A coordinate as SVG text: a tenth of a unit is enough. */
const n = (v: number): string => String(Math.round(v * 10) / 10);
const W = 480;
const H = 220;
const pad = { left: 40, right: 64, top: 8, bottom: 22 };

/** A round, readable top for an axis that holds `max`. */
function niceTop(max: number): number {
  if (!(max > 0)) return 1;
  const step = 10 ** Math.floor(Math.log10(max));
  for (const m of [1, 2, 2.5, 5, 10]) if (m * step >= max) return m * step;
  return 10 * step;
}

function axis(svg: SVGSVGElement, top: number, format: (v: number) => string): string {
  let out = "";
  for (let i = 0; i <= 4; i++) {
    const v = (top * i) / 4;
    const y = pad.top + (H - pad.top - pad.bottom) * (1 - i / 4);
    out += `<line x1="${n(pad.left)}" x2="${n(W - pad.right)}" y1="${n(y)}" y2="${n(y)}" stroke="var(--grid)" stroke-width="1"/>`;
    out += `<text x="${n(pad.left - 6)}" y="${n(y + 4)}" text-anchor="end">${format(v)}</text>`;
  }
  svg.setAttribute("aria-label", svg.id);
  return out;
}

const shown = () => samples.slice(-120);

function drawTimes(): void {
  const svg = svgOf("times");
  const last = shown();
  const max = Math.max(1, ...last.map((s) => Math.max(s.perasMs, s.typescriptMs ?? 0)));
  const top = niceTop(max);
  const plotW = W - pad.left - pad.right;
  const plotH = H - pad.top - pad.bottom;
  const x = (i: number) => pad.left + (last.length <= 1 ? plotW : (plotW * i) / (last.length - 1));
  const y = (v: number) => pad.top + plotH * (1 - v / top);
  let out = axis(svg, top, (v) => numbers.format(v));
  const line = (pick: (s: Sample) => number | null, color: string, name: string) => {
    const points = last
      .map((s, i) => [i, pick(s)] as const)
      .filter((p): p is readonly [number, number] => p[1] !== null);
    if (points.length === 0) return;
    const d = points
      .map(([i, v], k) => `${k === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(v).toFixed(1)}`)
      .join("");
    out += `<path d="${d}" fill="none" stroke="${color}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>`;
    const [li, lv] = points[points.length - 1] ?? [0, 0];
    out += `<circle cx="${n(x(li))}" cy="${n(y(lv))}" r="4" fill="${color}" stroke="var(--surface)" stroke-width="2"/>`;
    out += `<text x="${n(x(li) + 8)}" y="${n(y(lv) + 4)}">${name} ${one.format(lv)}</text>`;
  };
  line((s) => s.typescriptMs, "var(--typescript)", "TS");
  line((s) => s.perasMs, "var(--peras)", "Peras");
  const firstBatch = last[0]?.batch ?? 0;
  out += `<text x="${n(pad.left)}" y="${n(H - 4)}">${t.batch} ${numbers.format(firstBatch)}</text>`;
  out += `<text x="${n(W - pad.right)}" y="${n(H - 4)}" text-anchor="end">${numbers.format(last.at(-1)?.batch ?? 0)}</text>`;
  out += `<line id="times-cross" x1="0" x2="0" y1="${n(pad.top)}" y2="${n(H - pad.bottom)}" stroke="var(--muted)" stroke-width="1" visibility="hidden"/>`;
  out += `<rect x="${n(pad.left)}" y="${n(pad.top)}" width="${n(plotW)}" height="${n(plotH)}" fill="transparent"/>`;
  svg.innerHTML = out;
}

function drawHistogram(): void {
  const svg = svgOf("histogram");
  const counts = totals.histogram;
  const top = niceTop(Math.max(1, ...counts));
  const plotW = W - pad.left - pad.right;
  const plotH = H - pad.top - pad.bottom;
  const slot = plotW / counts.length;
  let out = axis(svg, top, (v) =>
    v >= 1e6
      ? `${one.format(v / 1e6)} M`
      : v >= 1e3
        ? `${numbers.format(v / 1e3)} k`
        : numbers.format(v),
  );
  counts.forEach((count, i) => {
    const h = (plotH * count) / top;
    const bx = pad.left + i * slot + 1;
    const bw = Math.max(1, slot - 2);
    const by = pad.top + plotH - h;
    if (h > 0) {
      const r = Math.min(4, bw / 2, h);
      out += `<path d="M${n(bx)},${n(by + h)}V${n(by + r)}Q${n(bx)},${n(by)} ${n(bx + r)},${n(by)}H${n(bx + bw - r)}Q${n(bx + bw)},${n(by)} ${n(bx + bw)},${n(by + r)}V${n(by + h)}Z" fill="var(--bar)"/>`;
    }
  });
  for (const c of [-10, 0, 10, 20, 30, 40, 50]) {
    const cx = pad.left + ((c + 10) / 2) * slot;
    out += `<text x="${n(cx)}" y="${n(H - 4)}" text-anchor="middle">${String(c)}</text>`;
  }
  out += `<rect x="${n(pad.left)}" y="${n(pad.top)}" width="${n(plotW)}" height="${n(plotH)}" fill="transparent"/>`;
  svg.innerHTML = out;
}

function drawZones(): void {
  const head = [t.zone, t.readings, t.mean, t.min, t.max, t.humidity, t.energy, t.alerts];
  const rows = [...totals.zones.values()].sort((a, b) => (a.name < b.name ? -1 : 1));
  const body = rows
    .map((z) =>
      [
        z.name,
        numbers.format(z.readings),
        two.format(z.tempSum / z.readings),
        two.format(z.tempMin),
        two.format(z.tempMax),
        one.format(z.humiditySum / z.readings),
        // A reading's power held five seconds: watt-seconds over 3600 is Wh.
        numbers.format(Math.round((z.energy * 5) / 3600)),
        numbers.format(z.alerts),
      ]
        .map((c) => `<td>${c}</td>`)
        .join(""),
    )
    .map((r) => `<tr>${r}</tr>`)
    .join("");
  const hottest =
    totals.hottestSensor >= 0
      ? `<tr><td colspan="8">${t.hottest}${t.colon} ${two.format(totals.hottest)} °C, ${t.sensor} ${String(totals.hottestSensor)}</td></tr>`
      : "";
  $("zones").innerHTML =
    `<thead><tr>${head.map((h) => `<th>${h}</th>`).join("")}</tr></thead><tbody>${body}${hottest}</tbody>`;
}

function draw(): void {
  drawn = false;
  drawTiles();
  drawTimes();
  drawHistogram();
  drawZones();
  jsonButton.disabled = samples.length === 0;
  csvButton.disabled = samples.length === 0;
}

// ---- Hover ----------------------------------------------------------------

function hover(
  svgId: string,
  tipId: string,
  text: (fraction: number) => string | null,
  cross?: string,
): void {
  const svg = svgOf(svgId);
  const tip = $(tipId);
  svg.addEventListener("pointermove", (e) => {
    const box = svg.getBoundingClientRect();
    const sx = ((e.clientX - box.left) / box.width) * W;
    const f = (sx - pad.left) / (W - pad.left - pad.right);
    const label = f >= 0 && f <= 1 ? text(f) : null;
    const line = cross === undefined ? null : svg.querySelector<SVGLineElement>(`#${cross}`);
    if (label === null) {
      tip.style.display = "none";
      line?.setAttribute("visibility", "hidden");
      return;
    }
    tip.textContent = label;
    tip.style.display = "block";
    const card = tip.parentElement?.getBoundingClientRect() ?? box;
    tip.style.left = `${n(Math.min(e.clientX - card.left + 12, card.width - tip.offsetWidth - 4))}px`;
    tip.style.top = `${n(e.clientY - card.top - 32)}px`;
    if (line !== null) {
      const n = shown().length;
      const i = Math.round(f * (n - 1));
      const lx = pad.left + (n <= 1 ? 1 : i / (n - 1)) * (W - pad.left - pad.right);
      line.setAttribute("x1", String(lx));
      line.setAttribute("x2", String(lx));
      line.setAttribute("visibility", "visible");
    }
  });
  svg.addEventListener("pointerleave", () => {
    tip.style.display = "none";
  });
}

hover(
  "times",
  "times-tip",
  (f) => {
    const last = shown();
    const s = last[Math.round(f * (last.length - 1))];
    if (s === undefined) return null;
    const ts = s.typescriptMs === null ? "" : ` · TypeScript ${one.format(s.typescriptMs)} ms`;
    return `${t.batch} ${numbers.format(s.batch)}${t.colon} Peras ${one.format(s.perasMs)} ms${ts}`;
  },
  "times-cross",
);
hover("histogram", "histogram-tip", (f) => {
  const i = Math.min(bins - 1, Math.floor(f * bins));
  const from = -10 + 2 * i;
  return `${String(from)} – ${String(from + 2)} °C${t.colon} ${numbers.format(totals.histogram[i] ?? 0)} ${t.readingsOf}`;
});

draw();
