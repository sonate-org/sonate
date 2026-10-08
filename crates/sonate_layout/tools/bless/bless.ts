/// <reference lib="dom" />
/**
 * Records Chrome's layout of every fixture in `tests/fixtures` as `<fixture>.snap.json`.
 *
 *   deno task bless                  # all fixtures
 *   deno task bless nested/ align/   # fixtures whose name contains one of the arguments
 *
 * Downloads the Chrome for Testing build pinned below on first use; set
 * BLESS_CHROME=/path/to/chrome to use another build. See tests/fixtures/README.md.
 */
import { launch } from "@astral/astral";
import { dirname, fromFileUrl, join, relative } from "@std/path";
import { ZipReader } from "@zip-js/zip-js";

/** Chrome is the reference implementation, so all snapshots come from this exact build. */
const CHROME_VERSION = "153.0.8010.52";

const SCHEMA = 1;
const MAX_CASES = 64;
const TOOL_DIR = fromFileUrl(new URL(".", import.meta.url));
const FIXTURES_DIR = join(TOOL_DIR, "../../tests/fixtures");

/** Tags without user-agent styles that matter here; base.css makes them flex containers too. */
const ALLOWED_TAGS = ["div", "span", "section", "article", "header", "footer", "nav", "main", "aside", "label"];
const STATES = ["hover", "active", "focus", "none"];

interface Axis {
  selector: string;
  /** A CSS property, or `:state` for an `@state` axis. */
  property: string;
  values: string[];
}

interface Header {
  axes: Axis[];
  /** Computed values to record for every element (`@check`). */
  checks: string[];
  tolerance?: number;
}

type OverrideList = [string, string, string][];
type StateList = [string, string][];

interface Case {
  params: string;
  overrides: OverrideList;
  states: StateList;
}

interface MeasuredCase {
  nodes: Record<string, unknown>[];
  warnings: string[];
}

function parseHeader(html: string): Header {
  const header: Header = { axes: [], checks: [] };
  const values = (text: string) => text.split("|").map((value) => value.trim()).filter(Boolean);
  for (const comment of html.matchAll(/<!--([\s\S]*?)-->/g)) {
    for (const raw of comment[1].split("\n")) {
      const line = raw.trim();
      if (line.startsWith("@matrix")) {
        const match = line.match(/^@matrix\s+([#.][\w-]+)\s+([a-z-]+)\s*:\s*(.+)$/);
        if (!match) throw new Error(`invalid @matrix line: ${line}`);
        header.axes.push({ selector: match[1], property: match[2], values: values(match[3]) });
      } else if (line.startsWith("@state")) {
        const match = line.match(/^@state\s+([#.][\w-]+)\s+(.+)$/);
        if (!match) throw new Error(`invalid @state line: ${line}`);
        const states = values(match[2]);
        const invalid = states.find((state) => !STATES.includes(state));
        if (invalid) throw new Error(`unknown state ${invalid}; use ${STATES.join(" | ")}`);
        header.axes.push({ selector: match[1], property: ":state", values: states });
      } else if (line.startsWith("@check")) {
        header.checks.push(...line.slice("@check".length).trim().split(/\s+/).filter(Boolean));
      } else if (line.startsWith("@tolerance")) {
        header.tolerance = Number(line.split(/\s+/)[1]);
        if (!Number.isFinite(header.tolerance)) throw new Error(`invalid @tolerance: ${line}`);
      } else if (line.startsWith("@")) {
        throw new Error(`unknown directive: ${line}`);
      }
    }
  }
  return header;
}

/** Every combination of the axis values, in declaration order. */
function expandCases(axes: Axis[]): Case[] {
  let combinations: [Axis, string][][] = [[]];
  for (const axis of axes) {
    combinations = combinations.flatMap((chosen) =>
      axis.values.map((value) => [...chosen, [axis, value]] as [Axis, string][])
    );
  }
  if (combinations.length > MAX_CASES) {
    throw new Error(`@matrix/@state expand to ${combinations.length} cases, the limit is ${MAX_CASES}`);
  }
  return combinations.map((chosen) => ({
    params: chosen
      .map(([axis, value]) =>
        axis.property === ":state" ? `${axis.selector} state=${value}` : `${axis.selector} ${axis.property}=${value}`
      )
      .join(", "),
    overrides: chosen
      .filter(([axis]) => axis.property !== ":state")
      .map(([axis, value]) => [axis.selector, axis.property, value]),
    states: chosen
      .filter(([axis, value]) => axis.property === ":state" && value !== "none")
      .map(([axis, value]) => [axis.selector, value]),
  }));
}

/** FNV-1a (64 bit); `fixture_hash` in src/testing/snapshot.rs computes the same value. */
function fixtureHash(html: string, baseCss: string): string {
  const input = `schema=${SCHEMA}\n${baseCss.replaceAll("\r\n", "\n")}\n\0\n${
    html.replaceAll("\r\n", "\n")
  }`;
  let hash = 0xcbf29ce484222325n;
  for (const byte of new TextEncoder().encode(input)) {
    hash ^= BigInt(byte);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

/**
 * Runs inside the page and installs `window.__bless`, which applies a case's overrides, finds
 * elements for the mouse, and reads computed values and rects. Must be self-contained because
 * astral sends its source text to the page.
 */
async function installInPage(checks: string[], allowedTags: string[]): Promise<void> {
  const fail = (message: string): never => {
    throw new Error(message);
  };

  // Computed values recorded in the snapshot, with their initial values (omitted when equal).
  const INITIAL: Record<string, string | number> = {
    "display": "flex",
    "position": "relative",
    "box-sizing": "content-box",
    "overflow-x": "visible",
    "overflow-y": "visible",
    "width": "auto",
    "height": "auto",
    "min-width": "auto",
    "min-height": "auto",
    "max-width": "none",
    "max-height": "none",
    "margin-top": "0px",
    "margin-right": "0px",
    "margin-bottom": "0px",
    "margin-left": "0px",
    "padding-top": "0px",
    "padding-right": "0px",
    "padding-bottom": "0px",
    "padding-left": "0px",
    "border-top-width": "0px",
    "border-right-width": "0px",
    "border-bottom-width": "0px",
    "border-left-width": "0px",
    "top": "auto",
    "right": "auto",
    "bottom": "auto",
    "left": "auto",
    "flex-direction": "row",
    "flex-wrap": "nowrap",
    "justify-content": "normal",
    "align-items": "normal",
    "align-self": "auto",
    "align-content": "normal",
    "flex-grow": 0,
    "flex-shrink": 1,
    "flex-basis": "auto",
    "order": 0,
    "row-gap": "normal",
    "column-gap": "normal",
    "font-size": "10px",
  };
  const ALIGN = ["normal", "stretch", "flex-start", "flex-end", "start", "end", "center"];
  const DISTRIBUTE = ["space-between", "space-around", "space-evenly"];
  const OVERFLOW = ["visible", "clip", "hidden", "scroll", "auto"];
  // Allowed keywords; length properties also accept px and %, numeric ones numbers.
  const KEYWORDS: Record<string, string[]> = {
    "display": ["flex", "none"],
    "position": ["relative", "absolute"],
    "box-sizing": ["content-box", "border-box"],
    "overflow-x": OVERFLOW,
    "overflow-y": OVERFLOW,
    "flex-direction": ["row", "row-reverse", "column", "column-reverse"],
    "flex-wrap": ["nowrap", "wrap", "wrap-reverse"],
    "justify-content": [...ALIGN, ...DISTRIBUTE],
    "align-items": [...ALIGN, "baseline"],
    "align-self": ["auto", ...ALIGN, "baseline"],
    "align-content": [...ALIGN, ...DISTRIBUTE],
    "width": ["auto"],
    "height": ["auto"],
    "min-width": ["auto"],
    "min-height": ["auto"],
    "max-width": ["none"],
    "max-height": ["none"],
    "margin-top": ["auto"],
    "margin-right": ["auto"],
    "margin-bottom": ["auto"],
    "margin-left": ["auto"],
    "top": ["auto"],
    "right": ["auto"],
    "bottom": ["auto"],
    "left": ["auto"],
    "flex-basis": ["auto"],
    "row-gap": ["normal"],
    "column-gap": ["normal"],
  };
  const PX_ONLY = new Set([
    "border-top-width",
    "border-right-width",
    "border-bottom-width",
    "border-left-width",
    "font-size",
  ]);
  const NUMBERS = new Set(["flex-grow", "flex-shrink", "order"]);
  // Properties the engine does not implement; fixtures must leave them alone.
  const FIXED: Record<string, string> = {
    "writing-mode": "horizontal-tb",
    "direction": "ltr",
    "aspect-ratio": "auto",
    "transform": "none",
    "float": "none",
    "visibility": "visible",
    "text-indent": "0px",
    "font-variant-caps": "normal",
    "zoom": "1",
  };
  // Text properties recorded for text nodes (`ts`), with their initial values.
  const TEXT_INITIAL: Record<string, string> = {
    "white-space": "normal",
    "text-transform": "none",
    "overflow-wrap": "normal",
    "word-break": "normal",
    "text-align": "start",
    "letter-spacing": "normal",
    "word-spacing": "0px",
    "line-height": "normal",
  };

  const round = (value: number) => Math.round(value * 1e6) / 1e6;

  const format = (property: string, value: CSSStyleValue | undefined): string | number => {
    // calc(), min(), max() and clamp() with percentages stay expressions in computed values.
    if (value instanceof CSSMathValue && !PX_ONLY.has(property) && !NUMBERS.has(property)) {
      return String(value);
    }
    if (value instanceof CSSUnitValue) {
      if (NUMBERS.has(property)) {
        if (value.unit !== "number") fail(`${property}: unexpected unit ${value.unit}`);
        return value.value;
      }
      if (value.unit === "px") return `${round(value.value)}px`;
      if (value.unit === "percent" && !PX_ONLY.has(property)) return `${round(value.value)}%`;
      return fail(`${property}: unsupported value ${value}`);
    }
    const keyword = String(value);
    if (!(KEYWORDS[property] ?? []).includes(keyword)) {
      fail(`${property}: unsupported value \`${keyword}\``);
    }
    return keyword;
  };

  await document.fonts.load("10px Ahem");
  await document.fonts.ready;
  if (!document.fonts.check("10px Ahem")) fail("the Ahem font did not load");

  const roots = [...document.body.children].filter((element) =>
    !["STYLE", "SCRIPT"].includes(element.tagName)
  );
  if (roots.length !== 1) fail(`expected one root element, found ${roots.length}`);
  const root = roots[0] as HTMLElement;
  const elements = [root, ...root.querySelectorAll<HTMLElement>("*")];
  for (const element of elements) {
    if (!allowedTags.includes(element.tagName.toLowerCase())) {
      fail(`<${element.tagName.toLowerCase()}> is not supported; use ${allowedTags.join(", ")}`);
    }
  }
  const originalStyles = new Map(elements.map((element) => [element, element.getAttribute("style")]));

  const walk = (
    element: HTMLElement,
    depth: number,
    parentBox: DOMRect | null,
    nodes: Record<string, unknown>[],
    warnings: string[],
  ) => {
    const computed = element.computedStyleMap();
    const label = element.id
      ? `#${element.id}`
      : element.classList.length > 0
      ? `.${element.classList[0]}`
      : element.tagName.toLowerCase();

    const style: Record<string, string | number> = {};
    for (const [property, initial] of Object.entries(INITIAL)) {
      let value = format(property, computed.get(property) as CSSStyleValue);
      // Newer Chrome computes `border-width: medium` even without a border; the used width is 0.
      const side = property.match(/^border-(\w+)-width$/)?.[1];
      if (side && ["none", "hidden"].includes(String(computed.get(`border-${side}-style`)))) {
        value = "0px";
      }
      if (value !== initial) style[property] = value;
    }
    for (const [property, expected] of Object.entries(FIXED)) {
      let actual: string;
      try {
        actual = String(computed.get(property));
      } catch {
        continue; // Not supported by this Chrome version.
      }
      if (actual !== expected) fail(`${label}: ${property} must stay ${expected}, found ${actual}`);
    }

    const box = parentBox && element.getClientRects().length > 0
      ? element.getBoundingClientRect()
      : null;
    const rect = box
      ? [
        round(box.x - (depth === 0 ? 0 : parentBox!.x)),
        round(box.y - (depth === 0 ? 0 : parentBox!.y)),
        round(box.width),
        round(box.height),
      ]
      : [0, 0, 0, 0];

    const node: Record<string, unknown> = { d: depth, label, style };
    const texts = [...element.childNodes].filter((child) =>
      child.nodeType === Node.TEXT_NODE && (child.textContent ?? "").trim() !== ""
    );
    if (texts.length > 0) {
      if (element.children.length > 0) fail(`${label}: mixes text and elements`);
      // The raw text: white-space processing is part of what the engine implements.
      const text = [...element.childNodes].map((child) => child.textContent ?? "").join("");
      if (!/^[\x20-\x7e\n\t]*$/.test(text)) fail(`${label}: text may only contain printable ASCII`);
      node.text = text;
      node.font = parseFloat(String(computed.get("font-size")));

      const css = getComputedStyle(element);
      const textStyle: Record<string, string> = {};
      for (const [property, initial] of Object.entries(TEXT_INITIAL)) {
        const value = css.getPropertyValue(property);
        if (value !== initial) textStyle[property] = value;
      }
      if (Object.keys(textStyle).length > 0) node.ts = textStyle;

      if (box) {
        // Line boxes of the text, as browsers report them for a range over it.
        const range = document.createRange();
        range.selectNodeContents(element);
        node.lines = [...range.getClientRects()]
          // Forced line breaks show up as empty rects.
          .filter((line) => line.width > 0)
          .map((line) => [
            round(line.x - box.x),
            round(line.y - box.y),
            round(line.width),
            round(line.height),
          ]);
      }
    }
    node.rect = rect;
    if (checks.length > 0) {
      const css = getComputedStyle(element);
      node.checks = Object.fromEntries(checks.map((property) => [property, css.getPropertyValue(property)]));
    }
    nodes.push(node);

    for (const child of element.children) {
      walk(child as HTMLElement, depth + 1, box, nodes, warnings);
    }
  };

  const target = (selector: string) => {
    const element = root.parentElement!.querySelector<HTMLElement>(selector);
    if (!element) fail(`selector ${selector} matches nothing`);
    return element!;
  };

  // deno-lint-ignore no-explicit-any
  (window as any).__bless = {
    applyCase(overrides: OverrideList) {
      for (const [element, style] of originalStyles) {
        if (style === null) element.removeAttribute("style");
        else element.setAttribute("style", style);
      }
      for (const [selector, property, value] of overrides) {
        const targets = root.parentElement!.querySelectorAll<HTMLElement>(selector);
        if (targets.length === 0) fail(`@matrix selector ${selector} matches nothing`);
        for (const element of targets) {
          element.style.setProperty(property, value);
          if (element.style.getPropertyValue(property) === "") {
            fail(`@matrix: invalid value ${property}: ${value}`);
          }
        }
      }

      const rootStyle = root.computedStyleMap();
      for (const property of ["width", "height"]) {
        const value = rootStyle.get(property);
        if (!(value instanceof CSSUnitValue) || value.unit !== "px") {
          fail(`the root needs an explicit ${property} in px`);
        }
      }
      for (const side of ["top", "right", "bottom", "left"]) {
        if (String(rootStyle.get(`margin-${side}`)) !== "0px") fail("the root must not have margins");
      }
    },
    /** The center of the element's border box, where the mouse goes for hover/active. */
    center(selector: string): [number, number] {
      const box = target(selector).getBoundingClientRect();
      return [box.x + box.width / 2, box.y + box.height / 2];
    },
    focus(selector: string) {
      const element = target(selector);
      element.focus();
      if (document.activeElement !== element) fail(`${selector} can't be focused; give it a tabindex`);
    },
    blur() {
      (document.activeElement as HTMLElement | null)?.blur();
    },
    measure(): MeasuredCase {
      const nodes: Record<string, unknown>[] = [];
      const warnings: string[] = [];
      walk(root, 0, new DOMRect(0, 0, 0, 0), nodes, warnings);
      return { nodes, warnings };
    },
  };
}

function formatSnapshot(
  hash: string,
  chrome: string,
  header: Header,
  cases: (Case & { measured: MeasuredCase })[],
): string {
  const lines = ["{", `  "schema": ${SCHEMA},`, `  "hash": "${hash}",`, `  "chrome": ${JSON.stringify(chrome)},`];
  if (header.tolerance !== undefined) lines.push(`  "tolerance": ${header.tolerance},`);
  lines.push(`  "cases": [`);
  cases.forEach((testCase, caseIndex) => {
    lines.push("    {");
    lines.push(`      "params": ${JSON.stringify(testCase.params)},`);
    lines.push(`      "overrides": ${JSON.stringify(testCase.overrides)},`);
    if (testCase.states.length > 0) lines.push(`      "states": ${JSON.stringify(testCase.states)},`);
    lines.push(`      "warnings": ${JSON.stringify(testCase.measured.warnings)},`);
    lines.push(`      "nodes": [`);
    testCase.measured.nodes.forEach((node, nodeIndex) => {
      const comma = nodeIndex + 1 < testCase.measured.nodes.length ? "," : "";
      lines.push(`        ${JSON.stringify(node)}${comma}`);
    });
    lines.push("      ]");
    lines.push(`    }${caseIndex + 1 < cases.length ? "," : ""}`);
  });
  lines.push("  ]", "}", "");
  return lines.join("\n");
}

async function findFixtures(dir: string): Promise<string[]> {
  const found: string[] = [];
  for await (const entry of Deno.readDir(dir)) {
    const path = join(dir, entry.name);
    if (entry.isDirectory) found.push(...await findFixtures(path));
    else if (entry.name.endsWith(".html")) found.push(path);
  }
  return found.sort();
}

function fixtureName(path: string): string {
  return relative(FIXTURES_DIR, path).replaceAll("\\", "/").replace(/\.html$/, "");
}

/** The pinned Chrome for Testing build, downloaded into the user cache on first use. */
async function chromePath(): Promise<string> {
  const override = Deno.env.get("BLESS_CHROME");
  if (override) return override;

  const { os, arch } = Deno.build;
  const platform = os === "linux"
    ? "linux64"
    : os === "darwin"
    ? (arch === "aarch64" ? "mac-arm64" : "mac-x64")
    : os === "windows"
    ? "win64"
    : null;
  if (!platform) throw new Error(`no Chrome for Testing build for ${os}; set BLESS_CHROME`);

  const cacheRoot = os === "darwin"
    ? join(Deno.env.get("HOME") ?? ".", "Library/Caches")
    : os === "windows"
    ? Deno.env.get("LOCALAPPDATA") ?? "."
    : Deno.env.get("XDG_CACHE_HOME") ?? join(Deno.env.get("HOME") ?? ".", ".cache");
  const dir = join(cacheRoot, "sonate-bless", `chrome-${CHROME_VERSION}`);
  const app = "Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";
  const executable = join(
    dir,
    `chrome-${platform}`,
    platform.startsWith("mac") ? app : os === "windows" ? "chrome.exe" : "chrome",
  );
  try {
    await Deno.stat(executable);
    return executable;
  } catch {
    // Not downloaded yet.
  }

  const url =
    `https://storage.googleapis.com/chrome-for-testing-public/${CHROME_VERSION}/${platform}/chrome-${platform}.zip`;
  console.log(`downloading Chrome ${CHROME_VERSION} to ${dir}`);
  const response = await fetch(url);
  if (!response.ok || !response.body) throw new Error(`could not download ${url}: ${response.status}`);
  const zip = new ZipReader(response.body);
  for (const entry of await zip.getEntries()) {
    if (entry.directory || !entry.getData) continue;
    const path = join(dir, entry.filename);
    await Deno.mkdir(dirname(path), { recursive: true });
    const file = await Deno.open(path, { create: true, truncate: true, write: true, mode: 0o755 });
    await entry.getData(file.writable);
  }
  await zip.close();
  return executable;
}

async function main() {
  const filters = Deno.args;
  const baseCss = await Deno.readTextFile(join(TOOL_DIR, "base.css"));
  const all = await findFixtures(FIXTURES_DIR);
  const fixtures = all.filter((path) =>
    filters.length === 0 || filters.some((filter) => fixtureName(path).includes(filter))
  );
  if (fixtures.length === 0) {
    console.error(`no fixtures match ${filters.join(" ")}`);
    Deno.exit(1);
  }

  const server = Deno.serve({ hostname: "127.0.0.1", port: 0, onListen() {} }, async (request) => {
    const path = decodeURIComponent(new URL(request.url).pathname);
    if (path === "/base.css") {
      return new Response(baseCss, { headers: { "content-type": "text/css" } });
    }
    if (path === "/Ahem.ttf") {
      return new Response(await Deno.readFile(join(TOOL_DIR, "Ahem.ttf")), {
        headers: { "content-type": "font/ttf" },
      });
    }
    if (path.startsWith("/fixture/")) {
      const html = await Deno.readTextFile(join(FIXTURES_DIR, path.slice("/fixture/".length)));
      const page =
        `<!doctype html><html><head><meta charset="utf-8"><link rel="stylesheet" href="/base.css"></head><body>${html}</body></html>`;
      return new Response(page, { headers: { "content-type": "text/html; charset=utf-8" } });
    }
    return new Response("not found", { status: 404 });
  });

  const browser = await launch({
    path: await chromePath(),
    args: ["--force-device-scale-factor=1"],
  });
  const chrome = await browser.version();
  const page = await browser.newPage();
  await page.setViewportSize({ width: 1200, height: 900 });

  let failed = 0;
  let caseCount = 0;
  for (const path of fixtures) {
    const name = fixtureName(path);
    try {
      const html = await Deno.readTextFile(path);
      const header = parseHeader(html);
      const cases = expandCases(header.axes);
      await page.goto(
        `http://127.0.0.1:${server.addr.port}/fixture/${relative(FIXTURES_DIR, path).replaceAll("\\", "/")}`,
        { waitUntil: "load" },
      );
      await page.evaluate(installInPage, { args: [header.checks, ALLOWED_TAGS] });

      // deno-lint-ignore no-explicit-any
      type Bless = any;
      const measured: MeasuredCase[] = [];
      for (const testCase of cases) {
        // Start without hover, press or focus: the mouse rests outside the fixture.
        await page.mouse.move(1199, 899);
        await page.evaluate(() => (window as Bless).__bless.blur());
        await page.evaluate((overrides) => (window as Bless).__bless.applyCase(overrides), {
          args: [testCase.overrides],
        });

        let pressed = false;
        for (const [selector, state] of testCase.states) {
          if (state === "focus") {
            await page.evaluate((selector) => (window as Bless).__bless.focus(selector), { args: [selector] });
            continue;
          }
          const [x, y] = await page.evaluate(
            (selector) => (window as Bless).__bless.center(selector) as [number, number],
            { args: [selector] },
          );
          await page.mouse.move(x, y);
          if (state === "active") {
            await page.mouse.down();
            pressed = true;
          }
        }
        measured.push(await page.evaluate(() => (window as Bless).__bless.measure() as MeasuredCase));
        if (pressed) await page.mouse.up();
      }
      const snapshot = formatSnapshot(
        fixtureHash(html, baseCss),
        chrome,
        header,
        cases.map((testCase, index) => ({ ...testCase, measured: measured[index] })),
      );
      await Deno.writeTextFile(path.replace(/\.html$/, ".snap.json"), snapshot);
      caseCount += cases.length;
      const warnings = [...new Set(measured.flatMap((testCase) => testCase.warnings))];
      for (const warning of warnings) console.warn(`warning: ${name}: ${warning}`);
    } catch (error) {
      failed++;
      const message = error instanceof Error ? error.message : String(error);
      console.error(`error: ${name}: ${message.split("\n")[0]}`);
    }
  }

  // Snapshots without a fixture are leftovers from renamed or deleted fixtures.
  if (filters.length === 0) {
    const fixtureSet = new Set(all);
    for await (const snapshot of findSnapshots(FIXTURES_DIR)) {
      if (!fixtureSet.has(snapshot.replace(/\.snap\.json$/, ".html"))) {
        await Deno.remove(snapshot);
        console.log(`removed orphaned ${relative(FIXTURES_DIR, snapshot)}`);
      }
    }
  }

  await browser.close();
  await server.shutdown();
  console.log(
    `blessed ${fixtures.length - failed} fixtures (${caseCount} cases) with ${chrome}${
      failed ? `, ${failed} failed` : ""
    }`,
  );
  if (failed) Deno.exit(1);
}

async function* findSnapshots(dir: string): AsyncGenerator<string> {
  for await (const entry of Deno.readDir(dir)) {
    const path = join(dir, entry.name);
    if (entry.isDirectory) yield* findSnapshots(path);
    else if (entry.name.endsWith(".snap.json")) yield path;
  }
}

await main();
