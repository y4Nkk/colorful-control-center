import { Icon } from "../Icon";
import type { KeymapTarget } from "../../api/client";
import { actionDetails, actionLabel, KEYMAP_DEVICE_LABELS, keyLabel } from "../../lib/keymap";
import { Surface } from "../Surface";
import { FactList } from "../FactList";
import { Collapsible } from "../Collapsible";

export function ProfileKeymapPreview({ target }: { target: KeymapTarget }) {
  return <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案按键映射与宏">
    <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="keyboard" />按键映射与宏</h3>
      <FactList compact facts={target.configuration.devices.map(device => ({ label: KEYMAP_DEVICE_LABELS[device.device],
        value: `${device.enabled ? "启用" : "关闭"} · ${device.bindings.length} 项映射` }))} />
      <p className="ccc-control-caption">{target.programsEnabled ? "全部设置成功后启用程序宏监听，需保持应用运行" : "不启用程序宏监听，保留宏配置"}</p>
      <Collapsible summary="映射回读说明"><p>目标为保存配置及驱动接受结果，过滤器未提供映射表回读。</p></Collapsible>
    </div>
    <div className="ccc-profile-values">{target.configuration.devices.map(device => <div className="ccc-profile-values" key={device.device}>
      <svg viewBox="0 0 210 50" role="img" aria-label={`${KEYMAP_DEVICE_LABELS[device.device]}映射范围`}>
        {device.device === "keyboard" ? Array.from({ length: 24 }, (_, i) => <rect key={i} x={i % 8 * 26} y={Math.floor(i / 8) * 17}
          width="20" height="11" rx="3" fill={device.enabled ? "var(--ccc-action)" : "var(--ccc-text-muted)"} opacity={device.enabled ? 0.75 : 0.35} />)
          : <g fill="var(--ccc-content-muted)" stroke="var(--ccc-hairline)"><rect x="70" y="1" width="68" height="48" rx="20" />
            <path d="M104 2v20m-30 0h60" fill="none" /><rect x="100" y="6" width="8" height="12" rx="4" fill={device.enabled ? "var(--ccc-action)" : "var(--ccc-text-muted)"} /></g>}
      </svg>
      <Collapsible summary={`${KEYMAP_DEVICE_LABELS[device.device]} · 查看全部 ${device.bindings.length} 项映射`}>
        <div className="ccc-profile-values">{device.bindings.map(binding => <div className="ccc-profile-values" key={binding.trigger}>
          <strong className="ccc-control-caption">{keyLabel(device.device, binding.trigger)} · {actionLabel(binding.action)}</strong>
          <p className="ccc-keymap-preview-value ccc-mono">{actionDetails(binding.action)}</p>
        </div>)}{!device.bindings.length && <p className="ccc-control-caption">此设备使用原键功能</p>}</div>
      </Collapsible>
    </div>)}</div>
  </Surface>;
}
