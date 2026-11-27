import postcss from "postcss";

const visualColorTokens = /^--ccc-(?:hardware-art-|display-art-|mode-art-|preset-(?:light|mid|low|deep)$|chart-(?:track|blue)$|administrator-elevated$|fact-(?:success|warning)$|action-fill$)/;

function paletteSection(reference, heading, next) {
  const section = reference.split(`\n${heading}:\n`)[1]?.split(`\n${next}:`)[0];
  if (!section) throw new Error(`Apple DESIGN.md 缺少 ${heading} 色阶或尺度`);
  return section;
}

function colorsIn(value) {
  const hexes = [...value.matchAll(/#[\da-f]{3,6}\b/gi)].map(([hex]) => hex.length === 4
    ? "#" + [...hex.slice(1).toLowerCase()].map(digit => digit.repeat(2)).join("") : hex.toLowerCase());
  for (const [, red, green, blue] of value.matchAll(/rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)/g)) {
    hexes.push("#" + [red, green, blue].map(channel => Number(channel).toString(16).padStart(2, "0")).join(""));
  }
  return hexes;
}

export function inspectAppleDesign(reference, styles) {
  const document = reference.replaceAll("\r\n", "\n");
  const colors = new Set(colorsIn(paletteSection(document, "colors", "typography")));
  const radii = new Set([...paletteSection(document, "rounded", "spacing").matchAll(/:\s*(\d+px)\b/g)].map(match => match[1]));
  const sizes = new Set([...paletteSection(document, "typography", "rounded").matchAll(/fontSize:\s*(\d+px)\b/g)].map(match => match[1]));
  const violations = [];

  for (const [path, content] of styles) {
    postcss.parse(content, { from: path }).walkDecls(rule => {
      const { prop, value } = rule;
      const fail = message => violations.push(`${path}:${rule.source.start.line}: ${message}`);
      if (!visualColorTokens.test(prop)) {
        for (const color of colorsIn(value)) if (!colors.has(color)) fail(`${color} 未定义于 Apple 设计文档`);
      }
      if (/^--ccc-radius-/.test(prop) && !radii.has(value)) fail(`圆角 ${value} 未定义于 Apple 设计文档`);
      if (prop === "border-radius" && !["0", "inherit"].includes(value) && !/^var\(--ccc-radius-[\w-]+\)$/.test(value)) {
        fail(`圆角须引用共享尺度（${value}）`);
      }
      if (/^(?:font-size|--ccc-text-(?:xs|sm|md|lg|xl|title|reading|temperature|mode))$/.test(prop) && /^\d+px$/.test(value) && !sizes.has(value)) {
        fail(`字号 ${value} 未定义于 Apple 设计文档`);
      }
      if (/^(?:box|text)-shadow$/.test(prop) && value !== "none") fail("界面层次使用色面和边框");
      if (/^background(?:-image)?$/.test(prop) && /gradient\(/.test(value)) fail("渐变由自绘 SVG 表达");
      if (/^(?:border(?:-width|-top|-bottom|-left|-right|-inline-start|-inline-end|-block-start|-block-end)?|outline)$/.test(prop)) {
        for (const [, width] of value.matchAll(/(?:^|\s)([\d.]+)px\b/g)) {
          if (![0, 1, 2, 3].includes(Number(width))) fail(`边界宽度超出文档尺度（${width}px）`);
        }
      }
    });
  }
  return violations;
}
