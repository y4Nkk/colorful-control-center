import { test } from "node:test";
import assert from "node:assert/strict";
import { compareDumps, parseDump } from "./ec-diff.mjs";

const table = () => Array.from({ length: 16 }, (_, row) => `${(row * 16).toString(16).padStart(2, "0")} | ${
  Array.from({ length: 16 }, (_, column) => (row * 16 + column).toString(16).padStart(2, "0")).join(" ")}`).join("\n");

test("parses full NBFC tables including ANSI colors and PowerShell UTF16", () => {
  const ansi = `\x1b[1;34m${table()}\x1b[0m`;
  assert.deepEqual(parseDump(Buffer.from(ansi)), Uint8Array.from({ length: 256 }, (_, i) => i));
  assert.deepEqual(parseDump(Buffer.concat([Buffer.from([255, 254]), Buffer.from(table(), "utf16le")])),
    Uint8Array.from({ length: 256 }, (_, i) => i));
});

test("rejects incomplete, duplicate, misaligned and invalid-byte tables", () => {
  for (const bad of [table().split("\n").slice(1).join("\n"), `${table()}\n${table()}`,
    table().replace("00 |", "01 |"), table().replace("01 02", "XX 02")]) {
    assert.throws(() => parseDump(Buffer.from(bad)));
  }
  assert.throws(() => parseDump(Buffer.alloc(255), true));
});

test("separates repeatable candidates from natural drift without verifying hardware", () => {
  const before = new Uint8Array(256), after = new Uint8Array(256);
  const repeatBefore = new Uint8Array(256), repeatAfter = new Uint8Array(256);
  after[0xe7] = repeatAfter[0xe7] = 60;
  after[0xcf] = 40; repeatAfter[0xcf] = 42;
  const result = compareDumps(before, after, repeatBefore, repeatAfter);
  assert.equal(result.candidates.length, 2);
  assert.equal(result.candidates.find((c) => c.register === "0xE7").sameValues, true);
  assert.equal(result.candidates.find((c) => c.register === "0xCF").sameValues, false);
  assert.equal(result.hardwareVerified, false);
  assert.equal(result.mapUpdated, false);
});
