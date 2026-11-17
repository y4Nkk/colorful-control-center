import { useId } from "react";
import source from "../../src-tauri/icons/icon.svg?raw";

const [, viewBox, drawing] = source.match(/<svg[^>]*viewBox="([^"]+)"[^>]*>([\s\S]*)<\/svg>/)!;

interface BrandLogoProps {
  className?: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  "data-tauri-drag-region"?: boolean;
}

/** 品牌绘图只来自 icon.svg；隔离同页预览与顶栏的滤镜、渐变和裁切引用。 */
export function BrandLogo(props: BrandLogoProps) {
  const prefix = `ccc-logo-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}-`;
  const html = drawing.replace(/id="([^"]+)"/g, `id="${prefix}$1"`)
    .replace(/#(ccc-[a-z-]+)/g, `#${prefix}$1`);
  return <svg {...props} viewBox={viewBox} aria-hidden="true" focusable="false"
    dangerouslySetInnerHTML={{ __html: html }} />;
}
