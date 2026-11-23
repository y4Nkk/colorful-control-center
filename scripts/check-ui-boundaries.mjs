import ts from "typescript";
import postcss from "postcss";
import selectorParser from "postcss-selector-parser";
import { UI_CONTRACT, VISUAL_PROPERTIES, classOwner, componentPath, primitiveFiles, stylePath } from "./ui-contract.mjs";

const owners = new Map(UI_CONTRACT.flatMap(owner => [owner.name, ...owner.files ?? []].map(name => [name, owner])));
const forbiddenNative = new Set(["button", "input", "select", "option", "textarea", "details", "summary", "dialog"]);
const utilityClasses = new Set(["ccc-mono"]);
const visualJs = name => VISUAL_PROPERTIES.test(name.replace(/[A-Z]/g, value => `-${value.toLowerCase()}`));
const moduleLayoutFile = "src/styles/module-layout.css";
const layoutContainers = new Set(["ccc-page", "ccc-control", "ccc-module-grid"]);
const placementProperties = [
  /^(?:min-|max-)?(?:width|inline-size)$/,
  /^margin(?:-(?:inline(?:-(?:start|end))?|left|right))?$/,
  /^(?:align|justify|place)-self$/,
  /^grid-(?:area|(?:row|column)(?:-(?:start|end))?)$/,
  /^flex(?:-(?:grow|shrink|basis))?$/,
];

function attribute(node, name) {
  return node.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.text === name);
}

function literalAttribute(node, name) {
  const initializer = attribute(node, name)?.initializer;
  const value = initializer && ts.isJsxExpression(initializer) ? initializer.expression : initializer;
  return value && ts.isStringLiteralLike(value) ? value.text : undefined;
}

function getClasses(attribute, tree) {
  return attribute?.initializer?.getText(tree).match(/ccc-[\w-]+/g) ?? [];
}

function importNames(tree) {
  const names = new Map();
  for (const statement of tree.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier)) continue;
    if (!statement.moduleSpecifier.text.includes("components/") && !statement.moduleSpecifier.text.startsWith(".")) continue;
    const bindings = statement.importClause?.namedBindings;
    if (bindings && ts.isNamedImports(bindings)) {
      for (const item of bindings.elements) names.set(item.name.text, item.propertyName?.text ?? item.name.text);
    }
  }
  return names;
}

function inspectInlineStyle(node, tree, fail) {
  const style = attribute(node, "style");
  if (!style) return;
  const expression = style.initializer && ts.isJsxExpression(style.initializer) ? style.initializer.expression : null;
  if (!expression || !ts.isObjectLiteralExpression(expression)) { fail("业务控件 style 必须是可检查的布局对象"); return; }
  for (const property of expression.properties) {
    if (!ts.isPropertyAssignment(property)) { fail("业务控件 style 不允许展开或间接覆盖外观"); continue; }
    const name = property.name.getText(tree).replaceAll(/["']/g, "");
    if (visualJs(name)) fail(`业务控件不得内联覆盖 ${name}`);
  }
}

function isModuleRoot(node, owner, classes) {
  if (classes.some(name => name === "ccc-panel" || layoutContainers.has(name))) return true;
  if (owner?.name !== "Surface") return false;
  const variant = literalAttribute(node, "variant");
  const namedSection = literalAttribute(node, "as") === "section"
    && (attribute(node, "aria-label") || attribute(node, "aria-labelledby"));
  return (variant === "group" || variant === "card")
    && (namedSection || literalAttribute(node, "role") === "region");
}

export function inspectUiSource(path, content) {
  const tree = ts.createSourceFile(path, content, ts.ScriptTarget.Latest, true, path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const names = importNames(tree), aliases = new Map(), moduleClasses = new Set(), violations = [];
  const internal = primitiveFiles.has(path);
  const fail = (node, message) => violations.push(`${path}:${tree.getLineAndCharacterOfPosition(node.getStart(tree)).line + 1}: ${message}`);
  function visit(node) {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)
      && node.moduleSpecifier.text.startsWith("lucide-react") && path !== "src/components/Icon.tsx") {
      fail(node, "图标库只允许由全局 Icon 组件接入");
    }
    if (ts.isImportDeclaration(node) && node.moduleSpecifier.getText(tree).includes("/Combobox")) fail(node, "Combobox 旧入口已移除，统一使用 Select");
    if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
      const tag = node.tagName.getText(tree), name = names.get(tag) ?? tag;
      const role = literalAttribute(node, "role");
      const roleOwner = UI_CONTRACT.find(owner => owner.roles?.includes(role));
      if (roleOwner && componentPath(roleOwner.name) !== path) fail(node, `role="${role}" 必须由全局 ${roleOwner.name} 组件提供`);
      if ((tag === "input" || name === "Input") && literalAttribute(node, "type") === "color") {
        fail(node, "颜色编辑必须使用全局 ColorPicker");
      } else if (forbiddenNative.has(tag) && !UI_CONTRACT.some(owner => [owner.name, ...owner.files ?? []].some(file => componentPath(file) === path) && owner.native.includes(tag))) {
        fail(node, `<${tag}> 没有本地所有权，必须接入全局原语`);
      }
      const owner = owners.get(name);
      const classes = getClasses(attribute(node, "className"), tree);
      const moduleRoot = isModuleRoot(node, owner, classes);
      for (const className of classes) {
        if (owner && !utilityClasses.has(className) && !classOwner(className)) aliases.set(className, owner);
        if (!internal && classOwner(className)) fail(node, `${className} 是全局私有类，业务必须使用组件属性`);
      }
      if (moduleRoot) {
        const literal = literalAttribute(node, "className");
        if (attribute(node, "className") && literal === undefined) fail(node, "模块根节点必须使用静态类名");
        const staticClasses = literal?.match(/\S+/g) ?? [];
        for (const name of staticClasses) moduleClasses.add(name);
        if (staticClasses.length > 1 && staticClasses.some(name => layoutContainers.has(name))) {
          fail(node, "模块网格通过 data-module-layout 选择分栏，根节点只使用布局类");
        }
        if (attribute(node, "style") || node.attributes.properties.some(ts.isJsxSpreadAttribute)) {
          fail(node, "模块根节点通过静态类声明样式，轨道由父级网格分配");
        }
      } else if (owner && !internal) inspectInlineStyle(node, tree, message => fail(node, message));
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return { aliases, moduleClasses, violations };
}

function selectorTargets(selector, parentTargets) {
  const targets = [];
  const nodes = selector.nodes.slice(selector.nodes.findLastIndex(node => node.type === "combinator") + 1);
  for (const node of nodes) {
    if (node.type === "class" || node.type === "tag") targets.push(node);
    if (node.type === "nesting") targets.push(...parentTargets);
    if (node.type === "pseudo" && [":is", ":where"].includes(node.value)) {
      for (const child of node.nodes ?? []) targets.push(...selectorTargets(child, parentTargets));
    }
  }
  return targets;
}

function ruleTargets(rule) {
  let parent = rule.parent;
  while (parent && parent.type !== "rule") parent = parent.parent;
  const parentTargets = parent ? ruleTargets(parent) : [];
  return selectorParser().astSync(rule.selector).nodes.flatMap(selector => selectorTargets(selector, parentTargets));
}

export function inspectUiStyles(path, content, { aliases = new Map(), moduleClasses = new Set() } = {}) {
  const violations = [];
  const tree = postcss.parse(content, { from: path });
  tree.walkRules(rule => {
    const declarations = rule.nodes.filter(node => node.type === "decl");
    const visual = declarations.some(node => VISUAL_PROPERTIES.test(node.prop));
    const placement = declarations.find(({ prop, value }) => placementProperties.some(pattern => pattern.test(prop))
      && !((prop === "min-width" || prop === "min-inline-size") && value === "0"));
    for (const target of ruleTargets(rule)) {
      const isClass = target.type === "class";
      const owner = isClass ? classOwner(target.value) : null;
      const alias = isClass ? aliases.get(target.value) : null;
      const native = target.type === "tag" && forbiddenNative.has(target.value);
      const aliasVisual = alias?.name === "Surface" ? declarations.some(node =>
        /^(?:background|border|box-shadow|outline|opacity|cursor)(?:-|$)/.test(node.prop)) : visual;
      const nativeStyle = path === "src/styles/base.css" || UI_CONTRACT.some(item => stylePath(item) === path && item.native.includes(target.value));
      if ((owner && path !== stylePath(owner)) || (alias && aliasVisual && path !== stylePath(alias)) || (native && visual && !nativeStyle)) {
        violations.push(`${path}:${rule.source.start.line}: ${target.value} 外观归全局原语所有，业务只能定义布局`);
      }
      if (isClass && layoutContainers.has(target.value)) {
        if (declarations.length && path !== moduleLayoutFile) violations.push(`${path}:${rule.source.start.line}: ${target.value} 的样式由 module-layout.css 管理`);
      } else if (placement && isClass && moduleClasses.has(target.value)) {
        violations.push(`${path}:${placement.source.start.line}: ${target.value} 的 ${placement.prop} 由父级模块网格分配`);
      }
    }
  });
  return [...new Set(violations)];
}
