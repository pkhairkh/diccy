#!/usr/bin/env node

const HOST = process.env.RDVF_WASM_SMOKE_URL ?? "http://127.0.0.1:4173";

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
  appendElement(bytes, 0x0028, 0x0100, "US", [8, 0]);
  appendElement(bytes, 0x0028, 0x0101, "US", [8, 0]);
  appendElement(bytes, 0x0028, 0x0102, "US", [7, 0]);
  appendElement(bytes, 0x0028, 0x0103, "US", [0, 0]);
  appendElement(bytes, 0x7fe0, 0x0010, "OB", [0]);
  return Buffer.from(bytes);
}

async function main() {
  let playwright;
  try {
    playwright = await import("playwright");
  } catch (_error) {
    console.log("playwright dependency not found; skipping browser integration tests.");
    process.exit(0);
  }

  const { chromium } = playwright;
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  try {
    await page.goto(HOST, { waitUntil: "domcontentloaded" });
    await page.waitForSelector("#boot_status");
    const boot = (await page.locator("#boot_status").textContent()) ?? "";
    if (!boot.includes("WASM module loaded")) {
      throw new Error(`unexpected boot status: ${boot}`);
    }

    const diagnostics = (await page.locator("#backend_diagnostics").textContent()) ?? "";
    if (!diagnostics.includes("probe")) {
      throw new Error("backend diagnostics missing probe payload");
    }

    await page.locator("#production_webgpu_toggle").check();
    await page.locator("#dicom_file").setInputFiles({
      name: "integration-smoke.dcm",
      mimeType: "application/dicom",
      buffer: sampleP10Buffer(),
    });
    await page.waitForTimeout(250);
    const previewStatus = (await page.locator("#preview_status").textContent()) ?? "";
    if (!previewStatus.includes("Rendered from DICOM")) {
      throw new Error(`preview did not render expected output: ${previewStatus}`);
    }

    await page.locator("#simulate_device_lost").click();
    await page.locator("#recover_device").click();
    await page.waitForTimeout(100);
    const backendLabel = (await page.locator("#stat_backend").textContent()) ?? "";
    if (!/CPU|WebGPU/.test(backendLabel)) {
      throw new Error(`unexpected backend label: ${backendLabel}`);
    }
    const transitions = (await page.locator("#stat_transition_count").textContent()) ?? "";
    if (!transitions.trim() || transitions.trim() === "-") {
      throw new Error("transition metrics were not populated");
    }
  } finally {
    await browser.close();
  }

  console.log("web renderer integration: PASS");
}

await main();
