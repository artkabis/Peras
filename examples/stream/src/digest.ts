// The digest of telemetry.peras, written as a web developer writes it: the
// reference the page compares Peras with, batch by batch.
import type { Digest, Zone } from "./telemetry.peras";

export type { Digest, Zone };

export const bins = 30;

const integer = /^[+-]?[0-9]+$/;
const decimal = /^[+-]?([0-9]+[.]?[0-9]*|[.][0-9]+)([eE][+-]?[0-9]+)?$/;
const zoneName = /^[a-z]{1,12}$/;

function readInt(text: string, bits: 32 | 64): bigint | undefined {
  if (!integer.test(text)) return undefined;
  const n = BigInt(text);
  const limit = 1n << BigInt(bits - 1);
  return n >= -limit && n < limit ? n : undefined;
}

function readDecimal(text: string): number | undefined {
  return decimal.test(text) ? Number(text) : undefined;
}

interface Reading {
  sensor: number;
  zone: string;
  temp: number;
  humidity: number;
  watts: number;
}

function parse(line: string): Reading | undefined {
  const f = line.split(",");
  if (f.length !== 6 || !zoneName.test(f[2] ?? "")) return undefined;
  const ts = readInt(f[0] ?? "", 64);
  const sensor = readInt(f[1] ?? "", 32);
  const temp = readDecimal(f[3] ?? "");
  const humidity = readDecimal(f[4] ?? "");
  const watts = readDecimal(f[5] ?? "");
  if (
    ts === undefined ||
    sensor === undefined ||
    temp === undefined ||
    humidity === undefined ||
    watts === undefined
  )
    return undefined;
  if (ts < 0n || temp < -60 || temp > 80 || humidity < 0 || humidity > 100 || watts < 0)
    return undefined;
  return { sensor: Number(sensor), zone: f[2] ?? "", temp, humidity, watts };
}

export function digest(text: string): Digest {
  let lines = 0;
  let broken = 0;
  let kept = 0;
  const zones = new Map<string, Zone>();
  const histogram = new Array<number>(bins).fill(0);
  let hottestSensor = -1;
  let hottest = -1000;
  for (const line of text.split("\n")) {
    if (line.length === 0) continue;
    lines++;
    const r = parse(line);
    if (r === undefined) {
      broken++;
      continue;
    }
    kept++;
    let z = zones.get(r.zone);
    if (z === undefined) {
      z = {
        name: r.zone,
        readings: 0,
        alerts: 0,
        tempSum: 0,
        tempMin: r.temp,
        tempMax: r.temp,
        humiditySum: 0,
        energy: 0,
      };
      zones.set(r.zone, z);
    }
    z.readings++;
    z.tempSum += r.temp;
    z.tempMin = Math.min(z.tempMin, r.temp);
    z.tempMax = Math.max(z.tempMax, r.temp);
    z.humiditySum += r.humidity;
    z.energy += r.watts;
    if (r.temp > 35 || r.humidity > 90) z.alerts++;
    const bin = Math.min(29, Math.max(0, Math.floor((r.temp + 10) / 2)));
    histogram[bin] = (histogram[bin] ?? 0) + 1;
    if (r.temp > hottest) {
      hottest = r.temp;
      hottestSensor = r.sensor;
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

/** A digest as canonical text: numbers as String writes them, -0 kept. */
export function canonical(d: Digest): string {
  const n = (x: number): string => (Object.is(x, -0) ? "-0" : String(x));
  const zones = d.zones.map(
    (z) =>
      `${z.name}:${n(z.readings)},${n(z.alerts)},${n(z.tempSum)},${n(z.tempMin)},${n(z.tempMax)},${n(z.humiditySum)},${n(z.energy)}`,
  );
  return `${n(d.lines)}|${n(d.broken)}|${n(d.kept)}|${zones.join(";")}|${d.histogram.map(n).join(",")}|${n(d.hottestSensor)}|${n(d.hottest)}`;
}
