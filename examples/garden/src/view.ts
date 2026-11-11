// What the two panels show: a square window of the plane [0, 1] × [0, 1],
// `zoom` times smaller than the plane, centred on (cx, cy). Pure functions,
// tested in test/view.test.ts.

export interface View {
  zoom: number;
  cx: number;
  cy: number;
}

/** The whole plane. */
export const home: View = { zoom: 1, cx: 0.5, cy: 0.5 };

/**
 * The deepest zoom offered: 2^16. The detail has no end, but each doubling adds
 * an octave to every noise, so a frame takes longer the deeper it is.
 */
export const maxZoom = 65536;

/** The view kept inside the plane, its zoom between 1 and `maxZoom`. */
export function clamp(view: View): View {
  const zoom = Math.min(maxZoom, Math.max(1, view.zoom));
  const half = 0.5 / zoom;
  const inside = (c: number): number => Math.min(1 - half, Math.max(half, c));
  return { zoom, cx: inside(view.cx), cy: inside(view.cy) };
}

/**
 * Zoomed by `factor` around the point at (u, v) of the panel, as fractions of
 * its side: that point of the plane stays under the cursor.
 */
export function zoomAt(view: View, factor: number, u: number, v: number): View {
  const zoom = Math.min(maxZoom, Math.max(1, view.zoom * factor));
  const x = view.cx + (u - 0.5) / view.zoom;
  const y = view.cy + (v - 0.5) / view.zoom;
  return clamp({ zoom, cx: x - (u - 0.5) / zoom, cy: y - (v - 0.5) / zoom });
}

/** Moved by (du, dv) fractions of the panel's side, as a drag moves it. */
export function pan(view: View, du: number, dv: number): View {
  return clamp({ zoom: view.zoom, cx: view.cx - du / view.zoom, cy: view.cy - dv / view.zoom });
}

/** The detail level the garden draws at this zoom: 6 + level octaves of noise. */
export function level(zoom: number): number {
  let level = 0;
  let z = zoom;
  while (z >= 2) {
    z /= 2;
    level += 1;
  }
  return level;
}

/** The square of a picture of the whole plane, `side` pixels, that the view shows. */
export function window(view: View, side: number): { x: number; y: number; size: number } {
  return {
    x: (view.cx - 0.5 / view.zoom) * side,
    y: (view.cy - 0.5 / view.zoom) * side,
    size: side / view.zoom,
  };
}

/**
 * Where a frame drawn for `drawn` lands in a panel of `side` pixels showing
 * `shown`: the old frame is moved and scaled at once, before the new one is ready.
 */
export function placed(
  drawn: View,
  shown: View,
  side: number,
): { x: number; y: number; size: number } {
  return {
    x: (drawn.cx - 0.5 / drawn.zoom - (shown.cx - 0.5 / shown.zoom)) * shown.zoom * side,
    y: (drawn.cy - 0.5 / drawn.zoom - (shown.cy - 0.5 / shown.zoom)) * shown.zoom * side,
    size: (shown.zoom / drawn.zoom) * side,
  };
}

/** The view in a URL fragment, `#z=8&x=0.37&y=0.83`, to share a place. */
export function toHash(view: View): string {
  return `#z=${String(view.zoom)}&x=${String(view.cx)}&y=${String(view.cy)}`;
}

/** The view a fragment names, or `home` if it names none. */
export function fromHash(hash: string): View {
  const params = new URLSearchParams(hash.replace(/^#/, ""));
  const zoom = Number(params.get("z"));
  const cx = Number(params.get("x"));
  const cy = Number(params.get("y"));
  if (![zoom, cx, cy].every(Number.isFinite) || !params.has("z")) return home;
  return clamp({ zoom, cx, cy });
}
