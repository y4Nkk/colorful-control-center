import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { inflateSync } from "node:zlib";
import postcss from "postcss";

export function assertVisiblePng(bytes) {
  const invalid = () => { throw new Error("Invalid or unsupported RGBA8 noninterlaced icon.png."); };
  if (!bytes.subarray(0, 8).equals(Buffer.from("89504e470d0a1a0a", "hex"))) invalid();
  let width, height, ended = false, dataEnded = false;
  const chunks = [];
  for (let offset = 8; offset < bytes.length;) {
    if (bytes.length - offset < 12) invalid();
    const length = bytes.readUInt32BE(offset);
    if (length > bytes.length - offset - 12) invalid();
    const type = bytes.toString("ascii", offset + 4, offset + 8);
    const data = bytes.subarray(offset + 8, offset + 8 + length);
    if (offset === 8 && type !== "IHDR") invalid();
    if (type === "IHDR") {
      if (offset !== 8 || length !== 13) invalid();
      width = data.readUInt32BE(0); height = data.readUInt32BE(4);
      if (!width || !height || width > 0x7fffffff || height > 0x7fffffff ||
          !data.subarray(8).equals(Buffer.from([8, 6, 0, 0, 0]))) invalid();
    } else if (type === "IDAT") {
      if (dataEnded) invalid();
      chunks.push(data);
    } else if (type === "IEND") {
      if (length !== 0 || offset + 12 !== bytes.length) invalid();
      ended = true;
    } else {
      // Ancillary metadata is irrelevant; other critical chunks are unsupported.
      if (!(bytes[offset + 4] & 32)) invalid();
      if (chunks.length) dataEnded = true;
    }
    offset += length + 12;
  }
  if (!ended || !chunks.length) invalid();
  const stride = width * 4 + 1;
  const size = stride * height;
  if (!Number.isSafeInteger(size)) invalid();
  const pixels = inflateSync(Buffer.concat(chunks), { maxOutputLength: size });
  if (pixels.length !== size) invalid();
  const alpha = Buffer.alloc(width);
  let visible = false;
  for (let y = 0; y < height; y++) {
    const row = y * stride, filter = pixels[row];
    if (filter > 4) invalid();
    let upperLeft = 0;
    for (let x = 0; x < width; x++) {
      const left = x ? alpha[x - 1] : 0, up = alpha[x];
      let predictor = 0;
      if (filter === 1) predictor = left;
      if (filter === 2) predictor = up;
      if (filter === 3) predictor = Math.floor((left + up) / 2);
      if (filter === 4) {
        const p = left + up - upperLeft;
        const a = Math.abs(p - left), b = Math.abs(p - up), c = Math.abs(p - upperLeft);
        predictor = a <= b && a <= c ? left : b <= c ? up : upperLeft;
      }
      // Alpha's left byte is four RGBA bytes away; RGB need not be reconstructed.
      alpha[x] = (pixels[row + 4 + x * 4] + predictor) & 255;
      upperLeft = up;
      if (alpha[x] !== 0) visible = true;
    }
  }
  if (!visible) throw new Error("Tauri generated a fully transparent icon.png.");
}

export const desktopIcons = [
  "32x32.png", "128x128.png", "128x128@2x.png", "icon.icns", "icon.ico", "icon.png",
  "Square30x30Logo.png", "Square44x44Logo.png", "Square71x71Logo.png",
  "Square89x89Logo.png", "Square107x107Logo.png", "Square142x142Logo.png",
  "Square150x150Logo.png", "Square284x284Logo.png", "Square310x310Logo.png", "StoreLogo.png",
];

// Tauri emits ICNS records in varying order; preserve every record and payload.
export function normalizeIcns(bytes) {
  if (bytes.length < 8 || bytes.toString("ascii", 0, 4) !== "icns" || bytes.readUInt32BE(4) !== bytes.length)
    throw new Error("Invalid ICNS header or length.");
  const chunks = [];
  for (let offset = 8; offset < bytes.length;) {
    if (bytes.length - offset < 8) throw new Error("Truncated ICNS chunk header.");
    const size = bytes.readUInt32BE(offset + 4);
    if (size < 8 || size > bytes.length - offset) throw new Error("Invalid ICNS chunk length.");
    chunks.push(bytes.subarray(offset, offset + size));
    offset += size;
  }
  return Buffer.concat([bytes.subarray(0, 8), ...chunks.sort(Buffer.compare)]);
}

function rasterize(cli, root, input, output, options = []) {
  const result = spawnSync(process.execPath, [cli, "icon", input, "--output", output, ...options], {
    cwd: root, stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`Tauri icon failed (${result.signal ?? result.status}).`);
}

function presetIcons(cli, root, temporary) {
  const directory = "src-tauri/icons/presets";
  const tokens = postcss.parse(readFileSync(join(root, "src/styles/tokens.css"), "utf8"));
  return ["quiet", "eco", "performance", "entertainment", "check"].map(palette => {
    const colors = new Map();
    tokens.walkRules(`[data-preset-palette="${palette}"]`, rule => {
      rule.walkDecls(/^--ccc-preset-/, value => { colors.set(value.prop, value.value); });
    });
    const source = readFileSync(join(root, directory, `${palette}.svg`), "utf8");
    const svg = source.replace(/var\((--ccc-preset-[\w-]+)\)/g, (_, token) => {
      const color = colors.get(token);
      if (!color || !/^#[a-f\d]{6}$/i.test(color))
        throw new Error(`Invalid SVG color token ${token} for ${palette}.`);
      return color;
    });
    const input = join(temporary, `${palette}.svg`);
    const output = join(temporary, palette);
    writeFileSync(input, svg);
    rasterize(cli, root, input, output, ["--png", "48"]);
    const png = readFileSync(join(output, "48x48.png"));
    assertVisiblePng(png);
    return [`${directory}/${palette}.png`, png];
  });
}

function main() {
  const args = process.argv.slice(2);
  if (args.length > 1 || (args.length === 1 && args[0] !== "--check"))
    throw new Error("Usage: node scripts/generate-icons.mjs [--check]");
  const check = args[0] === "--check";
  const root = fileURLToPath(new URL("..", import.meta.url));
  const source = readFileSync(join(root, "src-tauri/icons/icon.svg"));
  const cli = createRequire(import.meta.url).resolve("@tauri-apps/cli/tauri.js");
  const temporary = mkdtempSync(join(tmpdir(), "ccc-icons-"));
  try {
    const input = join(temporary, "icon.svg");
    const output = join(temporary, "icons");
    writeFileSync(input, source);
    rasterize(cli, root, input, output);

    // Read and validate the entire desktop set before touching any destination.
    const expected = [
      ["public/favicon.svg", source],
      ...desktopIcons.map(name => {
        const bytes = readFileSync(join(output, name));
        if (bytes.length === 0) throw new Error(`Tauri generated an empty icon: ${name}`);
        if (name === "icon.png") assertVisiblePng(bytes);
        return [`src-tauri/icons/${name}`, name === "icon.icns" ? normalizeIcns(bytes) : bytes];
      }),
      ...presetIcons(cli, root, temporary),
    ];
    if (check) {
      const issues = [];
      for (const [path, bytes] of expected) {
        try {
          const actual = readFileSync(join(root, path));
          if (!(path.endsWith(".icns") ? normalizeIcns(actual) : actual).equals(bytes))
            issues.push(`outdated: ${path}`);
        } catch (error) {
          if (error.code !== "ENOENT") throw error;
          issues.push(`missing: ${path}`);
        }
      }
      if (issues.length) throw new Error(`${issues.join("\n")}\nRun npm run generate:icons.`);
    } else {
      for (const [path, bytes] of expected) writeFileSync(join(root, path), bytes);
    }
    console.log(`ok: ${expected.length} icon assets ${check ? "checked" : "generated"}`);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
