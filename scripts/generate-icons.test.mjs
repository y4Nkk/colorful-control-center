import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { deflateSync } from "node:zlib";
import { assertVisiblePng, desktopIcons, normalizeIcns } from "./generate-icons.mjs";

function pngChunk(type, data) {
  const body = Buffer.concat([Buffer.from(type), data]);
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  const length = Buffer.alloc(4), checksum = Buffer.alloc(4);
  length.writeUInt32BE(data.length); checksum.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([length, body, checksum]);
}

const pngHeader = Buffer.from("00000003000000030806000000", "hex");
function pngData(raw, header = pngHeader) {
  return Buffer.concat([Buffer.from("89504e470d0a1a0a", "hex"), pngChunk("IHDR", header),
    pngChunk("IDAT", deflateSync(raw)), pngChunk("IEND", Buffer.alloc(0))]);
}

function png(alpha = 1, filter = 0) {
  // Hand-derived residuals for alpha rows [10,80,130], [90,20,120], [40,100,alpha].
  // The Paeth case exercises left, up, upper-left and tie selection; RGB is zero.
  const residuals = [
    [10, 80, 130, 90, 20, 120, 40, 100, alpha],
    [10, 70, 50, 90, -70, 100, 40, 60, alpha - 100],
    [10, 80, 130, 80, -60, -10, -50, 80, alpha - 120],
    [10, 75, 90, 85, -65, 45, -5, 70, alpha - 110],
    [10, 70, 50, 80, -70, 40, -50, 80, alpha - 120],
  ];
  return pngData(Buffer.from(residuals[filter].flatMap((value, index) =>
    [...(index % 3 === 0 ? [filter] : []), 0, 0, 0, alpha === 0 ? 0 : value & 255])));
}

function icns(records) {
  const chunks = records.map(([type, payload]) => {
    const bytes = Buffer.from(payload);
    const header = Buffer.alloc(8);
    header.write(type, 0, 4, "ascii");
    header.writeUInt32BE(bytes.length + 8, 4);
    return Buffer.concat([header, bytes]);
  });
  const header = Buffer.from("icns\0\0\0\0", "ascii");
  header.writeUInt32BE(8 + chunks.reduce((size, chunk) => size + chunk.length, 0), 4);
  return Buffer.concat([header, ...chunks]);
}

const presetPalettes = ["quiet", "eco", "performance", "entertainment"];
// 托盘勾选标记不取配色令牌。
const checkSource = '<svg xmlns="http://www.w3.org/2000/svg" stroke="#000"/>';
const presetAssets = [...presetPalettes, "check"];

function preparePresetFixture(root) {
  for (const palette of presetPalettes) {
    writeFileSync(join(root, "src-tauri/icons/presets", `${palette}.svg`),
      `<svg xmlns="http://www.w3.org/2000/svg" data-preset-palette="${palette}" fill="var(--ccc-preset-mid)"/>`);
  }
  writeFileSync(join(root, "src-tauri/icons/presets", "check.svg"), checkSource);
  const tokens = presetPalettes.map(palette =>
    `[data-preset-palette="${palette}"] { --ccc-preset-mid: ${palette === "quiet" ? "#406abb" : "#127069"}; }`).join("\n");
  writeFileSync(join(root, "src/styles/tokens.css"), tokens);
  writeFileSync(join(root, "node_modules/postcss/package.json"), '{"main":"index.mjs"}');
  writeFileSync(join(root, "node_modules/postcss/index.mjs"),
    `export { default } from ${JSON.stringify(import.meta.resolve("postcss"))};`);
}

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "ccc icons test-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const directory of ["scripts", "public", "src/styles", "src-tauri/icons/presets",
    "node_modules/postcss", "node_modules/@tauri-apps/cli"])
    mkdirSync(join(root, directory), { recursive: true });
  copyFileSync(new URL("./generate-icons.mjs", import.meta.url), join(root, "scripts/generate-icons.mjs"));
  writeFileSync(join(root, "src-tauri/icons/icon.svg"), '<svg xmlns="http://www.w3.org/2000/svg"/>');
  preparePresetFixture(root);
  const paths = ["public/favicon.svg", ...desktopIcons.map(name => `src-tauri/icons/${name}`),
    ...presetAssets.map(name => `src-tauri/icons/presets/${name}.png`)];
  for (const path of paths) writeFileSync(join(root, path), "old asset");
  // A local CLI double exercises the actual script without rendering the changing user source.
  writeFileSync(join(root, "node_modules/@tauri-apps/cli/package.json"), '{"name":"@tauri-apps/cli","type":"commonjs"}');
  writeFileSync(join(root, "node_modules/@tauri-apps/cli/tauri.js"), `
    const assert = require("node:assert/strict");
    const fs = require("node:fs");
    const path = require("node:path");
    const [command, input, option, output, ...options] = process.argv.slice(2);
    assert.equal(command, "icon"); assert.equal(option, "--output");
    assert.ok(!input.startsWith(process.cwd() + path.sep));
    assert.equal(path.dirname(input), path.dirname(output));
    fs.writeFileSync("cli-call.json", JSON.stringify({ input, output }));
    if (process.env.CCC_TEST_ICON_MODE === "fail") process.exit(23);
    const source = fs.readFileSync(input);
    fs.mkdirSync(output);
    const image = Buffer.from(${JSON.stringify(png().toString("base64"))}, "base64");
    if (options.length) {
      assert.deepEqual(options, ["--png", "48"]);
      const color = path.basename(input) === "quiet.svg" ? "#406abb" : "#127069";
      assert.ok(source.includes('fill="' + color + '"') || source.equals(Buffer.from(${JSON.stringify(checkSource)})));
      fs.writeFileSync(path.join(output, "48x48.png"), image);
      process.exit(0);
    }
    for (const name of ${JSON.stringify(desktopIcons)}) {
      if (process.env.CCC_TEST_ICON_MODE === "missing" && name === "StoreLogo.png") continue;
      let bytes = process.env.CCC_TEST_ICON_MODE === "empty" && name === "StoreLogo.png"
        ? Buffer.alloc(0) : Buffer.concat([source, Buffer.from(name)]);
      if (name === "icon.icns") bytes = (${icns.toString()})([["ic10", source], ["ic07", "small"]]);
      if (name === "icon.png") bytes = image;
      fs.writeFileSync(path.join(output, name), bytes);
    }
    for (const directory of ["android", "ios"]) fs.mkdirSync(path.join(output, directory));
  `);
  const run = (args = [], mode = "") => spawnSync(process.execPath, [join(root, "scripts/generate-icons.mjs"), ...args], {
    cwd: root, encoding: "utf8", env: { ...process.env, CCC_TEST_ICON_MODE: mode },
  });
  const snapshot = () => paths.map(path => existsSync(join(root, path)) ? readFileSync(join(root, path)) : null);
  const cleaned = () => {
    const { input, output } = JSON.parse(readFileSync(join(root, "cli-call.json"), "utf8"));
    assert.equal(existsSync(input), false);
    assert.equal(existsSync(output), false);
    assert.equal(existsSync(join(root, "src-tauri/icons/icon.svg")), true);
  };
  return { root, paths, run, snapshot, cleaned };
}

test("生成完整桌面清单、同源 favicon 和托盘图标，且不复制移动端产物", t => {
  const f = fixture(t);
  assert.equal(desktopIcons.length, 16);
  const generated = f.run();
  assert.equal(generated.status, 0, generated.stderr);
  const source = readFileSync(join(f.root, "src-tauri/icons/icon.svg"));
  assert.deepEqual(readFileSync(join(f.root, "public/favicon.svg")), source);
  for (const name of desktopIcons) {
    const expected = name === "icon.icns"
      ? icns([["ic07", "small"], ["ic10", source]])
      : name === "icon.png" ? png() : Buffer.concat([source, Buffer.from(name)]);
    assert.deepEqual(readFileSync(join(f.root, "src-tauri/icons", name)), expected);
  }
  assert.deepEqual(readdirSync(join(f.root, "src-tauri/icons")).sort(), [...desktopIcons, "icon.svg", "presets"].sort());
  for (const name of presetAssets)
    assertVisiblePng(readFileSync(join(f.root, "src-tauri/icons/presets", `${name}.png`)));
  f.cleaned();
  const before = f.snapshot();
  const checked = f.run(["--check"]);
  assert.equal(checked.status, 0, checked.stderr);
  assert.deepEqual(f.snapshot(), before);
  f.cleaned();
});

for (const [path, missing] of [["src-tauri/icons/icon.png", true], ["src-tauri/icons/icon.ico", false],
  ["public/favicon.svg", false], ["src-tauri/icons/presets/eco.png", false], ["src-tauri/icons/presets/quiet.png", true]]) {
  test(`检查拒绝${missing ? "缺失" : "过期或不一致"}的 ${path}，不写资产`, t => {
    const f = fixture(t);
    assert.equal(f.run().status, 0);
    if (missing) rmSync(join(f.root, path));
    else writeFileSync(join(f.root, path), "stale");
    const before = f.snapshot();
    const result = f.run(["--check"]);
    assert.equal(result.status, 1);
    assert.ok(result.stderr.includes(`${missing ? "missing" : "outdated"}: ${path}`), result.stderr);
    assert.deepEqual(f.snapshot(), before);
    f.cleaned();
  });
}

test("ICNS 规范化只消除记录顺序差异，保留重复记录、长度及 payload", () => {
  const records = [["ic10", "large"], ["ic07", "small"], ["ic07", "another"]];
  const canonical = normalizeIcns(icns(records));
  assert.deepEqual(canonical, normalizeIcns(icns([...records].reverse())));
  assert.deepEqual(canonical, icns([["ic07", "small"], ["ic07", "another"], ["ic10", "large"]]));
  assert.deepEqual(normalizeIcns(canonical), canonical);
  assert.notDeepEqual(canonical, normalizeIcns(icns([["ic10", "Large"], ...records.slice(1)])));
  assert.notDeepEqual(canonical, normalizeIcns(icns(records.slice(1))));
});

test("ICNS 拒绝错误文件头、文件长度和越界或截断的记录", () => {
  const valid = icns([["ic07", "payload"]]);
  const badMagic = Buffer.from(valid); badMagic.write("xxxx");
  const badLength = Buffer.from(valid); badLength.writeUInt32BE(valid.length + 1, 4);
  const shortChunk = Buffer.from(valid); shortChunk.writeUInt32BE(7, 12);
  const longChunk = Buffer.from(valid); longChunk.writeUInt32BE(valid.length, 12);
  const trailing = Buffer.concat([valid, Buffer.from([1])]); trailing.writeUInt32BE(trailing.length, 4);
  for (const bytes of [Buffer.alloc(0), badMagic, badLength, shortChunk, longChunk, trailing])
    assert.throws(() => normalizeIcns(bytes), /ICNS/);
});

test("检查接受 ICNS 记录顺序变化，仍拒绝 payload 变化且不写目标", t => {
  const f = fixture(t);
  assert.equal(f.run().status, 0);
  const source = readFileSync(join(f.root, "src-tauri/icons/icon.svg"));
  const path = join(f.root, "src-tauri/icons/icon.icns");
  writeFileSync(path, icns([["ic10", source], ["ic07", "small"]]));
  const before = f.snapshot();
  const reordered = f.run(["--check"]);
  assert.equal(reordered.status, 0, reordered.stderr);
  assert.deepEqual(f.snapshot(), before);
  writeFileSync(path, icns([["ic10", source], ["ic07", "SMALL"]]));
  const changed = f.snapshot();
  const outdated = f.run(["--check"]);
  assert.equal(outdated.status, 1);
  assert.match(outdated.stderr, /outdated: src-tauri\/icons\/icon\.icns/);
  assert.deepEqual(f.snapshot(), changed);
  f.cleaned();
});

for (const mode of ["fail", "missing", "empty"]) {
  test(`CLI ${mode} 时非零退出，全部目标保持原样且清理临时目录`, t => {
    const f = fixture(t);
    const before = f.snapshot();
    const result = f.run([], mode);
    assert.equal(result.status, 1);
    assert.match(result.stderr, mode === "fail" ? /Tauri icon failed \(23\)/ : /StoreLogo\.png/);
    assert.deepEqual(f.snapshot(), before);
    f.cleaned();
  });
}
