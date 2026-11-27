import { useState } from "react";
import { createRoot } from "react-dom/client";
import { Select } from "../../src/components/Select";
import { Button } from "../../src/components/Button";
import { Input } from "../../src/components/Input";
import { Checkbox } from "../../src/components/Checkbox";
import { Switch } from "../../src/components/Switch";
import { Slider } from "../../src/components/Slider";
import { Surface } from "../../src/components/Surface";
import { SegmentedControl } from "../../src/components/SegmentedControl";
import { ConfirmButton } from "../../src/components/ConfirmButton";
import { ColorPicker } from "../../src/components/ColorPicker";
import { TooltipProvider } from "../../src/components/Tooltip";
import { FactList, FactStatus } from "../../src/components/FactList";
import "../../src/styles/tokens.css";
import "../../src/styles/base.css";

function Fixture() {
  const [value, setValue] = useState("a"), [font, setFont] = useState("Consolas"), [checked, setChecked] = useState(false);
  const [range, setRange] = useState(30), [writes, setWrites] = useState(0), [disabled, setDisabled] = useState(false);
  const [options, setOptions] = useState([{ value: "a", label: "Alpha" }, { value: "b", label: "Bravo", disabled: true },
    { value: "c", label: "Charlie" }, { value: "d", label: "Delta" }]);
  const [loading, setLoading] = useState(false), [error, setError] = useState<string | null>(null);
  const [group, setGroup] = useState("one");
  const [color, setColor] = useState({ r: 127, g: 0, b: 203 });
  return <main aria-label="隔离控件" style={{ height: "100vh", overflow: "auto", padding: 16 }}>
    <Surface as="section" variant="panel" aria-label="控件表面" style={{ display: "grid", gap: 12, padding: 16 }}>
      <Button variant="secondary" onClick={() => document.documentElement.dataset.theme = document.documentElement.dataset.theme === "dark" ? "light" : "dark"}>切换主题</Button>
      <Button variant="secondary" onClick={() => setDisabled(value => !value)}>切换禁用</Button>
      <Button variant="secondary" onClick={() => setOptions([{ value: "x", label: "Xray" }, { value: "y", label: "Yankee" }])}>替换选项</Button>
      <Button variant="secondary" onClick={() => { setLoading(true); setError("隔离读取失败"); }}>模拟读取失败</Button>
      <Button variant="secondary" onClick={() => setOptions([])}>清空选项</Button>
      <Input aria-label="前一个字段" placeholder="输入文本" />
      <div id="anchor-area" style={{ display: "flex", maxWidth: "100%", overflow: "hidden", padding: 4 }}>
        <Select id="fixture-select" label="选择模式" value={value} options={options} disabled={disabled}
          loading={loading} error={error} onRetry={() => { setLoading(false); setError(null); }} onChange={setValue} />
      </div>
      <Input aria-label="后一个字段" />
      <output aria-label="已选模式">{value}</output>
      <Select label="选择字体" searchLabel="搜索字体" value={font} onChange={setFont}
        options={["Consolas", "Arial", "Monospace", ...Array.from({ length: 60 }, (_, i) => `Font ${i}`)].map(value => ({ value, label: value,
          description: "概览 Aa 0123", style: { fontFamily: value } }))} />
      <output aria-label="已选字体">{font}</output>
      <SegmentedControl label="测试单选" value={group} onChange={setGroup}
        options={[{ value: "one", label: "第一项" }, { value: "two", label: "第二项" },
          { value: "three", label: "不可选项", disabled: true, tooltip: "隔离不可用原因" }]} />
      <Checkbox label="测试复选" checked={checked} disabled={disabled} onChange={event => setChecked(event.target.checked)} />
      <Switch label="测试开关" checked={checked} disabled={disabled} onChange={setChecked} />
      <Slider aria-label="测试滑杆" min={0} max={100} value={range} disabled={disabled} onChange={setRange} />
      <output aria-label="滑杆数值">{range}</output>
      <ConfirmButton label="执行隔离操作" confirmLabel="确认执行隔离操作？" busy={disabled} onConfirm={() => setWrites(value => value + 1)} />
      <output aria-label="执行次数">{writes}</output>
      <div id="color-anchor"><ColorPicker label="测试颜色" value={color} onChange={setColor} disabled={disabled} swatchOnly /></div>
    </Surface>
    <div aria-hidden="true" style={{ height: 1000 }} />
  </main>;
}

function FactListFixture() {
  return <main style={{ padding: 16, minHeight: "100vh", background: "var(--ccc-workspace-bg)" }}>
    <Surface as="section" variant="card" style={{ padding: 16, display: "grid", gap: 16 }}>
      <h2 style={{ margin: 0 }}>设备诊断</h2>
      <FactList label="诊断字段" compact facts={[
        { label: "驱动报告的硬件设备识别码", icon: "gpu", format: "code", value: "PCI\\VEN_10DE&DEV_2705&SUBSYS_00000000&REV_A1" },
        { label: "连接模式", value: "独显直连" },
        { label: "当前功耗", icon: "power", format: "number", value: "96.4 W" },
        { label: "设备读取", icon: "check-circle", value: <FactStatus tone="success">已读取</FactStatus> },
        { label: "固件状态", icon: "warning", tone: "warning", value: <FactStatus tone="warning">未识别</FactStatus> },
        { label: "配置路径", icon: "logs", tone: "active", value: <a href="#capture">C:/ProgramData/ColorfulControlCenter/profiles/local-profile.json</a> },
      ]} />
      <FactList label="采样摘要" compact layout="inline" facts={[
        { label: "采样数", format: "number", value: "12" },
        { label: "状态", value: <FactStatus>已停止</FactStatus> },
      ]} />
      <h3 id="capture" style={{ margin: 0 }}>已读取的配置</h3>
      <FactList label="待应用参数" compact layout="tiles" facts={[
        { label: "模式", value: "性能" }, { label: "功耗墙", format: "number", value: "110 W" },
      ]} />
      <FactList label="档案参数差异" layout="comparison" facts={[
        { label: "内置屏幕", note: "显示试用需保留确认", current: "1920 × 1080 · 144 Hz", value: "1920 × 1080 · 60 Hz" },
        { label: "持续功耗上限", current: "未读取", value: "65 W" },
      ]} />
    </Surface>
  </main>;
}

const factList = new URLSearchParams(window.location.search).has("fact-list");
createRoot(document.getElementById("root")!).render(<TooltipProvider>{factList ? <FactListFixture /> : <Fixture />}</TooltipProvider>);
