// Fail closed if a built landing page would render without its layout CSS.
// node scripts/assert-built-styles.mjs dist
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";

const dist = process.argv[2];
if (!dist) {
  console.error("usage: assert-built-styles.mjs <dist-dir>");
  process.exit(2);
}

const pages = ["index.html", join("pt-br", "index.html")];
const required = [".desktop-nav", ".site-header", ".hero"];

const stylesheetHrefs = (html) => {
  const hrefs = [];
  for (const tag of html.match(/<link\b[^>]*>/gi) ?? []) {
    if (!/\brel\s*=\s*["']stylesheet["']/i.test(tag)) continue;
    const href = tag.match(/\bhref\s*=\s*["']([^"']+)["']/i)?.[1];
    if (href) hrefs.push(href);
  }
  return hrefs;
};

const resolveOnDisk = (page, href) => {
  if (/^(https?:)?\/\//i.test(href)) {
    throw new Error(`${page}: remote stylesheet ${href}`);
  }
  const relative = href.startsWith("/") ? href.slice(1) : join(dirname(page), href);
  return join(dist, relative);
};

const cssFor = (page, html) => {
  const chunks = [];
  for (const href of stylesheetHrefs(html)) {
    const file = resolveOnDisk(page, href);
    if (!existsSync(file)) {
      throw new Error(`${page}: stylesheet 404 on disk: ${href}`);
    }
    chunks.push(readFileSync(file, "utf8"));
  }
  for (const match of html.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/gi)) {
    chunks.push(match[1]);
  }
  return chunks.join("\n");
};

const hasFlexNav = (css) =>
  /\.desktop-nav[^{]*\{[^}]*display\s*:\s*flex/i.test(css.replace(/\s+/g, " "));

for (const page of pages) {
  const file = join(dist, page);
  if (!existsSync(file)) throw new Error(`missing ${page}`);
  const html = readFileSync(file, "utf8");
  const css = cssFor(page, html);
  if (!css.trim()) throw new Error(`${page}: no CSS inlined or linked`);
  for (const needle of required) {
    if (!css.includes(needle)) throw new Error(`${page}: CSS missing ${needle}`);
  }
  if (!hasFlexNav(css)) {
    throw new Error(`${page}: .desktop-nav is not display:flex`);
  }
}

console.log("ok");
