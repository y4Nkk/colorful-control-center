import assert from "node:assert/strict";
import { test } from "node:test";
import { inspectUiSource, inspectUiStyles } from "./check-ui-boundaries.mjs";

const source = text => inspectUiSource("src/components/control/Example.tsx", text);
const styles = (text, aliases) => inspectUiStyles("src/styles/example.css", text, aliases);

test("图标库只通过全局语义组件接入", () => {
  assert.equal(source('import { Fan } from "lucide-react";').violations.length, 1);
  assert.equal(source('import Fan from "lucide-react/icons/fan";').violations.length, 1);
  assert.deepEqual(inspectUiSource("src/components/Icon.tsx", 'import { Fan } from "lucide-react";').violations, []);
  assert.deepEqual(source('import { Icon } from "../Icon"; <Icon name="fan" />').violations, []);
});

for (const tag of ["button", "input", "select", "option", "textarea", "details", "summary"]) {
  test(`拒绝业务直接拼 ${tag}`, () => assert.equal(source(`<${tag} />`).violations.length, 1));
}
test("接受全局原语自己的原生结构", () => {
  assert.deepEqual(inspectUiSource("src/components/Button.tsx", "<button />").violations, []);
  assert.deepEqual(inspectUiSource("src/components/Checkbox.tsx", "<input />").violations, []);
  assert.deepEqual(inspectUiSource("src/components/Textarea.tsx", "<textarea className='ccc-field ccc-input' />").violations, []);
  assert.equal(inspectUiSource("src/components/Textarea.tsx", "<button />").violations.length, 1);
  assert.equal(inspectUiSource("src/components/Select.tsx", "<select />").violations.length, 1);
});
test("注释和字符串不是 JSX 原生入口", () => assert.deepEqual(source('const text = "<select>"; /* <button /> */').violations, []));
test("拒绝旧 Combobox 导入", () => assert.equal(source('import { Combobox } from "../Combobox"').violations.length, 1));
test("拒绝全局私有类和子部件在业务中直接拼装", () => {
  for (const name of ["ccc-surface", "ccc-select-option", "ccc-checkbox-box", "ccc-field"]) {
    assert.equal(source(`<div className="${name}"/>`).violations.length, 1);
    assert.equal(styles(`.${name} { border-radius: 0; }`).length, 1);
  }
});
test("别名控件与容器不能覆盖外观，允许业务布局及内容", () => {
  const result = source('import { Button as Action } from "../Button"; import { Surface } from "../Surface"; <><Action className="ccc-action"/><Surface className="ccc-panel"/></>');
  assert.deepEqual(result.violations, []);
  assert.equal(styles('.ccc-action { background: red; } .ccc-panel { border: 0; }', result.aliases).length, 2);
  assert.deepEqual(styles('.ccc-action { width: 100%; gap: 8px; } .ccc-panel { padding: 16px; font-size: 14px; }', result.aliases), []);
});
test("同一原语的多文件入口同样执行外观边界", () => {
  const result = source('import { Textarea } from "../Textarea"; <Textarea className="ccc-notes" style={{ color: "red" }}/>');
  assert.equal(result.violations.length, 1);
  assert.equal(styles('.ccc-notes { border: 0; }', result.aliases).length, 1);
  assert.deepEqual(styles('.ccc-notes { min-height: 120px; }', result.aliases), []);
});
test("选择器函数与嵌套规则不能绕过所有权", () => {
  for (const selector of [':is(.ccc-surface)', ':where(.ccc-select-option)', '.module { & .ccc-field']) {
    assert.ok(styles(`${selector} { color: red; }${selector.includes('{') ? '}' : ''}`).length > 0);
  }
});
test("状态查询与子内容样式不被误判为控件覆盖", () => {
  const aliases = source('import { Surface } from "../Surface"; <Surface className="ccc-panel"/>').aliases;
  assert.deepEqual(styles('.row:has(.ccc-select) { display: grid; } .ccc-panel h2 { color: red; }', aliases), []);
});
test("全局样式仅由对应样式文件拥有", () => {
  assert.deepEqual(inspectUiStyles('src/styles/select.css', '.ccc-select-option { color: red; }'), []);
  assert.equal(inspectUiStyles('src/styles/button.css', '.ccc-select-option { color: red; }').length, 1);
});
test("拒绝业务内联外观和不可检查 style，保留图形数据与布局", () => {
  assert.equal(source('import { Button } from "../Button"; <Button style={{ borderRadius: 0 }}/>').violations.length, 1);
  assert.equal(source('import { Surface } from "../Surface"; <Surface style={customStyle}/>').violations.length, 1);
  assert.deepEqual(source('import { Button } from "../Button"; <Button variant="swatch" style={{background: color, opacity: brightness}}/>').violations, []);
  assert.deepEqual(source('import { Button } from "../Button"; <Button variant="handle" style={{left: x, top: y}}/>').violations, []);
});
