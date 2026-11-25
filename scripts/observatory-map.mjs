// The observatory's map, the one the READMEs show: built from the page
// itself (examples/observatory), which reads the repository's results files,
// so that the image says what the results say. Each colour the page takes
// from its theme is written into the SVG, on a light ground of its own, as
// GitHub shows an image on either theme.
//
// Usage: PERAS_CHROMIUM=… node scripts/observatory-map.mjs [--check]
//        --check: nothing written; fails if docs/assets/observatory-map.*.svg
//        no longer say what the results say.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { extname, join } from "node:path";
import { chromium } from "playwright";

const check = process.argv.includes("--check");
const root = join(import.meta.dirname, "..");
const out = join(root, "target", "observatory-map");

const built = spawnSync(
  "pnpm",
  ["--filter", "peras-example-observatory", "build", "--outDir", out, "--emptyOutDir"],
  { cwd: root, encoding: "utf8" },
);
if (built.status !== 0) throw new Error(built.stderr || built.stdout);

const types = /** @type {Record<string, string>} */ ({
  ".js": "text/javascript",
  ".wasm": "application/wasm",
  ".html": "text/html",
});
const server = createServer((request, response) => {
  const path = (request.url ?? "/").split("?")[0] ?? "/";
  const file = join(out, path === "/" ? "index.html" : path);
  if (!existsSync(file)) {
    response.writeHead(404);
    response.end();
    return;
  }
  response.writeHead(200, { "content-type": types[extname(file)] ?? "text/plain" });
  response.end(readFileSync(file));
});
await new Promise((resolve) => {
  server.listen(0, () => {
    resolve(undefined);
  });
});
const address = server.address();
const port = typeof address === "object" && address !== null ? address.port : 0;

const executablePath = process.env.PERAS_CHROMIUM;
const browser = await chromium.launch(executablePath === undefined ? {} : { executablePath });
const stale = [];
for (const lang of ["en", "fr"]) {
  const page = await browser.newPage({ colorScheme: "light" });
  await page.goto(`http://localhost:${String(port)}/?lang=${lang}`);
  await page.waitForSelector("body[data-ready=true]", { state: "attached" });
  // The page's SVG, each colour resolved, a ground and a font of its own.
  const svg = /** @type {string} */ (
    await page.evaluate(`(() => {
      const source = document.getElementById("map");
      const copy = source.cloneNode(true);
      const from = source.querySelectorAll("*");
      const to = copy.querySelectorAll("*");
      from.forEach((element, i) => {
        const style = getComputedStyle(element);
        const target = to[i];
        for (const p of ["fill", "stroke", "stroke-dasharray", "opacity", "font-weight"]) {
          const v = style.getPropertyValue(p);
          if (v !== "" && v !== "none" && v !== "normal") target.setAttribute(p, v);
        }
        target.removeAttribute("class");
      });
      const [, , w, h] = (copy.getAttribute("viewBox") || "").split(" ");
      copy.setAttribute("xmlns", "http://www.w3.org/2000/svg");
      copy.setAttribute("width", w);
      copy.setAttribute("height", h);
      copy.setAttribute("font-family", "system-ui, -apple-system, Segoe UI, Roboto, sans-serif");
      copy.setAttribute("font-size", "12");
      const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
      rect.setAttribute("width", "100%");
      rect.setAttribute("height", "100%");
      rect.setAttribute("fill", getComputedStyle(document.body).backgroundColor);
      copy.insertBefore(rect, copy.firstChild);
      return copy.outerHTML;
    })()`)
  );
  await page.close();
  const file = join(root, "docs", "assets", `observatory-map.${lang}.svg`);
  const text = `${svg}\n`;
  if (check) {
    if (!existsSync(file) || readFileSync(file, "utf8") !== text) stale.push(file);
  } else {
    writeFileSync(file, text);
    console.log(`wrote ${file}`);
  }
}
await browser.close();
server.close();
if (stale.length > 0) {
  console.error(
    `${stale.join(", ")}: no longer what the results say; run node scripts/observatory-map.mjs`,
  );
  process.exit(1);
}
if (check) console.log("the observatory's map in the READMEs is what the results say");
