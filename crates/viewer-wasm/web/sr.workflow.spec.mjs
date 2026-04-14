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

function sampleP10Buffer(suffix = "1") {
  const bytes = [];
  for (let i = 0; i < 128; i += 1) {
    bytes.push(0);
  }
  bytes.push(...asciiBytes("DICM"));

  appendElement(bytes, 0x0002, 0x0010, "UI", asciiBytes("1.2.840.10008.1.2.1"));
  appendElement(bytes, 0x0008, 0x0016, "UI", asciiBytes("1.2.840.10008.5.1.4.1.1.7"));
  appendElement(bytes, 0x0008, 0x0018, "UI", asciiBytes(`1.2.3.4.5.${suffix}`));
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

test.describe("viewer-wasm SR workflow integration", () => {
  test.skip(
    process.env.RDVF_WORKFLOW_INTEGRATION !== "1",
    "Set RDVF_WORKFLOW_INTEGRATION=1 and run with local workflow server.",
  );

  test("executes create -> update -> retrieve and list from host UI", async ({ page }) => {
    await page.goto(HOST, { waitUntil: "domcontentloaded" });

    await page.locator("#dicom_file").setInputFiles({
      name: "sr-integration.dcm",
      mimeType: "application/dicom",
      buffer: sampleP10Buffer("sr"),
    });
    await expect(page.locator("#ingest_status")).toContainText("Accepted");

    await page.locator("#sr_role").selectOption("writer");
    await page.locator("#sr_study_uid").fill("1.2.3");
    await page.locator("#sr_series_uid").fill("1.2.3.4");
    await page.locator("#sr_sop_uid").fill("1.2.3.4.5.6");
    await page.locator("#sr_known_refs").fill("9.8.7");
    await page.locator("#sr_num_value").fill("12.5");

    await page.locator("#sr_create").click();
    await expect(page.locator("#sr_status")).toContainText("SR create committed");

    await page.locator("#sr_text_value").fill("updated finding");
    await page.locator("#sr_item_kind").selectOption("text");
    await page.locator("#sr_update").click();
    await expect(page.locator("#sr_status")).toContainText("SR update committed");

    await page.locator("#sr_load").click();
    await expect(page.locator("#sr_status")).toContainText("SR document loaded");
    await expect(page.locator("#sr_detail_output")).toContainText("version");

    await page.locator("#sr_list").click();
    await expect(page.locator("#sr_list_output")).toContainText("sop_instance_uid");
  });
});
