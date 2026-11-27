import { useId, useLayoutEffect, useMemo, useRef, useState, type Ref } from "react";
import { presetModeArtwork } from "./ModeArtwork";
import source from "./preset-illustrations.svg?raw";

const [, viewBox, definitions] = source.match(/<svg[^>]*viewBox="([^"]+)"[^>]*>\s*(<defs>[\s\S]*?<\/defs>)/)!;
const illustrations = Object.fromEntries(
  [...source.matchAll(/<symbol id="([^"]+)"[^>]*>([\s\S]*?)<\/symbol>/g)].map(([, name, drawing]) => [name, drawing]),
);
type Scene = { palette: string; active: boolean; outgoing: string | null };

function ArtworkLayer({ id, palette, drawing, outgoing }: {
  id: string; palette: string; drawing?: Ref<SVGGElement>; outgoing: boolean;
}) {
  const html = useMemo(() => ({
    // 同页的概览、配置页和退场图层分别持有自己的渐变、裁切和局部引用。
    __html: (definitions + illustrations[palette])
      .replace(/(\bid="|\bhref="#|\burl\(#)([\w-]+)/g, "$1" + id + "-$2"),
  }), [id, palette]);
  return <g className="ccc-preset-layer" data-preset-palette={palette}
    data-outgoing={outgoing || undefined}>
    <g className="ccc-preset-drawing" ref={drawing} dangerouslySetInnerHTML={html} />
  </g>;
}

export function PresetBackdrop({ mode }: { mode?: number }) {
  const id = "ccc-preset-backdrop-" + useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const palette = presetModeArtwork(mode)?.palette;
  if (!palette) return null;
  const colorId = id + "-color";
  const fadeId = id + "-fade";
  return <svg className="ccc-preset-backdrop" data-preset-palette={palette}
    viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true" focusable="false">
    <defs>
      <linearGradient id={colorId} x1="0%" y1="0%" x2="100%" y2="0%">
        <stop stopColor="var(--ccc-preset-mid)" stopOpacity=".22" />
        <stop offset="100%" stopColor="var(--ccc-preset-low)" stopOpacity=".29" />
      </linearGradient>
      <linearGradient id={fadeId} x1="0%" y1="0%" x2="0%" y2="100%">
        <stop offset="18%" stopColor="var(--ccc-content)" />
        <stop offset="100%" stopColor="var(--ccc-content)" stopOpacity="0" />
      </linearGradient>
    </defs>
    <rect width="100" height="100" fill={`url(#${colorId})`} />
    <rect width="100" height="100" fill={`url(#${fadeId})`} />
  </svg>;
}

export function PresetArtwork({ mode, active }: { mode?: number; active: boolean }) {
  const id = "ccc-preset-" + useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const drawing = useRef<SVGGElement>(null);
  const palette = presetModeArtwork(mode)?.palette ?? "neutral";
  const [scene, setScene] = useState<Scene>({ palette, active, outgoing: null });
  if (scene.palette !== palette || scene.active !== active) {
    const switching = active && scene.active && scene.palette !== "neutral" && palette !== "neutral";
    setScene({ palette, active, outgoing: switching ? scene.palette : null });
  }

  useLayoutEffect(() => {
    if (!scene.outgoing) return;
    const settle = () => setScene(current => current === scene ? { ...current, outgoing: null } : current);
    const reveal = drawing.current?.getAnimations()[0];
    if (!reveal) {
      settle();
      return;
    }
    let listening = true;
    const finish = () => { if (listening) settle(); };
    void reveal.finished.then(finish, finish);
    return () => { listening = false; };
  }, [scene]);

  return <svg className="ccc-preset-artwork" data-preset-palette={palette}
    data-active={active || undefined} data-switching={!!scene.outgoing || undefined}
    width="480" height="204" viewBox={viewBox} aria-hidden="true" focusable="false">
    {scene.outgoing && <ArtworkLayer key={scene.outgoing} id={id + "-" + scene.outgoing} palette={scene.outgoing} outgoing />}
    <ArtworkLayer key={palette} id={id + "-" + palette} palette={palette} drawing={drawing} outgoing={false} />
  </svg>;
}
