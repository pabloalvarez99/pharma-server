// Tests for pull-catalog.mjs SQL generation.
// Run: node scripts/web-sync/test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, join } from "node:path";

import { sqlString, sqlInt, buildSql } from "./pull-catalog.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const SCRIPT_URL = pathToFileURL(join(HERE, "pull-catalog.mjs")).href;

test("sqlString accepts product names with spaces (NUL-guard regression)", () => {
  // Previously the guard checked for " " (space) instead of "\0", which made
  // the script die(3) on every real product name.
  assert.equal(sqlString("Paracetamol 500mg x20"), "'Paracetamol 500mg x20'");
});

test("sqlString doubles single quotes", () => {
  assert.equal(sqlString("O'Brien"), "'O''Brien'");
});

test("sqlString maps null/undefined to NULL", () => {
  assert.equal(sqlString(null), "NULL");
  assert.equal(sqlString(undefined), "NULL");
});

test("sqlInt validates integers", () => {
  assert.equal(sqlInt(42), "42");
  assert.equal(sqlInt(null), "NULL");
});

test("sqlString rejects a literal NUL byte (exit 3)", () => {
  // Drive a child via stdin to avoid passing a NUL through argv/shell layers.
  const r = spawnSync(process.execPath, ["--input-type=module"], {
    input: `import('${SCRIPT_URL}').then(m => { m.sqlString('a\\u0000b'); });`,
    encoding: "utf8",
  });
  assert.equal(r.status, 3, `expected exit 3, got ${r.status} (stderr: ${r.stderr})`);
  assert.match(r.stderr, /NUL byte/);
});

test("buildSql tombstone uses structured skus, not regex on rendered SQL", () => {
  // A sku containing a comma must survive into the NOT IN clause intact.
  const items = [
    { sku: "ABC,123", name: "Comma SKU", price_clp: 1000 },
    { sku: "XYZ-9", name: "Normal", price_clp: 2000 },
  ];
  const valueRows = ["('ABC,123', ...)", "('XYZ-9', ...)"];
  const sql = buildSql(items, valueRows, "tufarmacia", "2026-05-24T00:00:00Z");
  assert.match(sql, /NOT IN \(/);
  assert.match(sql, /'ABC,123'/); // comma-containing sku quoted whole
  assert.match(sql, /'XYZ-9'/);
});

test("buildSql with no rows emits a no-op transaction (no NOT IN)", () => {
  const sql = buildSql([], [], "tufarmacia", "2026-05-24T00:00:00Z");
  assert.match(sql, /No-op transaction/);
  assert.doesNotMatch(sql, /NOT IN/);
});
