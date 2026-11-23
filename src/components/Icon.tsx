/** 界面图标唯一边界；显式导入使未使用的库图标不进入产物。 */
import {
  Layers, Settings, ChevronsLeft, ChevronRight, ChevronDown, ChevronUp, Pin, Minus, Square, Copy, X,
  Check, CircleCheck, Info, TriangleAlert, CircleX, RotateCcw, RefreshCw, Save, Download,
  Upload, Plus, Trash2, Play, Pause, SquarePen, Search, ShieldCheck, LoaderCircle,
  Cpu, Gpu, Fan, HardDrive, Keyboard, MousePointer2, Mic, Monitor, Battery, BatteryCharging, Activity, MemoryStick,
  PlugZap, Usb, Network, Palette, Thermometer, Gauge, Zap, Power, Sparkles, Moon,
  SunMoon, PanelsTopLeft, ScrollText, Volume2, VolumeX, Plane, ArrowRight, LayoutDashboard,
  type LucideIcon,
} from "lucide-react";

const ICONS = {
  airplane: Plane,
  overview: LayoutDashboard,
  "arrow-right": ArrowRight,
  volume: Volume2, "volume-off": VolumeX,
  profiles: Layers, settings: Settings,
  collapse: ChevronsLeft, "chevron-right": ChevronRight, "chevron-down": ChevronDown, "chevron-up": ChevronUp,
  pin: Pin, minimize: Minus, maximize: Square, restore: Copy, close: X,
  check: Check, "check-circle": CircleCheck, info: Info, warning: TriangleAlert, error: CircleX,
  undo: RotateCcw, refresh: RefreshCw, save: Save, download: Download, upload: Upload,
  add: Plus, delete: Trash2, play: Play, pause: Pause, edit: SquarePen, search: Search,
  confirm: ShieldCheck, loading: LoaderCircle, apply: Check,
  cpu: Cpu, gpu: Gpu, fan: Fan, storage: HardDrive, keyboard: Keyboard, touchpad: MousePointer2,
  activity: Activity, memory: MemoryStick,
  microphone: Mic, display: Monitor, battery: Battery, charging: BatteryCharging,
  plug: PlugZap, usb: Usb, network: Network, lighting: Palette,
  temperature: Thermometer, performance: Gauge, power: Zap, shutdown: Power,
  effects: Sparkles, sleep: Moon,
  appearance: SunMoon, window: PanelsTopLeft, logs: ScrollText,
} satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof ICONS;
interface IconProps {
  name: IconName;
  size?: 16 | 20 | 24;
  className?: string;
  label?: string;
}

export function Icon({ name, size = 20, className = "", label }: IconProps) {
  const Glyph = ICONS[name];
  return <Glyph className={`ccc-icon ${className}`.trim()} data-icon={name} data-size={size}
    strokeWidth={1.7} aria-hidden={label ? undefined : true} role={label ? "img" : undefined}
    aria-label={label} focusable="false" />;
}
