/**
 * 决策笔记结构与格式校验（AGENTS.md「决策笔记」契约的可执行形式）。
 *
 * 结构：{lifecycle}/{class}/yyyy-mm-dd-topic.md
 *   lifecycle ∈ proposed | implemented | rejected（archived/ 不校验格式）
 *   class     ∈ feature | bug-fix | simplification | architecture | process | testing
 * 格式：第 1 行 `# Agent Note: <标题>`；第 3 行 Status；首个小节 `## 问题`；
 *   按生命周期要求的小节 + `## 备选方案`。
 *
 * Run: node scripts/verify-notes.mjs
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const agentNoteRoot = process.env.AGENT_NOTE_ROOT
  ? resolve(process.env.AGENT_NOTE_ROOT)
  : resolve(process.cwd(), ".agents/notes");

const LIFECYCLES = ["proposed", "implemented", "rejected"];
const CLASSES = ["feature", "bug-fix", "simplification", "architecture", "process", "testing"];

const STATUS = {
  proposed: /^Status: proposed$|^状态[:：] ?(?:proposed|已提议)$/,
  implemented: /^Status: implemented$|^状态[:：] ?(?:implemented|已实现)$/,
  rejected: /^Status: rejected — .+$|^状态[:：] ?(?:rejected|已否决) — .+$/,
};

const REQUIRED = {
  proposed: [
    ["## Proposal", "## 提议", "## 方案", "## 提案"],
    ["## Acceptance criteria", "## 验收标准", "## 验收条件", "## 接受标准"],
    ["## Risks", "## 风险"],
  ],
  implemented: [
    ["## Decision", "## 决定", "## 决策"],
    ["## Consequences", "## 后果", "## 影响", "## 结果", "## 结果与代价", "## 影响与验证"],
  ],
  rejected: [["## Proposal", "## 提议", "## 方案", "## 提案"]],
};

const BANNED_IMPLEMENTED = new Set([
  "## proposal", "## plan", "## migration plan", "## acceptance criteria",
  "## 提议", "## 方案", "## 提案",
  "## 计划", "## 规划", "## 迁移计划",
  "## 验收标准", "## 验收条件", "## 接受标准",
]);

const PROBLEM_FIRST = ["## Problem", "## 问题"];
const ALTERNATIVES_RE = /^## (?:Alternatives considered|.{0,8}?(?:替代方案|备选(?:方案)?))$/;

function listMd(dir, prefix) {
  if (!existsSync(dir)) return [];
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith(".")) continue;
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...listMd(resolve(dir, entry.name), rel));
    else if (entry.isFile() && entry.name.endsWith(".md")) out.push(rel);
  }
  return out;
}

const notes = [];
const errors = [];

if (existsSync(agentNoteRoot)) {
  for (const entry of readdirSync(agentNoteRoot, { withFileTypes: true })) {
    if (entry.name === "INDEX.md") {
      errors.push("structure: INDEX.md — 禁止集中索引；浏览生命周期/分类目录即可");
      continue;
    }
    if (entry.isDirectory() && !["proposed", "implemented", "rejected", "archived"].includes(entry.name)) {
      errors.push(`structure: ${entry.name}/ — 未知生命周期目录（允许 proposed/implemented/rejected + archived/）`);
    }
  }
  for (const lifecycle of LIFECYCLES) {
    for (const relPath of listMd(resolve(agentNoteRoot, lifecycle), lifecycle).sort()) {
      const segments = relPath.split("/");
      if (segments.length !== 3) {
        errors.push(`structure: ${relPath} — 期望 {lifecycle}/{class}/yyyy-mm-dd-topic.md`);
        continue;
      }
      const [, cls, base] = segments;
      if (!CLASSES.includes(cls)) {
        errors.push(`structure: ${relPath} — 未知分类目录 "${cls}"（允许 ${CLASSES.join(", ")}）`);
        continue;
      }
      if (!/^\d{4}-\d{2}-\d{2}-.+\.md$/.test(base)) {
        errors.push(`structure: ${relPath} — 文件名必须是 yyyy-mm-dd-topic.md`);
        continue;
      }
      notes.push({ lifecycle, rel: relPath });
    }
  }
}

/** 归一化 BOM/CRLF；代码块与 HTML 注释内的行不参与格式判定。 */
function maskSource(raw) {
  const lines = raw.replace(/^\uFEFF/, "").replace(/\r\n?/g, "\n").split("\n");
  const fenced = new Array(lines.length).fill(false);
  let inFence = false;
  for (let i = 0; i < lines.length; i++) {
    if (/^\s{0,3}```/.test(lines[i])) inFence = !inFence;
    fenced[i] = inFence;
  }
  const stripped = lines.map((line, i) => (fenced[i] ? "```" : line.replace(/<!--.*?-->/g, "")));
  return stripped.join("\n").replace(/<!--[\s\S]*?-->/g, "").split("\n");
}

/** 剥掉标题尾注：`## 决定（说明）` → `## 决定`。 */
function headingBase(heading) {
  return heading.replace(/[（(].*$/, "").trimEnd();
}

for (const note of notes) {
  const fail = (message) => errors.push(`format: ${note.rel} — ${message}`);
  const lines = maskSource(readFileSync(resolve(agentNoteRoot, note.rel), "utf8"));

  if (!/^# Agent Note[:：] ?\S/.test(lines[0] ?? "")) fail("第 1 行必须是 `# Agent Note: <标题>`");
  if (lines[1] !== "") fail("第 2 行必须空行");

  const statusRe = STATUS[note.lifecycle];
  if (statusRe && !statusRe.test(lines[2] ?? "")) fail(`第 3 行必须是 ${note.lifecycle} 的 Status 行`);
  if (lines[3] !== "") fail("第 4 行必须空行");

  const statusCount = lines.filter((line) => Object.values(STATUS).some((re) => re.test(line.trim()))).length;
  if (statusCount !== 1) fail("Status 行必须恰好出现一次");

  const headings = lines.filter((line) => line.startsWith("## ")).map((line) => headingBase(line.trimEnd()));
  if (!PROBLEM_FIRST.includes(headings[0] ?? "")) {
    fail(`首个小节必须是 ## 问题 或 ## Problem（当前 ${JSON.stringify(headings[0] ?? "<无>")})`);
  }

  for (const group of REQUIRED[note.lifecycle] ?? []) {
    if (!group.some((heading) => headings.includes(heading))) {
      fail(`缺少必需小节之一：${JSON.stringify(group)}`);
    }
  }

  if (note.lifecycle === "implemented") {
    for (const heading of headings.filter((h) => BANNED_IMPLEMENTED.has(h.toLowerCase()))) {
      fail(`implemented 笔记禁止出现提案期小节：${heading}`);
    }
  }

  if (!headings.some((heading) => ALTERNATIVES_RE.test(heading))) {
    fail("缺少 ## 备选方案 / ## Alternatives considered");
  }
}

if (errors.length) {
  for (const error of errors) console.error(error);
  process.exit(1);
}
console.log(`ok: ${notes.length} 篇决策笔记校验通过`);
