/**
 * 全局化边界检查（AGENTS.md / DESIGN.md 契约的可执行形式，build 前置）。
 *
 * 1. IPC 边界：`invoke(` 与 `@tauri-apps/` import 只允许出现在 src/api/
 * 2. 令牌所有权：`--ccc-*` 自定义属性定义只允许出现在 src/styles/tokens.css
 * 3. 页面接线一致：PAGES 注册的每个页面必须在 AppWorkspace 有对应分支
 *
 * Run: node scripts/check-boundaries.mjs
 */
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = fileURLToPath(new URL("..", import.meta.url));
const srcDir = join(rootDir, "src");
const violations = [];

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(full);
    else yield full;
  }
}

const rel = (path) => relative(rootDir, path).replaceAll("\\", "/");

/** 1. IPC 边界 + 2. 令牌所有权 */
for (const file of walk(srcDir)) {
  const path = rel(file);
  if (file.endsWith(".ts") || file.endsWith(".tsx")) {
    if (!path.startsWith("src/api/")) {
      const content = readFileSync(file, "utf8");
      if (/\binvoke\s*\(/.test(content)) {
        violations.push(`${path}: invoke() 只允许出现在 src/api/（IPC 唯一边界，见 AGENTS.md）`);
      }
      if (content.includes("@tauri-apps/")) {
        violations.push(`${path}: @tauri-apps import 只允许出现在 src/api/`);
      }
    }
  }
  if (file.endsWith(".css") && path !== "src/styles/tokens.css") {
    const content = readFileSync(file, "utf8");
    const lines = content.split(/\r?\n/);
    lines.forEach((line, index) => {
      if (/^\s*--ccc-[\w-]+\s*:/.test(line)) {
        violations.push(
          `${path}:${index + 1}: --ccc-* 变量定义只允许出现在 src/styles/tokens.css（令牌唯一所有者）`,
        );
      }
    });
  }
}

/** 3. 页面接线一致：PAGES ↔ AppWorkspace 分支 */
const navigation = readFileSync(join(srcDir, "app", "navigation.ts"), "utf8");
const appWorkspace = readFileSync(join(srcDir, "app", "AppWorkspace.tsx"), "utf8");
const pageIds = [...navigation.matchAll(/id: "([a-z]+)"/g)].map((match) => match[1]);
if (pageIds.length === 0) violations.push("src/app/navigation.ts: PAGES 未找到任何页面注册");
for (const id of pageIds) {
  const component = `${id[0].toUpperCase()}${id.slice(1)}Workspace`;
  if (!appWorkspace.includes(component)) {
    violations.push(
      `src/app/AppWorkspace.tsx: 页面 "${id}" 已在 PAGES 注册但缺少 ${component} 分支（keep-alive 接线）`,
    );
  }
}

if (violations.length) {
  for (const violation of violations) console.error(violation);
  process.exit(1);
}
console.log(`ok: 边界检查通过（${pageIds.length} 个页面已接线）`);
