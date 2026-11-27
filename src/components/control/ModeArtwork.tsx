import quiet from "../../../src-tauri/icons/presets/quiet.svg?raw";
import eco from "../../../src-tauri/icons/presets/eco.svg?raw";
import performance from "../../../src-tauri/icons/presets/performance.svg?raw";
import entertainment from "../../../src-tauri/icons/presets/entertainment.svg?raw";

function compactArtwork(source: string) {
  const [, attributes, drawing] = source.match(/^<svg([^>]*)>([\s\S]*)<\/svg>/)!;
  const [, palette] = attributes.match(/\bdata-preset-palette="([^"]+)"/)!;
  const [, viewBox] = attributes.match(/\bviewBox="([^"]+)"/)!;
  return { palette, viewBox, drawing: { __html: drawing } };
}

const artworks = [quiet, eco, performance, entertainment].map(compactArtwork);

export function presetModeArtwork(mode: number | undefined) {
  return mode === undefined ? undefined : artworks[mode];
}

export function ModeArtwork({ mode }: { mode: number }) {
  const artwork = presetModeArtwork(mode);
  if (!artwork) return null;
  return <svg className="ccc-mode-art" data-preset-palette={artwork.palette}
    width="48" height="48" viewBox={artwork.viewBox} aria-hidden="true" focusable="false"
    dangerouslySetInnerHTML={artwork.drawing} />;
}
