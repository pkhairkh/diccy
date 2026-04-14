#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";

function parseArgs(argv) {
  const out = {
    url: process.env.RDVF_WASM_SMOKE_URL ?? "http://127.0.0.1:4173",
    output: "reports/performance/web-renderer-harness.json",
    interactions: 50,
    width: 512,
    height: 512,
  };
  for (let i = 2; i < argv.length; i += 1) {
    const arg = argv[i];
    const next = argv[i + 1];
    if (arg === "--url" && next) {
      out.url = next;
      i += 1;
    } else if (arg === "--output" && next) {
      out.output = next;
      i += 1;
    } else if (arg === "--interactions" && next) {
      out.interactions = Number.parseInt(next, 10) || out.interactions;
      i += 1;
    } else if (arg === "--width" && next) {
      out.width = Number.parseInt(next, 10) || out.width;
      i += 1;
    } else if (arg === "--height" && next) {
      out.height = Number.parseInt(next, 10) || out.height;
      i += 1;
    }
  }
  return out;
}

function u16le(bytes, value) {
  bytes.push(value & 0xff, (value >> 8) & 0xff);
}

function u32le(bytes, value) {
  bytes.push(value & 0xff, (value >> 8) & 0xff, (value >> 16) & 0xff, (value >> 24) & 0xff);
}

function asciiBytes(value) {
  return [...Buffer.from(value, "ascii")];
}

function appendElement(bytes, group, element, vr, payload) {
  u16le(bytes, group);
  u16le(bytes, element);
  bytes.push(vr.charCodeAt(0), vr.charCodeAt(1));
  const value = payload.length % 2 === 1 ? payload.concat([0]) : payload;
  if (["OB", "OW", "SQ", "UN", "UT"].includes(vr)) {
    u16le(bytes, 0);
    u32le(bytes, value.length);
  } else {
    u16le(bytes, value.length);
  }
  bytes.push(...value);
}

function syntheticPixels(width, height) {
  const pixels = new Array(width * height);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const idx = y * width + x;
      pixels[idx] = (x * 7 + y * 13) % 256;
    }
  }
  return pixels;
}

function buildLargeSampleP10(width, height) {
  const bytes = [];
  for (let i = 0; i < 128; i += 1) {
    bytes.push(0);
  }
  bytes.push(...asciiBytes("DICM"));

  appendElement(bytes, 0x0002, 0x0010, "UI", asciiBytes("1.2.840.10008.1.2.1"));
  appendElement(bytes, 0x0008, 0x0016, "UI", asciiBytes("1.2.840.10008.5.1.4.1.1.7"));
  appendElement(bytes, 0x0008, 0x0018, "UI", asciiBytes(`1.2.3.4.5.${width}.${height}`));
  appendElement(bytes, 0x0020, 0x000d, "UI", asciiBytes("1.2.3"));
  appendElement(bytes, 0x0020, 0x000e, "UI", asciiBytes("1.2.3.4"));
  appendElement(bytes, 0x0028, 0x0002, "US", [1, 0]);
  appendElement(bytes, 0x0028, 0x0004, "CS", asciiBytes("MONOCHROME2"));
  appendElement(bytes, 0x0028, 0x0010, "US", [height & 0xff, (height >> 8) & 0xff]);
  appendElement(bytes, 0x0028, 0x0011, "US", [width & 0xff, (width >> 8) & 0xff]);
  appendElement(bytes, 0x0028, 0x0100, "US", [8, 0]);
  appendElement(bytes, 0x0028, 0x0101, "US", [8, 0]);
  appendElement(bytes, 0x0028, 0x0102, "US", [7, 0]);
  appendElement(bytes, 0x0028, 0x0103, "US", [0, 0]);
  appendElement(bytes, 0x7fe0, 0x0010, "OB", syntheticPixels(width, height));

  return Buffer.from(bytes);
}

function ratio(numerator, denominator) {
  if (!denominator) {
    return 0;
  }
  return numerator / denominator;
}

const args = parseArgs(process.argv);

let playwright;
try {
  playwright = await import("playwright");
} catch (_error) {
  console.log("playwright dependency not found; skipping browser performance harness.");
  process.exit(0);
}

const { chromium } = playwright;
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext();
const page = await context.newPage();

const startedAt = new Date().toISOString();
await page.goto(args.url, { waitUntil: "domcontentloaded" });
await page.locator("#production_webgpu_toggle").check();
await page.locator("#dicom_file").setInputFiles({
  name: `perf-${args.width}x${args.height}.dcm`,
  mimeType: "application/dicom",
  buffer: buildLargeSampleP10(args.width, args.height),
});
await page.waitForTimeout(300);

const interactionStart = Date.now();
for (let i = 0; i < args.interactions; i += 1) {
  await page.locator("#zoom_in").click();
  await page.locator("#zoom_out").click();
  const box = await page.locator("#preview_canvas").boundingBox();
  if (box) {
    const x = box.x + ((i % 9) + 1) * (box.width / 10);
    const y = box.y + (((i * 3) % 9) + 1) * (box.height / 10);
    await page.mouse.click(x, y);
  }
}
const interactionDurationMs = Date.now() - interactionStart;
await page.waitForTimeout(200);

const snapshot = await page.evaluate(() => {
  if (typeof window.__RDVF_GET_SNAPSHOT === "function") {
    return window.__RDVF_GET_SNAPSHOT();
  }
  return null;
});
await browser.close();

const metrics = snapshot?.backendMetrics ?? {};
const webgpuPresent = Number(metrics.webgpu_present_count ?? 0);
const cpuPresent = Number(metrics.cpu_present_count ?? 0);
const fallbackToCpu = Number(metrics.fallback_to_cpu_count ?? 0);
const transitions = Number(metrics.backend_transition_count ?? 0);

const result = {
  started_at: startedAt,
  url: args.url,
  dataset: {
    width: args.width,
    height: args.height,
  },
  interactions: args.interactions,
  interaction_duration_ms: interactionDurationMs,
  adoption: {
    webgpu_present_count: webgpuPresent,
    cpu_present_count: cpuPresent,
    webgpu_adoption_rate: ratio(webgpuPresent, webgpuPresent + cpuPresent),
  },
  fallback: {
    fallback_to_cpu_count: fallbackToCpu,
    backend_transition_count: transitions,
    fallback_frequency: ratio(fallbackToCpu, Math.max(transitions, 1)),
    last_fallback_reason: metrics.last_fallback_reason ?? null,
    last_fallback_latency_ms: metrics.last_fallback_latency_ms ?? null,
    fallback_latency_budget_ms: metrics.fallback_latency_budget_ms ?? null,
    fallback_budget_violations: metrics.fallback_budget_violations ?? null,
  },
  backend: {
    active: snapshot?.activeBackend ?? null,
    production_webgpu_enabled: snapshot?.productionWebGpuEnabled ?? false,
  },
  metrics,
};

const outputPath = path.resolve(args.output);
await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");
console.log(`web renderer perf harness wrote ${outputPath}`);
