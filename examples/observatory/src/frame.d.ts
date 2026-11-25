// What the page imports from decision 0047's module, bench/frame/canvas-f3.peras:
// the declarations vite-plugin-peras would write beside it, kept here so that
// bench/ holds only the benchmark (the plugin runs with `dts: false`).
declare module "*/canvas-f3.peras" {
  export interface PerasKinds {
    u32: Uint32Array;
  }
  export declare class PerasArray<K extends keyof PerasKinds> {
    readonly kind: K;
    readonly length: number;
    view(): PerasKinds[K];
    release(): void;
  }
  export declare const __perasHeap: {
    live(): number;
    array<K extends keyof PerasKinds>(kind: K, init: number | readonly number[]): PerasArray<K>;
  };
  export declare function draw(
    pixels: PerasArray<"u32">,
    width: number,
    height: number,
    t: number,
  ): void;
  export declare function frame(width: number, height: number, t: number): number[];
}
