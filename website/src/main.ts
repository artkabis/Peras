// The documentation's few behaviours: theme, search, copy buttons, menu, and
// the table of contents following the reader.
import MiniSearch from "minisearch";

/** A search result, with the fields the index stores. */
interface SearchHit {
  url: string;
  title: string;
  section: string;
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
});

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
    storeFields: ["title", "section", "url"],
    searchOptions: { boost: { title: 3, section: 2 }, prefix: true, fuzzy: 0.2 },
  });
  index.addAll(docs);
  showResults();
}

function showResults(): void {
  if (!results || !input || !index) return;
  const query = input.value.trim();
  results.replaceChildren();
  if (query === "") return;
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
    li.append(a);
    results.append(li);
  }
}

input?.addEventListener("input", showResults);
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
