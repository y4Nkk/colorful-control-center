export const UI_CONTRACT = [
  { name: "Button", style: "button", prefixes: ["button"], native: ["button"] },
  { name: "Input", style: "field", prefixes: ["input", "field"], files: ["Textarea"], native: ["input", "textarea"] },
  { name: "ColorPicker", style: "color-picker", prefixes: ["color-picker", "color-swatch"], files: ["ColorSwatch", "ColorPickerEditor"], native: [] },
  { name: "Select", style: "select", prefixes: ["select"], files: ["SelectMenu"], native: [] },
  { name: "Checkbox", style: "choice", prefixes: ["checkbox"], native: ["input"] },
  { name: "Radio", style: "choice", prefixes: ["radio"], native: ["input"] },
  { name: "SegmentedControl", style: "choice", prefixes: ["segmented"], native: [] },
  { name: "Tabs", style: "tabs", prefixes: ["tabs"], native: ["button"], roles: ["tablist", "tab", "tabpanel"] },
  { name: "Slider", style: "slider", prefixes: ["range"], native: ["input"] },
  { name: "Surface", style: "surface", prefixes: ["surface"], native: [] },
  { name: "Switch", style: "switch", prefixes: ["switch"], native: ["input"] },
  { name: "Dialog", style: "dialog", prefixes: ["dialog"], native: ["dialog"] },
  { name: "ScrollArea", style: "scroll-area", prefixes: ["scroll-area", "scroll-viewport"], native: [] },
  { name: "BackToTopButton", style: "back-to-top", prefixes: ["back-to-top"], native: [] },
];

export const VISUAL_PROPERTIES = /^(?:background(?:-|$)|border(?:-|$)|color$|box-shadow$|outline(?:-|$)|font(?:-|$)|opacity$|cursor$|appearance$)/;
export const componentPath = name => `src/components/${name}.tsx`;
export const stylePath = owner => `src/styles/${owner.style}.css`;
export const primitiveFiles = new Set(UI_CONTRACT.flatMap(owner => [owner.name, ...owner.files ?? []].map(componentPath)));
export function classOwner(name) {
  return UI_CONTRACT.find(owner => owner.prefixes.some(prefix => name === `ccc-${prefix}` || name.startsWith(`ccc-${prefix}-`)));
}
