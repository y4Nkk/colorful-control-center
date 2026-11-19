import { Icon } from "../Icon";
import type { KeyboardLightingTarget, ProfileSettings } from "../../api/client";
import { hexOf } from "../../lib/color";
import { ColorSwatch } from "../ColorSwatch";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { FIELDS, stateText } from "../control/keyboard-values";
import { booleanText, timerText, topCoverText } from "../control/lighting-option-values";
import { EFFECTS } from "../control/useKeyboardLighting";
import { KeyboardBacklightVisual } from "../control/KeyboardBacklightVisual";

function KeyboardPicture({ target }: { target: KeyboardLightingTarget }) {
  const settings = target.settings, rgb = settings.family === "zoneRgb" ? settings : null;
  if (settings.family === "white") return <KeyboardBacklightVisual label="档案白光亮度" level={settings.brightness} enabled={target.enabled} maximum={5} showLevel />;
  const zoned = rgb && ["zones", "breath"].includes(rgb.effect), solid = rgb?.effect === "solid";
  return <div className="ccc-profile-values">
    <svg viewBox="0 0 330 58" role="img" aria-label={rgb ? "三区键盘灯区示意" : "白光键盘示意"}>
      {[0, 1, 2].map(zone => <g key={zone} opacity={target.enabled && (!zoned || rgb!.zones[zone].enabled) ? 1 : 0.25}>
        {Array.from({ length: 12 }, (_, key) => <rect key={key} x={zone * 112 + key % 4 * 26} y={Math.floor(key / 4) * 20}
          width="20" height="14" rx="3" fill={zoned ? hexOf(rgb!.zones[zone].color) : solid ? hexOf(rgb!.allColor) : "var(--ccc-text-muted)"} />)}
      </g>)}
    </svg>
    <span className="ccc-control-caption">{rgb && !zoned && !solid ? "动态色彩由固件生成" : "灯区配置示意"}</span>
  </div>;
}
export function ProfileKeyboardPreview({ settings }: { settings: ProfileSettings }) {
  const lighting = settings.keyboardLighting, options = settings.lightingOptions?.values;
  const effect = lighting?.settings.family === "zoneRgb" ? lighting.settings.effect : null;
  return <>
    {settings.keyboard && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案键盘功能">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="keyboard" />键盘功能</h3></div>
      <FactList compact facts={FIELDS.filter(field => settings.keyboard![field.key] !== null).map(field =>
        ({ label: field.title, value: stateText(settings.keyboard![field.key]) }))} />
    </Surface>}
    {lighting && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案键盘灯效">
      <div className="ccc-profile-values"><h3>{lighting.settings.family === "white" ? "白光键盘" : "三区 RGB 键盘"}</h3>
        <p className="ccc-control-caption">{booleanText(lighting.enabled)} · {lighting.enabled ? "亮度" : "保留亮度"} {lighting.settings.brightness} 级</p>
        {effect && <span className="ccc-control-caption">{EFFECTS.find(item => item.id === effect)?.label}</span>}
      </div><KeyboardPicture target={lighting} />
    </Surface>}
    {options && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案灯光策略与外围灯区">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="effects" />灯光策略与外围灯区</h3><p className="ccc-control-caption">{timerText(options.idleTimer)}</p>
        {options.bootOverride !== null && <span className="ccc-control-caption">开机灯效覆盖 · {booleanText(options.bootOverride)}</span>}</div>
      <div className="ccc-profile-values">{[{ title: "Logo", zone: options.logo }, { title: "普通灯条", zone: options.lightBar }].map(({ title, zone }) => zone &&
        <div className="ccc-profile-peripheral" key={title}><span className="ccc-control-caption">{title} · {booleanText(zone.enabled)}</span>
          <ColorSwatch color={zone.color} label={`${title}档案颜色`} dimmed={!zone.enabled} /></div>)}
        {options.topCover && <div className="ccc-profile-peripheral"><span className="ccc-control-caption">独立顶盖 · {topCoverText(options.topCover)}</span>
          <ColorSwatch color={options.topCover.color} label="独立顶盖档案颜色" dimmed={options.topCover.brightness === 0} /></div>}
        {!options.logo && !options.lightBar && !options.topCover && <span className="ccc-control-caption">此档案不配置外围灯区</span>}
      </div>
    </Surface>}
  </>;
}
