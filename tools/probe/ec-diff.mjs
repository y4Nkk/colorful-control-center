// 离线比较既有 EC 快照；不加载驱动、不探测硬件、不修改 ec-map.json。
import { readFileSync } from "node:fs";
import { extname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

export function parseDump(input, binary = false) {
  if (binary) {
    if (input.length !== 256) throw new Error("二进制 EC 快照必须恰好为 256 字节");
    return Uint8Array.from(input);
  }
  const text = input[0] === 0xff && input[1] === 0xfe
    ? input.subarray(2).toString("utf16le") : input.toString("utf8").replace(/^\uFEFF/, "");
  const bytes = new Uint8Array(256);
  const rows = new Set();
  for (const line of text.replace(/\x1b\[[0-9;]*m/g, "").split(/\r?\n/)) {
    const row = line.match(/^\s*(?:0x)?([\da-f]{2})\s*[:|]\s*(.*?)\s*$/i);
    if (!row) continue;
    const at = Number.parseInt(row[1], 16);
    const values = row[2].split(/\s+/);
    if (at % 16 !== 0 || rows.has(at) || values.length !== 16 || values.some((v) => !/^[\da-f]{2}$/i.test(v))) {
      throw new Error(`无效或重复的 EC 行：${row[1]}`);
    }
    rows.add(at);
    values.forEach((value, index) => { bytes[at + index] = Number.parseInt(value, 16); });
  }
  if (rows.size !== 16) throw new Error(`EC 快照不完整：需要 16 行，实际 ${rows.size} 行`);
  return bytes;
}

export function compareDumps(before, after, repeatBefore, repeatAfter) {
  if ([before, after, repeatBefore, repeatAfter].some((b) => b.length !== 256)) {
    throw new Error("两轮前后快照均须包含 256 个寄存器");
  }
  const candidates = [];
  for (let at = 0; at < 256; at++) {
    if (before[at] === after[at] && repeatBefore[at] === repeatAfter[at]) continue;
    candidates.push({ register: `0x${at.toString(16).padStart(2, "0").toUpperCase()}`,
      first: [before[at], after[at]], repeat: [repeatBefore[at], repeatAfter[at]],
      sameValues: before[at] === repeatBefore[at] && after[at] === repeatAfter[at],
      sameDirection: Math.sign(after[at] - before[at]) === Math.sign(repeatAfter[at] - repeatBefore[at]) });
  }
  return { hardwareVerified: false, mapUpdated: false, candidates };
}

function main(args) {
  if (args.length !== 4) throw new Error("用法：node tools/probe/ec-diff.mjs before after repeat-before repeat-after");
  const dumps = args.map((path) => parseDump(readFileSync(path), extname(path).toLowerCase() === ".bin"));
  console.log(JSON.stringify(compareDumps(...dumps), null, 2));
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try { main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
