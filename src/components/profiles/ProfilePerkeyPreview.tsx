import type { PerkeyTarget, RgbColor } from "../../api/client";
import { perkeyLayout } from "../../lib/perkey-layout";
import { PERKEY_NAMES, SNAKE_DIRECTIONS, WAVE_DIRECTIONS } from "../../lib/perkey-pattern";
import { hexOf } from "../control/useKeyboardLighting";
import { Surface } from "../Surface";
import { FactList } from "../FactList";

function PerkeyPicture({ target }: { target: PerkeyTarget }) {
  const layout = perkeyLayout(target.device), pattern = target.values.pattern;
  const dynamicColors = "color" in pattern ? [pattern.color] : pattern.kind === "scan" ? pattern.colors : null;
  const color = (id: number): RgbColor | null => pattern.kind === "static" ? pattern.colors[id] : dynamicColors?.[id % dynamicColors.length] ?? null;
  const keys = layout?.keys ?? Array.from({ length: 120 }, (_, id) => ({ id: String(id), label: `灯位 ${Math.floor(id / 20) + 1}-${id % 20 + 1}`,
    legend: "", x: id % 20 * 32, y: Math.floor(id / 20) * 28, width: 27, height: 22, leds: [id] }));
  return <div className="ccc-profile-values">
    <svg viewBox={`0 0 ${layout?.width ?? 640} ${layout?.height ?? 168}`} role="img" aria-label={layout ? "档案逐键键帽配置" : "档案逐键灯位矩阵"}>
      <g>{keys.map(key => <g key={key.id}>
        <title>{key.label} · {key.leds.length} 个灯位</title>
        <rect x={key.x} y={key.y} width={key.width} height={key.height} rx="4" fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)" strokeWidth="2" />
        {key.leds.map((id, index) => <rect key={id} x={key.x + index * key.width / key.leds.length} y={key.y + key.height * 0.7}
          width={key.width / key.leds.length} height={key.height * 0.3} rx="2" opacity={target.values.enabled ? 1 : 0.3} fill={color(id) ? hexOf(color(id)!) : "var(--ccc-text-muted)"} />)}
        {key.legend && <text x={key.x + key.width / 2} y={key.y + key.height * 0.45} textAnchor="middle" dominantBaseline="middle"
          fontSize={Math.min(20, key.width * 0.35)} fill="var(--ccc-text)">{key.legend}</text>}
      </g>)}</g>
    </svg>
    <span className="ccc-control-caption">{layout ? "键帽与灯位配置示意" : "布局未识别，显示 6 × 20 灯位矩阵"}{pattern.kind !== "static" && " · 动态效果仅示意配置色彩"}</span>
  </div>;
}

export function ProfilePerkeyPreview({ target }: { target: PerkeyTarget }) {
  const { values } = target, pattern = values.pattern;
  const reactive = ["ripple", "fn", "fade"].includes(pattern.kind);
  const direction = pattern.kind === "wave" ? WAVE_DIRECTIONS[pattern.direction] : pattern.kind === "snake" ? SNAKE_DIRECTIONS[pattern.direction] : null;
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案逐键灯效">
    <div className="ccc-profile-values"><h3>逐键 RGB · {PERKEY_NAMES[pattern.kind]}</h3>
      <FactList compact facts={[
        { label: "灯光", value: values.enabled ? "开启" : "关闭" },
        { label: values.enabled ? "亮度" : "保留亮度", value: `${values.brightness} 级` },
        ...(!["static", "fn"].includes(pattern.kind) ? [{ label: "速度", value: `${values.speed + 1} 档` }] : []),
        ...(direction ? [{ label: "方向", value: direction }] : []),
        ...(pattern.kind === "ripple" ? [{ label: "响应", value: pattern.reactive ? "单键" : "扩散" }] : []),
      ]} />
      {reactive && <p className="ccc-control-caption">{values.enabled ? "全部设置应用成功后启用按键响应" : "灯光关闭，不启用按键响应"}</p>}
    </div><PerkeyPicture target={target} />
  </Surface>;
}
