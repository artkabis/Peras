// The documentation's few behaviours: theme, search, copy buttons, menu,
// diagrams, and the table of contents following the reader.
import MiniSearch from "minisearch";

/** A search result, with the fields the index stores. */
interface SearchHit {
  url: string;
  title: string;
  section: string;
  text: string;
  terms: string[];
}

interface SearchDoc {
  id: string;
  title: string;
  section: string;
  url: string;
  text: string;
}

const words = JSON.parse(document.body.dataset.words ?? "{}") as Record<string, string>;
const lang = document.body.dataset.lang ?? "en";
// From this page to the site's root, so the site works at any address.
const root = document.body.dataset.root ?? "";
const word = (key: string): string => words[key] ?? key;

// Theme: the system's, until the reader chooses; the choice is remembered.
document.querySelector(".theme")?.addEventListener("click", () => {
  const root = document.documentElement;
  const dark = root.dataset.theme
    ? root.dataset.theme === "dark"
    : matchMedia("(prefers-color-scheme: dark)").matches;
  root.dataset.theme = dark ? "light" : "dark";
  try {
    localStorage.setItem("peras-theme", root.dataset.theme);
  } catch {
    // Storage may be unavailable: the choice then lasts for the page only.
  }
  void drawDiagrams();
});

/** Whether the page is shown dark: the reader's choice, else the system's. */
function isDark(): boolean {
  const chosen = document.documentElement.dataset.theme;
  return chosen ? chosen === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
}

// Diagrams: Mermaid is loaded only by a page that has one, and draws them in
// the site's colours, again when the theme changes.
const diagrams = [...document.querySelectorAll<HTMLElement>("pre.mermaid")].map((pre) => ({
  pre,
  source: pre.textContent,
}));
async function drawDiagrams(): Promise<void> {
  if (diagrams.length === 0) return;
  const { default: mermaid } = await import("mermaid");
  const style = getComputedStyle(document.documentElement);
  const token = (name: string): string => style.getPropertyValue(name).trim();
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    fontFamily: token("--sans"),
    themeVariables: {
      darkMode: isDark(),
      background: token("--bg"),
      primaryColor: token("--bg-soft"),
      primaryTextColor: token("--fg"),
      primaryBorderColor: token("--accent"),
      secondaryColor: token("--accent-soft"),
      tertiaryColor: token("--code-bg"),
      lineColor: token("--muted"),
      textColor: token("--fg"),
      clusterBkg: token("--code-bg"),
      clusterBorder: token("--line"),
      edgeLabelBackground: token("--bg"),
    },
  });
  for (const { pre, source } of diagrams) {
    pre.removeAttribute("data-processed");
    pre.textContent = source;
  }
  await mermaid.run({ nodes: diagrams.map((d) => d.pre) });
}
void drawDiagrams();
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => void drawDiagrams());

// Menu, on small screens.
document.querySelector(".menu")?.addEventListener("click", () => {
  document.body.classList.toggle("menu-open");
});

// Copy buttons on code blocks.
for (const block of document.querySelectorAll<HTMLElement>(".code")) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "copy";
  button.textContent = word("copy");
  button.addEventListener("click", () => {
    const code = block.querySelector("pre")?.textContent ?? "";
    void navigator.clipboard.writeText(code).then(() => {
      button.textContent = word("copied");
      setTimeout(() => (button.textContent = word("copy")), 1500);
    });
  });
  block.append(button);
}

// Search: the index is loaded the first time it is opened.
const dialog = document.querySelector<HTMLDialogElement>("dialog.search");
const input = dialog?.querySelector("input");
const results = dialog?.querySelector(".results");
let index: MiniSearch<SearchDoc> | undefined;

async function openSearch(): Promise<void> {
  if (!dialog || !input) return;
  dialog.showModal();
  input.focus();
  if (index) return;
  const response = await fetch(`${root}search-${lang}.json`);
  const docs = (await response.json()) as SearchDoc[];
  index = new MiniSearch<SearchDoc>({
    fields: ["title", "section", "text"],
    storeFields: ["title", "section", "url", "text"],
    searchOptions: { boost: { title: 3, section: 2 }, prefix: true, fuzzy: 0.2 },
  });
  index.addAll(docs);
  showResults();
}

/** The index of the result chosen with the arrow keys. */
let chosen = 0;

/** `text` with each of `terms` marked, as DOM nodes. */
function marked(text: string, terms: string[]): Node[] {
  const pattern = terms
    .filter((t) => t.length > 1)
    .map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
    .join("|");
  if (pattern === "") return [document.createTextNode(text)];
  const nodes: Node[] = [];
  let last = 0;
  for (const match of text.matchAll(new RegExp(pattern, "gi"))) {
    const at = match.index;
    nodes.push(document.createTextNode(text.slice(last, at)));
    const mark = document.createElement("mark");
    mark.textContent = match[0];
    nodes.push(mark);
    last = at + match[0].length;
  }
  nodes.push(document.createTextNode(text.slice(last)));
  return nodes;
}

/** A few words of `text` around the first of `terms`. */
function excerpt(text: string, terms: string[]): string {
  const lower = text.toLowerCase();
  const at = Math.max(
    0,
    Math.min(
      ...terms.map((t) => lower.indexOf(t.toLowerCase())).filter((i) => i >= 0),
      text.length,
    ),
  );
  const start = Math.max(0, at - 60);
  const piece = text.slice(start, start + 180);
  return `${start > 0 ? "…" : ""}${piece}${start + 180 < text.length ? "…" : ""}`;
}

function choose(next: number): void {
  const items = results?.querySelectorAll("a") ?? [];
  if (items.length === 0) return;
  chosen = (next + items.length) % items.length;
  items.forEach((a, i) => a.classList.toggle("chosen", i === chosen));
  items[chosen]?.scrollIntoView({ block: "nearest" });
}

function showResults(): void {
  if (!results || !input || !index) return;
  const query = input.value.trim();
  results.replaceChildren();
  chosen = 0;
  if (query === "") {
    const li = document.createElement("li");
    li.className = "empty";
    li.textContent = word("searchHelp");
    results.append(li);
    return;
  }
  const found = index.search(query).slice(0, 12) as unknown as SearchHit[];
  if (found.length === 0) {
    const li = document.createElement("li");
    li.className = "empty";
    li.textContent = word("noResults");
    results.append(li);
    return;
  }
  for (const hit of found) {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.href = `${root}${hit.url}`;
    const title = document.createElement("strong");
    title.textContent = hit.title;
    a.append(title);
    if (hit.section) {
      const section = document.createElement("span");
      section.textContent = hit.section;
      a.append(section);
    }
    const text = document.createElement("small");
    text.append(...marked(excerpt(hit.text, hit.terms), hit.terms));
    a.append(text);
    li.append(a);
    results.append(li);
  }
  choose(0);
}

input?.addEventListener("input", showResults);
input?.addEventListener("keydown", (event) => {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    choose(chosen + (event.key === "ArrowDown" ? 1 : -1));
  } else if (event.key === "Enter") {
    const target = results?.querySelectorAll("a")[chosen];
    if (target) location.href = target.href;
  }
});
document.querySelector(".search-open")?.addEventListener("click", () => void openSearch());
dialog?.addEventListener("click", (event) => {
  if (event.target === dialog) dialog.close();
});
document.addEventListener("keydown", (event) => {
  const typing =
    event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement;
  if ((event.key === "/" && !typing) || (event.key === "k" && (event.metaKey || event.ctrlKey))) {
    event.preventDefault();
    void openSearch();
  }
});

// The table of contents marks the section being read.
const links = new Map<string, HTMLAnchorElement>();
for (const a of document.querySelectorAll<HTMLAnchorElement>(".toc a")) {
  links.set(a.hash.slice(1), a);
}
const observer = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      for (const a of links.values()) a.classList.remove("active");
      links.get(entry.target.id)?.classList.add("active");
    }
  },
  { rootMargin: "0px 0px -70% 0px" },
);
for (const id of links.keys()) {
  const heading = document.getElementById(id);
  if (heading) observer.observe(heading);
}
