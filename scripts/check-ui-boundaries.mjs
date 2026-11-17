import ts from "typescript";
import postcss from "postcss";
import selectorParser from "postcss-selector-parser";
import { UI_CONTRACT, VISUAL_PROPERTIES, classOwner, componentPath, primitiveFiles, stylePath } from "./ui-contract.mjs";

const owners = new Map(UI_CONTRACT.flatMap(owner => [owner.name, ...owner.files ?? []].map(name => [name, owner])));
const forbiddenNative = new Set(["button", "input", "select", "option", "textarea", "details", "summary"]);
const utilityClasses = new Set(["ccc-mono"]);
const visualJs = name => VISUAL_PROPERTIES.test(name.replace(/[A-Z]/g, value => `-${value.toLowerCase()}`));

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

function inspectInlineStyle(node, owner, tree, fail) {
  const attribute = node.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.text === "style");
  if (!attribute) return;
  const expression = attribute.initializer && ts.isJsxExpression(attribute.initializer) ? attribute.initializer.expression : null;
  if (!expression || !ts.isObjectLiteralExpression(expression)) { fail("业务控件 style 必须是可检查的布局对象"); return; }
  const swatch = owner.name === "Button" && node.attributes.properties.some(item => ts.isJsxAttribute(item)
    && item.name.text === "variant" && item.initializer && ts.isStringLiteral(item.initializer) && item.initializer.text === "swatch");
  for (const property of expression.properties) {
    if (!ts.isPropertyAssignment(property)) { fail("业务控件 style 不允许展开或间接覆盖外观"); continue; }
    const name = property.name.getText(tree).replaceAll(/["']/g, "");
    if (visualJs(name) && !(swatch && ["background", "opacity"].includes(name))) fail(`业务控件不得内联覆盖 ${name}`);
  }
}

export function inspectUiSource(path, content) {
  const tree = ts.createSourceFile(path, content, ts.ScriptTarget.Latest, true, path.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const names = importNames(tree), aliases = new Map(), violations = [];
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
      if (forbiddenNative.has(tag) && !UI_CONTRACT.some(owner => [owner.name, ...owner.files ?? []].some(file => componentPath(file) === path) && owner.native.includes(tag))) {
        fail(node, `<${tag}> 没有本地所有权，必须接入全局原语`);
      }
      const owner = owners.get(name);
      const classAttribute = node.attributes.properties.find(item => ts.isJsxAttribute(item) && item.name.text === "className");
      for (const className of getClasses(classAttribute, tree)) {
        if (owner && !utilityClasses.has(className) && !classOwner(className)) aliases.set(className, owner);
        if (!internal && classOwner(className)) fail(node, `${className} 是全局私有类，业务必须使用组件属性`);
      }
      if (owner && !internal) inspectInlineStyle(node, owner, tree, message => fail(node, message));
    }
    ts.forEachChild(node, visit);
  }
  visit(tree);
  return { aliases, violations };
}

function selectorTargets(selector) {
  const targets = [];
  const nodes = selector.nodes.slice(selector.nodes.findLastIndex(node => node.type === "combinator") + 1);
  for (const node of nodes) {
    if (node.type === "class" || node.type === "tag") targets.push(node);
    if (node.type === "pseudo" && [":is", ":where"].includes(node.value)) {
      for (const child of node.nodes ?? []) targets.push(...selectorTargets(child));
    }
  }
  return targets;
}

export function inspectUiStyles(path, content, aliases = new Map()) {
  const violations = [];
  const tree = postcss.parse(content, { from: path });
  tree.walkRules(rule => {
    const visual = rule.nodes.some(node => node.type === "decl" && VISUAL_PROPERTIES.test(node.prop));
    selectorParser(selectors => selectors.each(selector => {
      for (const target of selectorTargets(selector)) {
        const owner = target.type === "class" ? classOwner(target.value) : null;
        const alias = target.type === "class" ? aliases.get(target.value) : null;
        const native = target.type === "tag" && forbiddenNative.has(target.value);
        const aliasVisual = alias?.name === "Surface" ? rule.nodes.some(node => node.type === "decl"
          && /^(?:background|border|box-shadow|outline|opacity|cursor)(?:-|$)/.test(node.prop)) : visual;
        const nativeStyle = path === "src/styles/base.css" || UI_CONTRACT.some(item => stylePath(item) === path && item.native.includes(target.value));
        if ((owner && path !== stylePath(owner)) || (alias && aliasVisual && path !== stylePath(alias)) || (native && visual && !nativeStyle)) {
          violations.push(`${path}:${rule.source.start.line}: ${target.value} 外观归全局原语所有，业务只能定义布局`);
        }
      }
    })).processSync(rule.selector);
  });
  return [...new Set(violations)];
}
