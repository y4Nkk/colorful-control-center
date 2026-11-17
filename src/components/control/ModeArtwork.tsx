const IMAGES = ["quiet", "eco", "performance", "entertainment", "quiet"] as const;

export function ModeArtwork({ mode }: { mode: number }) {
  const image = IMAGES[mode];
  if (!image) return null;
  return <img className="ccc-mode-art" data-mode-art={mode} src={`/mode-artwork/${image}.png`}
    width={1586} height={992} alt="" aria-hidden="true" draggable={false} decoding="async" />;
}
