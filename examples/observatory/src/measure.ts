// The visitor's measure (docs/en/observatory.md, 3): each version draws the
// same frames with no canvas, 21 rounds, the order alternating round by
// round; each round's time over the TypeScript's, its median and range.

export interface Version {
  name: string;
  /** Draws frame `f`. */
  draw(f: number): void;
}

export interface Summary {
  median: number;
  low: number;
  high: number;
}

export interface Rounds {
  /** Each version's time of each round, in milliseconds. */
  times: Record<string, number[]>;
  /** Each version's time over the reference's, round by round. */
  ratios: Record<string, number[]>;
}

export function summary(xs: readonly number[]): Summary {
  const s = [...xs].sort((a, b) => a - b);
  return {
    median: s[Math.floor(s.length / 2)] ?? Number.NaN,
    low: s[0] ?? Number.NaN,
    high: s[s.length - 1] ?? Number.NaN,
  };
}

/**
 * Times `rounds` rounds of `frames` frames, the first version being the
 * reference, and calls `progress` after each round so the page can breathe.
 */
export async function measure(
  versions: readonly Version[],
  rounds: number,
  frames: number,
  now: () => number,
  progress: (done: number) => Promise<void>,
): Promise<Rounds> {
  const times: Record<string, number[]> = Object.fromEntries(versions.map((v) => [v.name, []]));
  // One round to warm each version up, not kept.
  for (const v of versions) for (let f = 0; f < frames; f++) v.draw(f);
  for (let round = 0; round < rounds; round++) {
    const order = round % 2 === 0 ? versions : [...versions].reverse();
    for (const v of order) {
      const t0 = now();
      for (let f = 0; f < frames; f++) v.draw(round * frames + f);
      times[v.name]?.push(now() - t0);
    }
    await progress(round + 1);
  }
  const reference = versions[0]?.name ?? "";
  const ratios: Record<string, number[]> = {};
  for (const v of versions) {
    ratios[v.name] = (times[v.name] ?? []).map((t, i) => t / (times[reference]?.[i] ?? Number.NaN));
  }
  return { times, ratios };
}
