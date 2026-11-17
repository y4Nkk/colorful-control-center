/**
 * 全局化边界检查（AGENTS.md / DESIGN.md 契约的可执行形式，build 前置）。
 *
 * 1. IPC 边界：`invoke(` 与 `@tauri-apps/` import 只允许出现在 src/api/
 * 2. 令牌所有权：`--ccc-*` 自定义属性定义只允许出现在 src/styles/tokens.css
 * 3. 页面接线一致：PAGES 注册的每个页面必须在 AppWorkspace 有对应分支
 * 4. 内容折叠：details/summary 只允许由全局 Collapsible 原语拥有
 * 5. 控件所有权：业务只能组合全局控件，不直接拼原生元素或覆盖控件样式
 *
 * Run: node scripts/check-boundaries.mjs
 */
import { readdirSync, readFileSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { inspectUiSource, inspectUiStyles } from "./check-ui-boundaries.mjs";

const rootDir = fileURLToPath(new URL("..", import.meta.url));
const srcDir = join(rootDir, "src");
const violations = [];
const aliases = new Map();
const styles = [];

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) yield* walk(full);
    else yield full;
  }
}

const rel = (path) => relative(rootDir, path).replaceAll("\\", "/");

/** 1. IPC 边界 + 2. 令牌所有权 + 4. 内容折叠所有权 */
for (const file of walk(srcDir)) {
  const path = rel(file);
  if (file.endsWith(".ts") || file.endsWith(".tsx")) {
    const content = readFileSync(file, "utf8");
    const ui = inspectUiSource(path, content);
    violations.push(...ui.violations);
    for (const [name, owner] of ui.aliases) aliases.set(name, owner);
    if (!path.startsWith("src/api/")) {
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
    styles.push([path, content]);
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
for (const [path, content] of styles) violations.push(...inspectUiStyles(path, content, aliases));

/** 3. 页面接线一致：PAGES ↔ AppWorkspace 分支 */
const navigation = readFileSync(join(srcDir, "app", "navigation.ts"), "utf8");
const appWorkspace = readFileSync(join(srcDir, "app", "AppWorkspace.tsx"), "utf8");
// 页面→工作区组件：显式 workspace 字段优先（控制台为合并页 ControlWorkspace），
// 否则按 PageId 约定推断 <Id>Workspace。
const pageMatches = [...navigation.matchAll(/id: "([a-z]+)"(?:,\s*workspace: "(\w+)")?/g)];
if (pageMatches.length === 0) violations.push("src/app/navigation.ts: PAGES 未找到任何页面注册");
for (const [, id, workspace] of pageMatches) {
  const component = workspace ?? `${id[0].toUpperCase()}${id.slice(1)}Workspace`;
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
console.log(`ok: 边界检查通过（${pageMatches.length} 个页面已接线）`);
