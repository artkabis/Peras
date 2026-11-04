import { fib } from "./fib.peras";

// English first, then French, as everywhere in Peras.
const texts = {
  en: {
    title: "fib in Peras",
    intro: "The same function, in Peras compiled to WebAssembly, and in JavaScript.",
    run: "Run",
    build: "Build",
    result: "fib(n)",
    time: "Time",
  },
  fr: {
    title: "fib en Peras",
    intro: "La même fonction, en Peras compilé en WebAssembly, et en JavaScript.",
    run: "Lancer",
    build: "Version",
    result: "fib(n)",
    time: "Durée",
  },
};
const lang = navigator.language.startsWith("fr") ? "fr" : "en";
const t = texts[lang];
document.documentElement.lang = lang;

function element(id: string): HTMLElement {
  const found = document.getElementById(id);
  if (found === null) throw new Error(`missing #${id}`);
  return found;
}

element("title").textContent = t.title;
element("intro").textContent = t.intro;
element("run").textContent = t.run;
element("h-build").textContent = t.build;
element("h-result").textContent = t.result;
element("h-time").textContent = t.time;

function fibJs(n: number): number {
  return n < 2 ? n : (fibJs(n - 1) + fibJs(n - 2)) | 0;
}

/** Runs `f(n)`, returning its result and the time it took. */
function timed(f: (n: number) => number, n: number): [number, string] {
  const start = performance.now();
  const value = f(n);
  return [value, `${(performance.now() - start).toFixed(1)} ms`];
}

function update(): void {
  const n = Math.min(40, Math.max(0, (element("n") as HTMLInputElement).valueAsNumber || 0));
  const [wasm, wasmTime] = timed(fib, n);
  const [js, jsTime] = timed(fibJs, n);
  element("wasm-result").textContent = String(wasm);
  element("wasm-time").textContent = wasmTime;
  element("js-result").textContent = String(js);
  element("js-time").textContent = jsTime;
}

element("form").addEventListener("submit", (event) => {
  event.preventDefault();
  update();
});
update();
document.body.dataset.ready = "true";
