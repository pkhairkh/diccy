import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const componentsRoot = path.join(repoRoot, "frontend", "src", "components");

const LAYERS = ["primitives", "components", "composites", "panels", "pages"];
const allowedImports = {
  primitives: new Set(["primitives"]),
  components: new Set(["primitives", "components"]),
  composites: new Set(["primitives", "components", "composites"]),
  panels: new Set(["components", "composites", "panels"]),
  pages: new Set(["panels", "pages"]),
};

function walk(dir) {
  if (!fs.existsSync(dir)) return [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const next = path.join(dir, entry.name);
    if (entry.isDirectory()) files.push(...walk(next));
    if (entry.isFile() && next.endsWith(".vue")) files.push(next);
  }
  return files;
}

function layerFor(filePath) {
  const normalized = filePath.replaceAll("\\", "/");
  for (const layer of LAYERS) {
    if (normalized.includes(`/components/${layer}/`)) return layer;
  }
  return null;
}

function parseImports(text) {
  const imports = [];
  const regex = /from\s+["']([^"']+)["']/g;
  let match = null;
  while ((match = regex.exec(text)) !== null) imports.push(match[1]);
  return imports;
}

function resolveImport(sourceFile, specifier) {
  if (specifier.startsWith(".")) {
    const resolved = path.resolve(path.dirname(sourceFile), specifier);
    return resolved.endsWith(".vue") ? resolved : `${resolved}.vue`;
  }
  if (specifier.startsWith("@/")) {
    return path.join(repoRoot, "frontend", "src", specifier.slice(2));
  }
  return null;
}

const violations = [];
for (const file of walk(componentsRoot)) {
  const sourceLayer = layerFor(file);
  if (!sourceLayer) continue;
  const imports = parseImports(fs.readFileSync(file, "utf8"));
  for (const specifier of imports) {
    const target = resolveImport(file, specifier);
    if (!target) continue;
    const targetLayer = layerFor(target);
    if (!targetLayer) continue;
    if (!allowedImports[sourceLayer].has(targetLayer)) {
      violations.push(
        `${path.relative(repoRoot, file)} imports ${path.relative(repoRoot, target)} (${sourceLayer} -> ${targetLayer})`,
      );
    }
  }
}

assert.equal(
  violations.length,
  0,
  `Layer import violations:\n${violations.map((item) => `- ${item}`).join("\n")}`,
);

