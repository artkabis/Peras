// The stream: readings of 480 sensors in twelve zones of a building, as CSV
// text, one batch at a time. A batch is a pure function of its number and
// the seed: the same batch gives the same bytes, in any browser, on any run.

/** lowbias32 (Wellons, 2018) of a row, a field and a seed. */
function hash(row: number, field: number, seed: number): number {
  let h =
    (Math.imul(row, 374761393) + Math.imul(field, 668265263) + Math.imul(seed, 1274126177)) | 0;
  h ^= h >>> 16;
  h = Math.imul(h, 2146121005);
  h ^= h >>> 15;
  h = Math.imul(h, -2073254261);
  h ^= h >>> 16;
  return h;
}

/** 24 bits of a hash as a number in [0, 1). */
function unit(row: number, field: number, seed: number): number {
  return (hash(row, field, seed) & 16777215) / 16777216;
}

export const zones = [
  "atrium",
  "boiler",
  "cellar",
  "dock",
  "garage",
  "hall",
  "lab",
  "loft",
  "office",
  "roof",
  "server",
  "vault",
] as const;

/** The usual temperature of each zone, in °C. */
const base = [21, 31, 13, 9, 11, 19, 20, 24, 21, 6, 27, 16];

export const sensors = 480;

/** The second the stream starts at: 2026-11-01, in Unix seconds. */
export const origin = 1793491200;

/** One reading every 5 s per sensor: a row's time, from its index. */
const step = 5 / sensors;

/** Whether row `i` is broken on purpose: about one in 400. */
export function isBroken(i: number, seed = 7): boolean {
  return (hash(i, 7, seed) >>> 0) % 400 === 0;
}

/** How many of rows `first` to `first + rows - 1` are broken on purpose. */
export function injected(first: number, rows: number, seed = 7): number {
  let n = 0;
  for (let i = first; i < first + rows; i++) if (isBroken(i, seed)) n++;
  return n;
}

/**
 * Rows `first` to `first + rows - 1` of the stream as CSV lines, each ended
 * by "\n". About one row in 400 is broken, of five kinds in turn.
 */
export function batch(first: number, rows: number, seed = 7): string {
  const out: string[] = [];
  for (let i = first; i < first + rows; i++) {
    const sensor = i % sensors;
    const zone = sensor % zones.length;
    const ts = origin + Math.floor(i * step);
    // A day's swing, the sensor's own offset, noise, and rare spikes.
    const day = Math.sin(((ts % 86400) / 86400) * 2 * Math.PI);
    const spike = unit(i, 1, seed) < 0.004 ? 12 + 10 * unit(i, 2, seed) : 0;
    const temp =
      (base[zone] ?? 20) + 3 * day + 2 * (unit(sensor, 3, seed) - 0.5) + unit(i, 4, seed) + spike;
    const humidity = 35 + 30 * unit(i, 5, seed) + (zone === 2 ? 30 : 0);
    const watts = zone === 10 ? 800 + 400 * unit(i, 6, seed) : 20 * unit(i, 6, seed);
    const name = zones[zone] ?? "hall";
    let line = `${String(ts)},${String(sensor)},${name},${temp.toFixed(2)},${humidity.toFixed(1)},${watts.toFixed(3)}`;
    if (isBroken(i, seed)) {
      const kind = (hash(i, 8, seed) >>> 0) % 5;
      if (kind === 0) line = line.slice(0, line.lastIndexOf(","));
      else if (kind === 1) line = line.replace(temp.toFixed(2), "NaN");
      else if (kind === 2) line = line.replace(temp.toFixed(2), temp.toFixed(2).replace(".", ","));
      else if (kind === 3) line = line.replace(name, name.toUpperCase());
      else line = line.replace(humidity.toFixed(1), `${humidity.toFixed(1)}%`);
    }
    out.push(line);
  }
  return out.join("\n") + "\n";
}
