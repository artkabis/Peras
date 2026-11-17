// Fails when a Markdown page links to a file that does not exist, or to a
// heading the page it names does not have. External links are not followed.
// No dependencies: runs on a bare Node install.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, normalize, relative } from "node:path";

const root = join(import.meta.dirname, "..");
const skipped = new Set(["node_modules", "target", ".git", "dist", "cache"]);

/**
 * Every Markdown file under a directory.
 * @param {string} dir
 * @returns {string[]}
 */
function pages(dir) {
  /** @type {string[]} */
  const out = [];
  for (const name of readdirSync(dir)) {
    if (skipped.has(name)) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) out.push(...pages(path));
    else if (name.endsWith(".md")) out.push(path);
  }
  return out;
}

/**
 * A page's text without its code blocks, where `](` is code, not a link.
 * @param {string} file
 */
const prose = (file) => readFileSync(file, "utf8").replace(/```[\s\S]*?```/g, "");

/**
 * The anchor of a heading, as the site and GitHub make it.
 * @param {string} heading
 */
function slug(heading) {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[`*_~[\]()]/g, "")
    .replace(/[^\p{L}\p{N}_\- ]/gu, "")
    .replaceAll(" ", "-");
}

/** @type {Map<string, Set<string>>} */
const anchors = new Map();
/** @param {string} file */
function headings(file) {
  let found = anchors.get(file);
  if (!found) {
    found = new Set([...prose(file).matchAll(/^#+\s+(.*)$/gm)].map((m) => slug(m[1] ?? "")));
    anchors.set(file, found);
  }
  return found;
}

const files = pages(root);
/** @type {string[]} */
const broken = [];
for (const file of files) {
  for (const match of prose(file).matchAll(/\]\(([^)\s]+)\)/g)) {
    const link = match[1] ?? "";
    if (/^(https?|mailto):/.test(link)) continue;
    const [path = "", fragment = ""] = link.split("#");
    const target = path ? normalize(join(dirname(file), decodeURI(path))) : file;
    const where = relative(root, file);
    if (!existsSync(target)) {
      broken.push(`${where}: no file for ${link}`);
    } else if (fragment && target.endsWith(".md") && !headings(target).has(fragment)) {
      broken.push(`${where}: no heading for ${link}`);
    }
  }
}

if (broken.length > 0) {
  console.error(
    `${String(broken.length)} broken links:\n${broken.map((b) => `  ${b}`).join("\n")}`,
  );
  process.exit(1);
}
console.log(
  `links: ${String(files.length)} Markdown pages, every link reaches its file and heading`,
);
