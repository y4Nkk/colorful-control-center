import type { RgbColor } from "../api/client";

export type HsvColor = { h: number; s: number; v: number };

export function hexOf({ r, g, b }: RgbColor) {
  return `#${[r, g, b].map(channel => channel.toString(16).padStart(2, "0")).join("")}`;
}

export function parseHex(input: string): RgbColor | null {
  const hex = /^#?([\da-f]{6})$/i.exec(input.trim())?.[1];
  if (!hex) return null;
  const value = Number.parseInt(hex, 16);
  return { r: value >> 16, g: (value >> 8) & 255, b: value & 255 };
}

export function sameColor(a: RgbColor, b: RgbColor) {
  return a.r === b.r && a.g === b.g && a.b === b.b;
}

export function hsvOf({ r, g, b }: RgbColor, grayHue = 0): HsvColor {
  const high = Math.max(r, g, b);
  const chroma = high - Math.min(r, g, b);
  if (!chroma) return { h: grayHue, s: 0, v: high / 255 };
  const sector = high === r ? (g - b) / chroma : high === g ? 2 + (b - r) / chroma : 4 + (r - g) / chroma;
  return { h: (sector * 60 + 360) % 360, s: chroma / high, v: high / 255 };
}

export function rgbOfHsv({ h, s, v }: HsvColor): RgbColor {
  const sector = h / 60;
  const chroma = v * s;
  const middle = chroma * (1 - Math.abs(sector % 2 - 1));
  const offset = v - chroma;
  const [r, g, b] = [[chroma, middle, 0], [middle, chroma, 0], [0, chroma, middle],
    [0, middle, chroma], [middle, 0, chroma], [chroma, 0, middle]][Math.floor(sector)];
  return { r: Math.round((r + offset) * 255), g: Math.round((g + offset) * 255), b: Math.round((b + offset) * 255) };
}
