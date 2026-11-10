// Renders docs/en and docs/fr into HTML pages for Vite to build.
//
// Every Markdown page under docs/<lang>/ becomes /<lang>/<path>.html, inside
// the site's layout: navigation, table of contents, language switch, search.
// A link to a page that does not exist fails the build, as do code blocks
// whose language is unknown.

import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import MarkdownIt from "markdown-it";
import type { LanguageRegistration } from "shiki";
import { createHighlighter } from "shiki";
import { type Lang, sections, words } from "./nav.ts";

const website = dirname(fileURLToPath(import.meta.url));
const root = resolve(website, "..");
const docs = join(root, "docs");
export const out = join(website, ".pages");
const repository = "https://github.com/artkabis/Peras";
const langs: Lang[] = ["en", "fr"];

/** A page: its source path under docs/<lang>/ without `.md`. */
interface Page {
  lang: Lang;
  path: string;
  file: string;
  title: string;
  /** The first paragraph, as plain text: the page's description. */
  description: string;
  html: string;
  toc: { level: number; id: string; text: string }[];
}

/** One searchable section of a page. */
interface SearchDoc {
  id: string;
  title: string;
  section: string;
  url: string;
  text: string;
}

const codeLanguages: Record<string, string> = {
  peras: "peras",
  apeiron: "peras",
  typescript: "typescript",
  ts: "typescript",
  javascript: "javascript",
  js: "javascript",
  sh: "shellscript",
  bash: "shellscript",
  shell: "shellscript",
  json: "json",
  toml: "toml",
  yaml: "yaml",
  rust: "rust",
  text: "text",
  mermaid: "mermaid",
  "": "text",
};

/** Every Markdown file under a directory, as paths relative to it. */
function markdownFiles(dir: string): string[] {
  const found: string[] = [];
  const walk = (current: string): void => {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith(".md")) found.push(relative(dir, full));
    }
  };
  walk(dir);
  return found.sort();
}

/**
 * An id for a heading, computed as GitHub does, so that a link works both on
 * GitHub and here: lower case, punctuation removed, spaces become hyphens,
 * accented letters kept. `Format d’un diagnostic` → `format-dun-diagnostic`.
 */
function slug(text: string, taken: Set<string>): string {
  const base = text
    .toLowerCase()
    .trim()
    .replace(/[^\p{L}\p{N}\s_-]/gu, "")
    .replace(/\s/g, "-");
  let id = base;
  for (let n = 1; taken.has(id); n++) id = `${base}-${String(n)}`;
  taken.add(id);
  return id;
}

/** Whether a block is under an error page's "Example" or "Fix" heading. */
function errorSection(
  tokens: { type: string; tag: string; content: string }[],
  at: number,
): string {
  for (let i = at - 1; i >= 1; i--) {
    const token = tokens[i];
    if (token?.type === "heading_open" && token.tag === "h2") {
      const heading = tokens[i + 1]?.content ?? "";
      if (heading === "Example" || heading === "Exemple") return "example";
      if (heading === "Fix" || heading === "Correction") return "fix";
      return "";
    }
  }
  return "";
}

function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

/** Renders every page; returns the HTML files Vite builds. */
export async function render(): Promise<string[]> {
  const grammar = JSON.parse(
    readFileSync(join(website, "src", "peras.tmLanguage.json"), "utf8"),
  ) as LanguageRegistration;
  const highlighter = await createHighlighter({
    themes: ["github-light", "github-dark"],
    langs: [grammar, "typescript", "javascript", "shellscript", "json", "toml", "yaml", "rust"],
  });
  const problems: string[] = [];
  const pages = new Map<string, Page>();
  // Every heading id of every page, and the anchors links point to.
  const ids = new Map<string, Set<string>>();
  const anchors: { from: string; to: string; anchor: string }[] = [];
  const sources = new Map<string, string>();
  for (const lang of langs) {
    for (const file of markdownFiles(join(docs, lang))) {
      const path = file.replace(/\.md$/, "");
      sources.set(`${lang}/${path}`, readFileSync(join(docs, lang, file), "utf8"));
    }
  }

  for (const [key, written] of sources) {
    const [lang, ...rest] = key.split("/") as [Lang, ...string[]];
    // The line that links a page to its translation is for readers on
    // GitHub; the site has its own language switch.
    const source = written.replace(
      /^_(French translation|Version anglaise, qui fait foi) ?: [^\n]*_\n+/m,
      "",
    );
    const path = rest.join("/");
    const md = new MarkdownIt({ html: true, linkify: true });
    const toc: Page["toc"] = [];
    const taken = new Set<string>();
    let title = path;

    // Code blocks: highlighted, with a label saying what the compiler does with them.
    md.renderer.rules.fence = (tokens, index) => {
      const token = tokens[index];
      if (token === undefined) return "";
      const [name = "", ...attrs] = token.info.trim().split(/\s+/);
      const language = codeLanguages[name];
      if (language === undefined) {
        problems.push(`${key}.md: unknown code block language "${name}"`);
        return "";
      }
      const code = token.content.replace(/\n$/, "");
      // A diagram: drawn in the browser by Mermaid, in the site's colours.
      if (language === "mermaid") {
        return `<figure class="diagram"><pre class="mermaid">${escapeHtml(code)}</pre></figure>\n`;
      }
      const highlighted =
        language === "text"
          ? `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`
          : highlighter.codeToHtml(code, {
              lang: language,
              themes: { light: "github-light", dark: "github-dark" },
              defaultColor: false,
            });
      let label = "";
      if (name === "peras" || name === "apeiron") {
        // What the compiler does with the block; the docs tests hold it to it.
        // On an error's page, the example is refused with that code and the
        // fix no longer is (crates/perasc-ir/tests/ui.rs checks both).
        const section = errorSection(tokens, index);
        const code = path.split("/").pop() ?? "";
        const kind =
          attrs[0] ??
          (path.startsWith("errors/E") && section === "example"
            ? "error"
            : path.startsWith("errors/E") && section === "fix"
              ? "fixed"
              : "check");
        const text: Record<string, string> = {
          build: words[lang].compiled,
          check: words[lang].checked,
          planned: words[lang].planned,
          error: `${words[lang].error} ${attrs[1] ?? code}`,
          fixed: words[lang].fixed,
        };
        // A managed module compiles to JavaScript, an owned one to WebAssembly.
        if (name === "apeiron") text.build = words[lang].compiledJs;
        const said = text[kind];
        if (said === undefined) problems.push(`${key}.md: unknown kind of ${name} block "${kind}"`);
        label = `<span class="code-label code-${escapeHtml(kind)}">${escapeHtml(said ?? kind)}</span>`;
      }
      return `<div class="code">${label}${highlighted}</div>\n`;
    };

    // Headings get ids; h2 and h3 go to the table of contents.
    md.core.ruler.push("heading_ids", (state) => {
      const tokens = state.tokens;
      for (let i = 0; i < tokens.length; i++) {
        const open = tokens[i];
        const inline = tokens[i + 1];
        if (open?.type !== "heading_open" || inline === undefined) continue;
        const text = inline.content.replaceAll("`", "");
        const level = Number(open.tag.slice(1));
        if (level === 1) {
          title = text;
          continue;
        }
        const id = slug(text, taken);
        open.attrSet("id", id);
        ids.set(key, taken);
        if (level <= 3) toc.push({ level, id, text });
      }
    });

    // Links: `.md` becomes `.html`; a missing page is an error; files outside
    // docs/ point to the repository.
    const defaultLink =
      md.renderer.rules.link_open ??
      ((tokens, i, options, _env, self) => self.renderToken(tokens, i, options));
    md.renderer.rules.link_open = (tokens, i, options, env, self) => {
      const token = tokens[i];
      const href = String(token?.attrGet("href") ?? "");
      if (token !== undefined && href.startsWith("#")) {
        anchors.push({ from: key, to: key, anchor: href.slice(1) });
      } else if (token !== undefined && !/^[a-z]+:/.test(href)) {
        const [target = "", anchor] = href.split("#");
        const absolute = resolve(dirname(join(docs, lang, path)), target);
        const inDocs = relative(docs, absolute);
        if (inDocs.startsWith("..")) {
          token.attrSet("href", `${repository}/blob/main/${relative(root, absolute)}`);
        } else if (target.endsWith(".md")) {
          const targetKey = inDocs.replace(/\.md$/, "");
          if (!sources.has(targetKey)) problems.push(`${key}.md: broken link to ${href}`);
          if (anchor !== undefined) anchors.push({ from: key, to: targetKey, anchor });
          const fromDir = dirname(`${lang}/${path}`);
          const relativeTarget = relative(fromDir, targetKey) || (targetKey.split("/").pop() ?? "");
          token.attrSet(
            "href",
            `${relativeTarget}.html${anchor === undefined ? "" : `#${anchor}`}`,
          );
        } else if (target !== "" && !existsSync(absolute)) {
          problems.push(`${key}.md: broken link to ${href}`);
        }
      }
      return defaultLink(tokens, i, options, env, self);
    };

    const html = md.render(source);
    const first = /<p>([\s\S]*?)<\/p>/.exec(html)?.[1] ?? "";
    const description = first
      .replace(/<[^>]+>/g, "")
      .replace(/&[a-z]+;/g, " ")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 300);
    pages.set(key, { lang, path, file: `${lang}/${path}.md`, title, description, html, toc });
  }

  for (const { from, to, anchor } of anchors) {
    // Links are percent-encoded (`bibliothèque` becomes `biblioth%C3%A8que`); ids are not.
    if (!(ids.get(to)?.has(decodeURIComponent(anchor)) ?? false)) {
      problems.push(`${from}.md: link to a section that does not exist: ${to}.md#${anchor}`);
    }
  }
  if (problems.length > 0) {
    throw new Error(
      `The documentation has ${String(problems.length)} problem(s):\n${problems.join("\n")}`,
    );
  }

  rmSync(out, { recursive: true, force: true });
  mkdirSync(join(out, "public"), { recursive: true });
  cpSync(join(website, "src"), join(out, "_assets"), { recursive: true });
  const inputs: string[] = [];
  const search: Record<Lang, SearchDoc[]> = { en: [], fr: [] };
  for (const page of pages.values()) {
    const target = join(out, page.lang, `${page.path}.html`);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, layout(page, pages));
    inputs.push(target);
    search[page.lang].push(...searchDocs(page));
  }
  for (const lang of langs) {
    writeFileSync(join(out, "public", `search-${lang}.json`), JSON.stringify(search[lang]));
  }
  // The root sends each reader to their language.
  const index = join(out, "index.html");
  writeFileSync(
    index,
    `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Peras</title>
<link rel="icon" href="data:," />
<script>location.replace(navigator.language.startsWith("fr") ? "fr/index.html" : "en/index.html");</script>
</head>
<body><p><a href="en/index.html">English</a> · <a href="fr/index.html">Français</a></p></body>
</html>
`,
  );
  inputs.push(index);
  return inputs;
}

/** A page's sections as search documents. */
function searchDocs(page: Page): SearchDoc[] {
  const url = `${page.lang}/${page.path}.html`;
  const parts = page.html.split(/<h2 id="([^"]+)">/);
  const text = (html: string): string =>
    html
      .replace(/<[^>]+>/g, " ")
      .replace(/&[a-z]+;/g, " ")
      .replace(/\s+/g, " ")
      .trim();
  const docs: SearchDoc[] = [
    { id: url, title: page.title, section: "", url, text: text(parts[0] ?? "") },
  ];
  for (let i = 1; i < parts.length; i += 2) {
    const id = parts[i] ?? "";
    const body = parts[i + 1] ?? "";
    const heading = page.toc.find((t) => t.id === id)?.text ?? "";
    docs.push({
      id: `${url}#${id}`,
      title: page.title,
      section: heading,
      url: `${url}#${id}`,
      text: text(body),
    });
  }
  return docs;
}

/** The page in its layout. Every link is relative, so the site works at any address. */
function layout(page: Page, pages: Map<string, Page>): string {
  const { lang } = page;
  // From this page back to the site's root: `../../` for en/guide/basics.html.
  const up = "../".repeat(page.path.split("/").length);
  const w = words[lang];
  const other: Lang = lang === "en" ? "fr" : "en";
  const otherHref = pages.has(`${other}/${page.path}`)
    ? `${up}${other}/${page.path}.html`
    : `${up}${other}/index.html`;
  const ordered = sections.flatMap((s) => s.pages);
  const at = ordered.indexOf(page.path);
  const link = (path: string | undefined, label: string, cls: string): string => {
    if (path === undefined) return "";
    const target = pages.get(`${lang}/${path}`);
    return target === undefined
      ? ""
      : `<a class="${cls}" href="${up}${lang}/${path}.html"><span>${label}</span>${escapeHtml(target.title)}</a>`;
  };
  const sidebar = sections
    .map((section) => {
      const items = section.pages
        .map((path) => {
          const target = pages.get(`${lang}/${path}`);
          if (target === undefined) return "";
          const current =
            path === page.path || (path === "errors/index" && page.path.startsWith("errors/"));
          return `<li><a href="${up}${lang}/${path}.html"${current ? ' aria-current="page"' : ""}>${escapeHtml(target.title)}</a></li>`;
        })
        .join("");
      return `<section><h2>${escapeHtml(section.title[lang])}</h2><ul>${items}</ul></section>`;
    })
    .join("");
  const toc = page.toc
    .map(
      (t) => `<li class="toc-${String(t.level)}"><a href="#${t.id}">${escapeHtml(t.text)}</a></li>`,
    )
    .join("");
  const top = (path: string, label: string): string =>
    `<a href="${up}${lang}/${path}.html"${page.path.split("/")[0] === path.split("/")[0] ? ' aria-current="true"' : ""}>${label}</a>`;
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>${escapeHtml(page.title)} · Peras</title>
<meta name="description" content="${escapeHtml(page.description)}" />
<meta property="og:title" content="${escapeHtml(page.title)} · Peras" />
<meta property="og:description" content="${escapeHtml(page.description)}" />
<meta name="theme-color" content="#a34b26" />
<link rel="icon" href="data:," />
<script>try{const t=localStorage.getItem("peras-theme");if(t)document.documentElement.dataset.theme=t}catch{}</script>
<link rel="stylesheet" href="${up}_assets/style.css" />
<script type="module" src="${up}_assets/main.ts"></script>
</head>
<body data-lang="${lang}" data-root="${up}" data-words='${escapeHtml(JSON.stringify(w))}'>
<header class="top">
  <button class="menu" type="button" aria-label="${w.menu}">☰</button>
  <a class="brand" href="${up}${lang}/index.html"><span class="mark">π</span> Peras</a>
  <nav class="primary">
    ${top("guide/introduction", sections[0]?.title[lang] ?? "")}
    ${top("reference/status", sections[1]?.title[lang] ?? "")}
    ${top("errors/index", sections[2]?.title[lang] ?? "")}
  </nav>
  <button class="search-open" type="button" aria-label="${w.search}"><svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"/><path d="m20 20-4-4" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg><span>${w.search}</span><kbd>/</kbd></button>
  <a class="lang" href="${otherHref}" hreflang="${other}">${w.other}</a>
  <button class="theme" type="button" aria-label="${w.theme}" title="${w.theme}"><svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true"><circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2"/><path d="M12 3a9 9 0 0 0 0 18z" fill="currentColor"/></svg></button>
  <a class="github" href="${repository}">GitHub</a>
</header>
<div class="layout">
  <aside class="sidebar"><nav>${sidebar}</nav></aside>
  <main>
    <article class="doc">${page.html}</article>
    <nav class="pager">${link(at > 0 ? ordered[at - 1] : undefined, w.previous, "prev")}${link(at >= 0 ? ordered[at + 1] : undefined, w.next, "next")}</nav>
    <footer><a href="${repository}/edit/main/docs/${page.file}">${w.edit}</a></footer>
  </main>
  <aside class="toc">${toc === "" ? "" : `<h2>${w.onThisPage}</h2><ul>${toc}</ul>`}</aside>
</div>
<dialog class="search"><input type="search" placeholder="${w.searchPlaceholder}" aria-label="${w.search}" /><ul class="results"></ul></dialog>
</body>
</html>
`;
}
