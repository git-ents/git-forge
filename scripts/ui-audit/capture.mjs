#!/usr/bin/env node
// UI-audit capture harness (docs/ui-audit/phase-0.md).
//
// Boots git-forge-ui against the phase-0 fixture repo, walks every GET
// route from `crates/git-forge-ui/src/pages/*.rs`'s `#[page("...")]`
// attributes, and for each route x theme writes page.html / styles.json /
// page.png under target/ui-audit/<run>/<theme>/<route>/.
//
// Usage:
//   node scripts/ui-audit/capture.mjs [run-id]
//
// `playwright` is not a project dependency (see phase-0.md); this script
// resolves it from the npx cache. Run it via:
//   npx --yes -p playwright@1.62.1 node scripts/ui-audit/capture.mjs [run-id]
// or export NODE_PATH to a directory containing a `playwright` install.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { homedir, tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, "..", "..");

// `playwright` is not a project dependency (docs/ui-audit/phase-0.md); it
// lives in the npx cache. Static `import "playwright"` can't see it because
// Node's ESM resolver ignores NODE_PATH, so resolve it by hand: honor
// PLAYWRIGHT_MODULE_DIR if set, otherwise scan `~/.npm/_npx/*` (where `npx
// playwright ...` installs it) for the first `node_modules/playwright`.
function findPlaywrightDir() {
  if (process.env.PLAYWRIGHT_MODULE_DIR) return process.env.PLAYWRIGHT_MODULE_DIR;
  const npxCache = path.join(homedir(), ".npm", "_npx");
  if (!existsSync(npxCache)) return null;
  for (const entry of readdirSync(npxCache)) {
    const candidate = path.join(npxCache, entry, "node_modules", "playwright");
    if (existsSync(path.join(candidate, "package.json"))) return candidate;
  }
  return null;
}

const playwrightDir = findPlaywrightDir();
if (!playwrightDir) {
  console.error(
    "[capture] could not find a `playwright` install under ~/.npm/_npx.\n" +
      "Run `npx --yes playwright@1.62.1 --version` once to populate the npx cache,\n" +
      "or set PLAYWRIGHT_MODULE_DIR to a `playwright` package directory.",
  );
  process.exit(1);
}
const require = createRequire(import.meta.url);
const { chromium } = require(playwrightDir);

const STYLE_SELECTORS = [
  ".paragraph",
  ".paragraph > p",
  "p",
  "h1",
  "h2",
  "h3",
  "h4",
  "ul",
  "ol",
  "li",
  "pre",
  "code",
  "blockquote",
  "table",
  ".admonition",
  "button",
  ".badge",
];
const STYLE_PROPERTIES = [
  "marginTop",
  "marginBottom",
  "paddingTop",
  "paddingBottom",
  "lineHeight",
  "fontSize",
  "display",
];
const MAX_ELEMENTS_PER_SELECTOR = 60;
const MAX_SIBLING_RUN = 3;

function runId() {
  const arg = process.argv[2];
  if (arg) return arg;
  return new Date().toISOString().replace(/[:.]/g, "-");
}

// The 30 GET `#[page("...")]` routes in crates/git-forge-ui/src/pages/*.rs,
// with `{param}` placeholders filled from the fixture built by
// crates/git-forge-ui/tests/fixture.rs (build_fixture). Routes whose
// fixture-backed id doesn't exist (releases/annotations are kept at zero
// entities on purpose, per phase-0.md) use a placeholder id and are expected
// to return a documented non-2xx status.
function routes(ids) {
  return [
    // `/` 307-redirects to `/inbox` (crates/git-forge-ui/src/pages/root.rs);
    // Playwright follows the redirect and reports the final response, so
    // this captures the same content as /inbox under its own route dir.
    { path: "/" },
    { path: "/inbox" },
    { path: "/palette" },
    { path: "/tree" },
    { path: `/tree?ref=HEAD&path=${encodeURIComponent(ids.mdPath)}&view=preview`, prose: true },
    { path: `/tree?ref=HEAD&path=${encodeURIComponent(ids.adocPath)}&view=preview`, prose: true },
    { path: `/tree?ref=HEAD&path=${encodeURIComponent(ids.rustPath)}&view=source`, prose: true },
    { path: "/issues" },
    { path: "/issues/new" },
    { path: `/issues/${ids.issueId}`, prose: true },
    { path: `/issues/${ids.issueId}/edit` },
    { path: "/reviews" },
    { path: "/reviews/new" },
    { path: `/reviews/${ids.reviewId}`, prose: true },
    { path: `/reviews/${ids.reviewId}/decision` },
    { path: `/reviews/${ids.reviewId}/edit` },
    { path: "/annotations" },
    { path: `/annotations/${ids.annotationId}`, expectNon2xx: true },
    { path: "/gates" },
    { path: `/gates/${ids.gateId}` },
    { path: "/releases" },
    { path: `/releases/${ids.releaseTag}`, expectNon2xx: true },
    { path: "/releases/new" },
    { path: `/releases/${ids.releaseTag}/edit`, expectNon2xx: true },
    { path: "/members" },
    { path: `/members/${ids.memberId}` },
    { path: "/members/new" },
    { path: `/members/${ids.memberId}/edit` },
    { path: "/query" },
    { path: "/search" },
    { path: `/comments/${ids.commentId}` },
    { path: `/comments/edit/${ids.commentId}` },
    { path: "/comments/new" },
  ];
}

function routeDirName(routePath) {
  const [pathname, query] = routePath.split("?");
  let name = pathname === "/" ? "root" : pathname.replace(/^\//, "").replace(/\//g, "_");
  if (query) {
    name += "__" + query.replace(/[^a-zA-Z0-9=_-]/g, "_");
  }
  return name || "root";
}

async function buildFixture(fixtureDir) {
  console.log(`[fixture] building at ${fixtureDir}`);
  const result = spawnSync(
    "cargo",
    ["test", "-p", "git-forge-ui", "--test", "fixture", "--", "--ignored", "--exact", "build_fixture"],
    {
      cwd: REPO_ROOT,
      env: { ...process.env, UI_AUDIT_REPO: fixtureDir },
      stdio: "inherit",
    },
  );
  if (result.status !== 0) {
    throw new Error(`fixture build failed with status ${result.status}`);
  }
}

function spawnServer(fixtureDir) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "cargo",
      ["run", "-p", "git-forge-ui", "--", "--repo", fixtureDir, "--host", "127.0.0.1", "--port", "0"],
      { cwd: REPO_ROOT, stdio: ["ignore", "pipe", "pipe"] },
    );
    let settled = false;
    let stdoutBuf = "";
    const timeout = setTimeout(() => {
      if (!settled) {
        settled = true;
        child.kill();
        reject(new Error("timed out waiting for git-forge-ui to print its URL"));
      }
    }, 60_000);

    child.stdout.on("data", (chunk) => {
      stdoutBuf += chunk.toString();
      // The printed URL says `https://`, but crates/git-forge-ui/src/bin/
      // git-forge-ui.rs binds a plain tokio::net::TcpListener and hands it to
      // topcoat::serve with no TLS layer anywhere in that path (verified: a
      // TLS ClientHello against it gets a protocol-version alert; a raw HTTP/
      // 1.1 request over plain TCP, as tests/ui.rs's HttpClient sends, gets a
      // normal response). Contrary to phase-0.md, there's no cert to trust --
      // connect over http://, not https://.
      const match = stdoutBuf.match(/https:\/\/127\.0\.0\.1:(\d+)/);
      if (match && !settled) {
        settled = true;
        clearTimeout(timeout);
        resolve({ child, baseUrl: `http://127.0.0.1:${match[1]}` });
      }
    });
    child.stderr.on("data", () => {});
    child.on("exit", (code) => {
      if (!settled) {
        settled = true;
        clearTimeout(timeout);
        reject(new Error(`git-forge-ui exited before printing its URL (code ${code})`));
      }
    });
  });
}

// Runs in-page. Strips data: URIs, dedupes inline SVGs (keeping the first
// distinct copy of each), and collapses runs of >3 structurally-identical
// siblings to the first 3 plus a `<!-- N more -->` marker.
function stripDom() {
  const root = document.documentElement.cloneNode(true);

  for (const el of root.querySelectorAll("*")) {
    for (const attr of Array.from(el.attributes)) {
      if (attr.value.trim().startsWith("data:")) {
        el.setAttribute(attr.name, "data:stripped");
      }
    }
  }

  const seenSvg = new Set();
  for (const svg of Array.from(root.querySelectorAll("svg"))) {
    const key = svg.outerHTML;
    if (seenSvg.has(key)) {
      svg.remove();
    } else {
      seenSvg.add(key);
    }
  }

  const shapeKey = (el) => `${el.tagName}.${el.className || ""}`;
  for (const parent of Array.from(root.querySelectorAll("*"))) {
    const children = Array.from(parent.children);
    let i = 0;
    while (i < children.length) {
      let j = i + 1;
      while (j < children.length && shapeKey(children[j]) === shapeKey(children[i])) j++;
      const runLength = j - i;
      if (runLength > 3) {
        const extra = runLength - 3;
        for (let k = i + 3; k < j; k++) {
          children[k].remove();
        }
        const marker = document.createComment(` ${extra} more `);
        children[i + 2].after(marker);
      }
      i = j;
    }
  }

  return "<!DOCTYPE html>\n<html" + (root.getAttribute("data-theme") ? ` data-theme="${root.getAttribute("data-theme")}"` : "") + ">\n" + root.innerHTML;
}

function captureStyles({ selectors, properties, max: maxPerSelector }) {
  const results = {};
  for (const selector of selectors) {
    let elements;
    try {
      elements = Array.from(document.querySelectorAll(selector));
    } catch {
      results[selector] = { error: "invalid selector" };
      continue;
    }
    const total = elements.length;
    const capped = elements.slice(0, maxPerSelector);
    const rows = capped.map((el) => {
      const style = window.getComputedStyle(el);
      const styleValues = {};
      for (const prop of properties) styleValues[prop] = style[prop];
      const text = (el.textContent || "").trim().replace(/\s+/g, " ").slice(0, 80);
      return {
        tag: el.tagName.toLowerCase(),
        class: el.className && typeof el.className === "string" ? el.className : "",
        text,
        style: styleValues,
      };
    });
    results[selector] = {
      total,
      captured: rows.length,
      overflow: total > maxPerSelector ? total - maxPerSelector : 0,
      elements: rows,
    };
  }
  return results;
}

async function captureRoute(context, baseUrl, theme, route, outRoot, index) {
  const dirName = routeDirName(route.path);
  const outDir = path.join(outRoot, theme, dirName);
  await mkdir(outDir, { recursive: true });

  const page = await context.newPage();
  let status = null;
  let error = null;
  const artifacts = {};
  try {
    const response = await page.goto(baseUrl + route.path, { waitUntil: "load", timeout: 30_000 });
    status = response ? response.status() : null;

    if (status && status >= 200 && status < 400) {
      const html = await page.evaluate(stripDom);
      const htmlPath = path.join(outDir, "page.html");
      await writeFile(htmlPath, html, "utf8");
      artifacts.html = path.relative(outRoot, htmlPath);

      const styles = await page.evaluate(captureStyles, {
        selectors: STYLE_SELECTORS,
        properties: STYLE_PROPERTIES,
        max: MAX_ELEMENTS_PER_SELECTOR,
      });
      const stylesPath = path.join(outDir, "styles.json");
      await writeFile(stylesPath, JSON.stringify(styles, null, 2), "utf8");
      artifacts.styles = path.relative(outRoot, stylesPath);

      const pngPath = path.join(outDir, "page.png");
      await page.setViewportSize({ width: 1280, height: 900 });
      await page.screenshot({ path: pngPath, fullPage: false });
      artifacts.screenshot = path.relative(outRoot, pngPath);

      if (route.prose) {
        const fullPngPath = path.join(outDir, "page-full.png");
        await page.screenshot({ path: fullPngPath, fullPage: true });
        artifacts.screenshotFull = path.relative(outRoot, fullPngPath);
      }
    }
  } catch (err) {
    error = String(err && err.message ? err.message : err);
  } finally {
    await page.close();
  }

  index.push({
    path: route.path,
    theme,
    status,
    expectNon2xx: Boolean(route.expectNon2xx),
    error,
    artifacts,
  });
}

async function main() {
  const run = runId();
  const outRoot = path.join(REPO_ROOT, "target", "ui-audit", run);
  await mkdir(outRoot, { recursive: true });
  console.log(`[capture] run id: ${run}`);
  console.log(`[capture] output: ${outRoot}`);

  const fixtureDir = await mkdtemp(path.join(tmpdir(), "ui-audit-fixture-"));
  let serverHandle;
  let browser;
  try {
    await buildFixture(fixtureDir);

    console.log("[capture] booting git-forge-ui");
    serverHandle = await spawnServer(fixtureDir);
    console.log(`[capture] server up at ${serverHandle.baseUrl}`);

    browser = await chromium.launch();

    const ids = {
      mdPath: "README.md",
      adocPath: "guide.adoc",
      rustPath: "src/lib.rs",
      issueId: "audit-issue-1",
      reviewId: "audit-review-1",
      gateId: "merge-gate",
      releaseTag: "no-such-release",
      annotationId: "no-such-annotation",
      memberId: "alice",
      commentId: "audit-comment-issue-1",
    };
    const routeList = routes(ids);

    const index = [];
    for (const theme of ["light", "dark"]) {
      // ignoreHTTPSErrors is harmless but moot: the server is plain HTTP
      // (see spawnServer above) despite the URL it prints.
      const context = await browser.newContext({ ignoreHTTPSErrors: true });
      await context.addCookies([
        {
          name: "theme",
          value: theme,
          domain: "127.0.0.1",
          path: "/",
        },
      ]);
      for (const route of routeList) {
        console.log(`[capture] ${theme} ${route.path}`);
        await captureRoute(context, serverHandle.baseUrl, theme, route, outRoot, index);
      }
      await context.close();
    }

    const indexPath = path.join(outRoot, "index.json");
    await writeFile(indexPath, JSON.stringify({ run, routes: index }, null, 2), "utf8");
    console.log(`[capture] wrote ${indexPath}`);

    const undocumented = index.filter(
      (entry) => !(entry.status >= 200 && entry.status < 300) && !entry.expectNon2xx,
    );
    if (undocumented.length > 0) {
      console.error("[capture] undocumented non-2xx routes:");
      for (const entry of undocumented) {
        console.error(`  ${entry.theme} ${entry.path} -> ${entry.status} ${entry.error || ""}`);
      }
      process.exitCode = 1;
    }
  } finally {
    if (browser) await browser.close();
    if (serverHandle) {
      serverHandle.child.kill();
    }
    await rm(fixtureDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
