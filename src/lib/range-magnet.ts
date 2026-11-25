export function magnetAt(raw: number, pixelsPerUnit: number, previous: number | undefined, min: number, max: number, interval: number) {
  const within = (mark: number, pixels: number, fraction: number) => {
    if (mark < min || mark > max) return false;
    const gap = Math.min(interval, mark === min ? interval : mark - min, mark === max ? interval : max - mark);
    // Leave room between magnets on narrow tracks and near partial end intervals.
    return Math.abs(raw - mark) * pixelsPerUnit <= Math.min(pixels, gap * pixelsPerUnit * fraction);
  };
  if (previous !== undefined && within(previous, 6, 0.36)) return previous;
  const nearest = Math.round(raw / interval) * interval;
  return within(nearest, 4, 0.24) ? nearest : undefined;
}
