// Decision 0015, version B: the stream's digest in TypeScript, optimized by
// hand. The text is walked once by character codes: no split, no regular
// expression, no array per line; a number's text is checked by hand and read
// by Number only once known to be one. It gives the very digest of version A
// (examples/stream/src/digest.ts), the sums of floats included.
import type { Digest, Zone } from "../../examples/stream/src/telemetry.peras";

const bins = 30;
const COMMA = 44;
const NEWLINE = 10;
const MINUS = 45;
const PLUS = 43;
const DOT = 46;
const ZERO = 48;
const NINE = 57;

const isDigit = (c: number): boolean => c >= ZERO && c <= NINE;

/** A strict integer between `from` and `to` that fits `bits` bits, or undefined. */
function int(text: string, from: number, to: number, bits: 32 | 64): number | undefined {
  let i = from;
  const c0 = text.charCodeAt(i);
  if (c0 === PLUS || c0 === MINUS) i++;
  if (i === to) return undefined;
  for (let k = i; k < to; k++) if (!isDigit(text.charCodeAt(k))) return undefined;
  // Up to 15 digits, Number is exact; beyond, BigInt decides the range.
  if (to - i <= 15) return Number(text.slice(from, to));
  const n = BigInt(text.slice(from, to));
  const limit = 1n << BigInt(bits - 1);
  if (n < -limit || n >= limit) return undefined;
  return n < 0n ? -1 : 1; // only the sign of a large value is read here
}

/** The decimal number of parseF64's grammar between `from` and `to`, or undefined. */
function decimal(text: string, from: number, to: number): number | undefined {
  let i = from;
  const c0 = text.charCodeAt(i);
  if (c0 === PLUS || c0 === MINUS) i++;
  let digits = 0;
  while (i < to && isDigit(text.charCodeAt(i))) {
    i++;
    digits++;
  }
  if (i < to && text.charCodeAt(i) === DOT) {
    i++;
    while (i < to && isDigit(text.charCodeAt(i))) {
      i++;
      digits++;
    }
  }
  if (digits === 0) return undefined;
  if (i < to) {
    const e = text.charCodeAt(i);
    if (e !== 101 && e !== 69) return undefined;
    i++;
    const s = text.charCodeAt(i);
    if (i < to && (s === PLUS || s === MINUS)) i++;
    if (i === to) return undefined;
    while (i < to) if (!isDigit(text.charCodeAt(i++))) return undefined;
  }
  return Number(text.slice(from, to));
}

/** Whether the text between `from` and `to` is 1 to 12 letters a to z. */
function zoneName(text: string, from: number, to: number): boolean {
  if (to <= from || to - from > 12) return false;
  for (let i = from; i < to; i++) {
    const c = text.charCodeAt(i);
    if (c < 97 || c > 122) return false;
  }
  return true;
}

export function digest(text: string): Digest {
  let lines = 0;
  let broken = 0;
  let kept = 0;
  const zones = new Map<string, Zone>();
  const histogram = new Array<number>(bins).fill(0);
  let hottestSensor = -1;
  let hottest = -1000;
  const commas = [0, 0, 0, 0, 0];
  let start = 0;
  const end = text.length;
  while (start < end) {
    let stop = text.indexOf("\n", start);
    if (stop < 0) stop = end;
    const lineStart = start;
    start = stop + 1;
    if (stop === lineStart) continue;
    lines++;
    // Exactly five commas: six fields.
    let n = 0;
    for (let i = lineStart; i < stop; i++) {
      if (text.charCodeAt(i) === COMMA) {
        if (n === 5) {
          n = 6;
          break;
        }
        commas[n++] = i;
      }
    }
    const [c1 = 0, c2 = 0, c3 = 0, c4 = 0, c5 = 0] = commas;
    if (n !== 5 || !zoneName(text, c2 + 1, c3)) {
      broken++;
      continue;
    }
    const ts = int(text, lineStart, c1, 64);
    const sensor = int(text, c1 + 1, c2, 32);
    const temp = decimal(text, c3 + 1, c4);
    const humidity = decimal(text, c4 + 1, c5);
    const watts = decimal(text, c5 + 1, stop);
    if (
      ts === undefined ||
      sensor === undefined ||
      temp === undefined ||
      humidity === undefined ||
      watts === undefined ||
      ts < 0 ||
      temp < -60 ||
      temp > 80 ||
      humidity < 0 ||
      humidity > 100 ||
      watts < 0
    ) {
      broken++;
      continue;
    }
    kept++;
    const name = text.slice(c2 + 1, c3);
    let z = zones.get(name);
    if (z === undefined) {
      z = {
        name,
        readings: 0,
        alerts: 0,
        tempSum: 0,
        tempMin: temp,
        tempMax: temp,
        humiditySum: 0,
        energy: 0,
      };
      zones.set(name, z);
    }
    z.readings++;
    z.tempSum += temp;
    if (temp < z.tempMin) z.tempMin = temp;
    if (temp > z.tempMax) z.tempMax = temp;
    z.humiditySum += humidity;
    z.energy += watts;
    if (temp > 35 || humidity > 90) z.alerts++;
    const bin = Math.min(29, Math.max(0, Math.floor((temp + 10) / 2)));
    histogram[bin] = (histogram[bin] ?? 0) + 1;
    if (temp > hottest) {
      hottest = temp;
      hottestSensor = sensor;
    }
  }
  return {
    lines,
    broken,
    kept,
    zones: [...zones.values()].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0)),
    histogram,
    hottestSensor,
    hottest,
  };
}
