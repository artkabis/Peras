import { describe, expect, it } from "vitest";
import {
  clamp,
  fromHash,
  home,
  level,
  maxZoom,
  pan,
  placed,
  toHash,
  window,
  zoomAt,
} from "../src/view.ts";

describe("the playground's view", () => {
  it("stays inside the plane, between zoom 1 and the deepest", () => {
    expect(clamp({ zoom: 0.5, cx: -3, cy: 9 })).toEqual(home);
    expect(clamp({ zoom: 4, cx: 0, cy: 1 })).toEqual({ zoom: 4, cx: 0.125, cy: 0.875 });
    expect(clamp({ zoom: 1e9, cx: 0.5, cy: 0.5 }).zoom).toBe(maxZoom);
  });

  it("keeps the point under the cursor where it was when zooming", () => {
    const view = { zoom: 2, cx: 0.4, cy: 0.6 };
    const [u, v] = [0.8, 0.3];
    const under = (w: typeof view): [number, number] => [
      w.cx + (u - 0.5) / w.zoom,
      w.cy + (v - 0.5) / w.zoom,
    ];
    const next = zoomAt(view, 3, u, v);
    expect(next.zoom).toBe(6);
    const [x0, y0] = under(view);
    const [x1, y1] = under(next);
    expect(x1).toBeCloseTo(x0, 12);
    expect(y1).toBeCloseTo(y0, 12);
  });

  it("moves with a drag, by the panel's fraction over the zoom", () => {
    expect(pan({ zoom: 4, cx: 0.5, cy: 0.5 }, 0.2, -0.4)).toEqual({ zoom: 4, cx: 0.45, cy: 0.6 });
  });

  it("gives the garden's level as its render does", () => {
    expect([1, 1.99, 2, 8, 63.9, 64].map(level)).toEqual([0, 0, 1, 3, 5, 6]);
  });

  it("finds the window of the stored image, and where an old frame lands", () => {
    expect(window({ zoom: 8, cx: 0.375, cy: 0.8125 }, 1024)).toEqual({ x: 320, y: 768, size: 128 });
    const drawn = { zoom: 2, cx: 0.25, cy: 0.25 };
    expect(placed(drawn, drawn, 500)).toEqual({ x: 0, y: 0, size: 500 });
    expect(placed(drawn, home, 500)).toEqual({ x: 0, y: 0, size: 250 });
  });

  it("is shared in the URL and read back exactly", () => {
    const view = { zoom: 64, cx: 0.37, cy: 0.83 };
    expect(fromHash(toHash(view))).toEqual(view);
    expect(fromHash("")).toEqual(home);
    expect(fromHash("#z=abc")).toEqual(home);
  });
});
