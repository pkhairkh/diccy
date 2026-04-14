#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { existsSync } from "node:fs";
import { createHash } from "node:crypto";

const SCRIPT_DIR = new URL(".", import.meta.url).pathname;
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const HOST = process.env.RDVF_WASM_SMOKE_URL ?? "http://127.0.0.1:4173";
const BASELINE_DIR = path.resolve(ROOT_DIR, "tools", "ui-flow-baselines");
const REPORT_DIR = path.resolve(ROOT_DIR, "reports", "visual-regression");
const RECORD_MODE = process.env.RDVF_UI_FLOW_SCREENSHOT_RECORD === "1";
const SKIP = process.env.RDVF_UI_FLOW_SCREENSHOT_SKIP === "1";

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

function sampleP10Buffer() {
  const bytes = [];
  for (let i = 0; i < 128; i += 1) {
    bytes.push(0);
  }
  bytes.push(...asciiBytes("DICM"));
  appendElement(bytes, 0x0002, 0x0010, "UI", asciiBytes("1.2.840.10008.1.2.1"));
  appendElement(bytes, 0x0008, 0x0016, "UI", asciiBytes("1.2.840.10008.5.1.4.1.1.7"));
  appendElement(bytes, 0x0008, 0x0018, "UI", asciiBytes("1.2.3.4.5"));
  appendElement(bytes, 0x0020, 0x000d, "UI", asciiBytes("1.2.3"));
  appendElement(bytes, 0x0020, 0x000e, "UI", asciiBytes("1.2.3.4"));
  appendElement(bytes, 0x0028, 0x0002, "US", [1, 0]);
  appendElement(bytes, 0x0028, 0x0004, "CS", asciiBytes("MONOCHROME2"));
  appendElement(bytes, 0x0028, 0x0010, "US", [1, 0]);
  appendElement(bytes, 0x0028, 0x0011, "US", [1, 0]);
  appendElement(bytes, 0x0028, 0x0100, "US", [16, 0]);
  appendElement(bytes, 0x0028, 0x0101, "US", [12, 0]);
  appendElement(bytes, 0x0028, 0x0102, "US", [11, 0]);
  appendElement(bytes, 0x0028, 0x0103, "US", [0, 0]);
  appendElement(bytes, 0x7fe0, 0x0010, "OB", [0]);
  return Buffer.from(bytes);
}

function sha256Hex(data) {
  return createHash("sha256").update(data).digest("hex");
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function loadPlaywright() {
  try {
    return await import("playwright");
  } catch (_error) {
    console.log("playwright dependency not found; skipping browser visual flow diff.");
    process.exit(0);
  }
}

async function waitForText(page, selector, pattern, label) {
  const regex = typeof pattern === "string" ? new RegExp(pattern) : pattern;
  const deadlineMs = Date.now() + 12000;
  while (Date.now() < deadlineMs) {
    const text = (await page.locator(selector).textContent()) ?? "";
    if (regex.test(text)) {
      return text;
    }
    await page.waitForTimeout(100);
  }
  throw new Error(`${label} did not reach expected status (${pattern}); last='${await page.locator(selector).textContent()}'.`);
}

async function setFieldValue(page, selector, value) {
  const field = page.locator(selector);
  await field.scrollIntoViewIfNeeded();
  await field.fill(String(value));
}

async function setSelectValue(page, selector, value) {
  const field = page.locator(selector);
  await field.selectOption(String(value));
}

async function runAndCaptureMpr(page, config) {
  await setFieldValue(page, "#mpr_crosshair_x", config.crosshairX);
  await setFieldValue(page, "#mpr_crosshair_y", config.crosshairY);
  await setFieldValue(page, "#mpr_crosshair_z", config.crosshairZ);
  await setFieldValue(page, "#mpr_output_width", config.width);
  await setFieldValue(page, "#mpr_output_height", config.height);
  await setFieldValue(page, "#mpr_slab_thickness", config.slabThickness);
  await setSelectValue(page, "#mpr_slab_mode", config.slabMode);
  await page.locator("#render_mpr").click();
  const status = await waitForText(page, "#mpr_status", /Tri-planar MPR rendered/, "MPR render");
  if (!status) {
    throw new Error("MPR render did not complete.");
  }
}

async function runFusion(page, config) {
  await setFieldValue(page, "#fusion_alpha", config.alpha);
  await setSelectValue(page, "#fusion_colormap", config.colormap);
  await setFieldValue(page, "#fusion_suv_scale", config.suvScale);
  await page.locator("#run_fusion").click();
  await waitForText(page, "#fusion_status", /Fusion preview rendered/, "fusion render");
}

async function compareOrRecordSnapshot(name, imageBytes) {
  await ensureDir(BASELINE_DIR);
  const baselinePath = path.join(BASELINE_DIR, `${name}.png`);
  if (RECORD_MODE) {
    await fs.writeFile(baselinePath, imageBytes);
    console.log(`recorded snapshot: ${baselinePath}`);
    return { status: "recorded", hash: sha256Hex(imageBytes) };
  }

  if (!existsSync(baselinePath)) {
    throw new Error(`missing baseline snapshot: ${baselinePath}`);
  }

  const baseline = await fs.readFile(baselinePath);
  const match = baseline.equals(imageBytes);
  if (!match) {
    const actualPath = path.join(REPORT_DIR, `${name}.actual.png`);
    await ensureDir(REPORT_DIR);
    await fs.writeFile(actualPath, imageBytes);
    throw new Error(`snapshot mismatch: ${name} (actual written to ${actualPath})`);
  }
  return { status: "pass", hash: sha256Hex(imageBytes) };
}

async function screenshotCanvas(page, selector, name, results) {
  const locator = page.locator(selector);
  await locator.scrollIntoViewIfNeeded();
  const image = await locator.screenshot({ type: "png", animations: "disabled" });
  const compareResult = await compareOrRecordSnapshot(name, image);
  results.push({ flow: name, ...compareResult, bytes: image.length });
}

async function main() {
  if (SKIP) {
    console.log("RDVF_UI_FLOW_SCREENSHOT_SKIP=1, skipping UI visual regression flow automation.");
    return;
  }

  const playwright = await loadPlaywright();
  const { chromium } = playwright;
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1365, height: 1400 } });

  const runResults = [];
  try {
    await page.goto(HOST, { waitUntil: "domcontentloaded" });
    await waitForText(page, "#boot_status", /WASM module loaded/, "bootstrap");
    await waitForText(page, "#backend_diagnostics", /probe/, "backend diagnostics");

    const webgpuToggle = page.locator("#production_webgpu_toggle");
    if (await webgpuToggle.isChecked()) {
      await webgpuToggle.uncheck();
    }

    await page.locator("#dicom_file").setInputFiles({
      name: "flow-smoke.dcm",
      mimeType: "application/dicom",
      buffer: sampleP10Buffer(),
    });
    await waitForText(page, "#preview_status", /Rendered from DICOM/, "DICOM preview");

    await runAndCaptureMpr(page, {
      crosshairX: 0,
      crosshairY: 0,
      crosshairZ: 0,
      width: 128,
      height: 128,
      slabThickness: 1,
      slabMode: "average",
    });
    await screenshotCanvas(page, "#mpr_axial_canvas", "mpr-axial-baseline", runResults);

    await runAndCaptureMpr(page, {
      crosshairX: 0,
      crosshairY: 0,
      crosshairZ: 0,
      width: 128,
      height: 128,
      slabThickness: 3,
      slabMode: "max",
    });
    await screenshotCanvas(page, "#mpr_axial_canvas", "mpr-slab-max", runResults);

    await runFusion(page, {
      alpha: 0.45,
      colormap: "hotiron",
      suvScale: 5.0,
    });
    await screenshotCanvas(page, "#fusion_canvas", "fusion-baseline", runResults);
  } finally {
    await browser.close();
  }

  const summary = {
    ran_at: new Date().toISOString(),
    host: HOST,
    mode: RECORD_MODE ? "record" : "diff",
    flows: runResults,
    count: runResults.length,
  };
  await ensureDir(REPORT_DIR);
  await fs.writeFile(path.join(REPORT_DIR, "ui-flow-screenshot-regression.json"), `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  console.log(`UI flow screenshot ${RECORD_MODE ? "record" : "diff"} run complete: ${runResults.length} flow image(s).`);
}

await main();
