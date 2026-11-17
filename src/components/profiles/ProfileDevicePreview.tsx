import { Icon } from "../Icon";
import type { ProfileSettings } from "../../api/client";
import { Surface } from "../Surface";
import { LightBarPreview } from "../control/LightBarEditor";
import { lightBarSummary } from "../control/lightbar-values";
import { DUST_DAYS, formatDustSchedule } from "../control/DustScheduleEditor";
import { ProfileKeyboardPreview } from "./ProfileKeyboardPreview";
import { ProfilePerkeyPreview } from "./ProfilePerkeyPreview";
import { ProfileKeymapPreview } from "./ProfileKeymapPreview";
import { ProfilePlatformPreview } from "./ProfilePlatformPreview";
import { ProfileBatteryPreview } from "./ProfileBatteryPreview";
import { ProfilePerformancePreview } from "./ProfilePerformancePreview";
import { ProfileLanPreview } from "./ProfileLanPreview";

export function ProfileDevicePreview({ settings }: { settings: ProfileSettings }) {
  const schedule = settings.dustSchedule;
  return <>
    <ProfilePerformancePreview settings={settings} />
    <ProfileBatteryPreview settings={settings} />
    <ProfilePlatformPreview settings={settings} />
    <ProfileLanPreview settings={settings} />
    <ProfileKeyboardPreview settings={settings} />
    {settings.perkeyLighting && <ProfilePerkeyPreview target={settings.perkeyLighting} />}
    {settings.keymap && <ProfileKeymapPreview target={settings.keymap} />}
    {settings.lightBar && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案独立灯条">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="lighting" />独立灯条</h3><p className="ccc-control-caption">{lightBarSummary(settings.lightBar)}</p>
      </div>
      <LightBarPreview draft={settings.lightBar} />
    </Surface>}
    {schedule && <Surface as="section" variant="group" className="ccc-profile-device" aria-label="档案除尘计划">
      <div className="ccc-profile-values"><h3 className="ccc-icon-title"><Icon name="fan" />除尘计划</h3><p className="ccc-control-caption">{formatDustSchedule(schedule)}</p>
        <span className="ccc-control-caption">仅配置计划，不立即启动除尘</span></div>
      <div className="ccc-profile-values">
        <div className="ccc-profile-weekdays" role="list" aria-label="除尘计划星期">{DUST_DAYS.map(day =>
          <span key={day.bit} role="listitem" className="ccc-profile-day" data-selected={!!(schedule.weekdays & day.bit)}
            aria-label={`${day.label} ${schedule.weekdays & day.bit ? "执行" : "不执行"}`}>{day.label.slice(1)}</span>)}</div>
        <span className="ccc-control-caption">{schedule.onResume ? "系统唤醒后执行" : "唤醒触发关闭"}</span>
      </div>
    </Surface>}
  </>;
}
