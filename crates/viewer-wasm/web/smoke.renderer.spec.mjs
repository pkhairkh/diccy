import { test, expect } from "@playwright/test";

const HOST = process.env.RDVF_WASM_SMOKE_URL ?? "http://127.0.0.1:4173";

function u16le(bytes, value) {
  bytes.push(value & 0xff, (value >> 8) & 0xff);
}

function u32le(bytes, value) {
  bytes.push(value & 0xff, (value >> 8) & 0xff, (value >> 16) & 0xff, (value >> 24) & 0xff);
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

function asciiBytes(value) {
  return [...Buffer.from(value, "ascii")];
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

test.describe("viewer-wasm renderer smoke", () => {
  test("boots and exposes backend diagnostics", async ({ page }) => {
    await page.goto(HOST, { waitUntil: "domcontentloaded" });
    await expect(page.locator("#boot_status")).toContainText("WASM module loaded");
    await expect(page.locator("#renderer_badge")).not.toContainText("-");
    await expect(page.locator("#backend_diagnostics")).toContainText("probe");
  });

  test("covers probe -> backend select -> render -> fallback flow", async ({ page }) => {
    await page.goto(HOST, { waitUntil: "domcontentloaded" });
    await page.locator("#production_webgpu_toggle").check();

    await page.locator("#dicom_file").setInputFiles({
      name: "smoke.dcm",
      mimeType: "application/dicom",
      buffer: sampleP10Buffer(),
    });

    await expect(page.locator("#ingest_status")).toContainText("Accepted");
    await expect(page.locator("#preview_status")).toContainText("Rendered from DICOM");
    await expect(page.locator("#stat_transition_count")).not.toContainText("-");

    await page.locator("#simulate_device_lost").click();
    await expect(page.locator("#stat_backend")).toContainText(/CPU|WebGPU/);
    await page.locator("#recover_device").click();
    await expect(page.locator("#stat_backend")).toContainText(/CPU|WebGPU/);
    await expect(page.locator("#backend_diagnostics")).toContainText("metrics");
  });
});
