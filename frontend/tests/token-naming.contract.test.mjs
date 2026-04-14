import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";

const repoRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..", "..");
const extractedPath = path.join(repoRoot, "frontend", "src", "styles", "extractedTokens.ts");
const semanticPath = path.join(repoRoot, "frontend", "src", "styles", "semanticTokens.ts");
const classTokenPath = path.join(repoRoot, "frontend", "src", "styles", "tokens.ts");

function readKeys(filePath) {
  const text = fs.readFileSync(filePath, "utf8");
  const keyRegex = /^\s{2}([A-Za-z0-9]+):/gm;
  const keys = [];
  let match = null;
  while ((match = keyRegex.exec(text)) !== null) {
    keys.push(match[1]);
  }
  return keys;
}

function assertUnique(keys, scope) {
  assert.equal(keys.length, new Set(keys).size, `${scope} contains duplicate token keys`);
}

const extractedKeys = readKeys(extractedPath);
const semanticKeys = readKeys(semanticPath);
const classTokenKeys = readKeys(classTokenPath);

assert.ok(extractedKeys.length > 0, "extracted tokens must not be empty");
assert.ok(semanticKeys.length > 0, "semantic tokens must not be empty");
assert.ok(classTokenKeys.length > 0, "class tokens must not be empty");

assertUnique(extractedKeys, "extractedTokens");
assertUnique(semanticKeys, "semanticTokens");
assertUnique(classTokenKeys, "classTokens");

for (const key of extractedKeys) {
  assert.match(key, /^(page|panel|composite|component|metric)[A-Z0-9][A-Za-z0-9]*$/, `invalid extracted token key: ${key}`);
  assert.ok(!/_l\d+_c\d+/.test(key), `line/column encoded token names are forbidden: ${key}`);
}

for (const key of semanticKeys) {
  assert.match(key, /^connector[A-Z0-9][A-Za-z0-9]*$/, `invalid semantic token key: ${key}`);
  assert.ok(!/_l\d+_c\d+/.test(key), `line/column encoded token names are forbidden: ${key}`);
}

for (const key of classTokenKeys) {
  assert.match(key, /^[a-z][A-Za-z0-9]*$/, `invalid class token key: ${key}`);
  assert.ok(!/_l\d+_c\d+/.test(key), `line/column encoded token names are forbidden: ${key}`);
}
