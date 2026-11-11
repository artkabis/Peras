// Decision 0014: the sales data, a CSV export generated from a fixed seed with
// the integer hash of decision 0013. The same seed gives the same bytes.

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

export const countries = [
  "US",
  "DE",
  "FR",
  "GB",
  "JP",
  "CN",
  "IN",
  "BR",
  "CA",
  "IT",
  "ES",
  "KR",
  "AU",
  "NL",
  "MX",
  "SE",
  "CH",
  "PL",
  "BE",
  "AT",
];

/** The year 2026, in Unix seconds. */
export const yearStart = 1767225600;
const yearSeconds = 365 * 86400;

/** `rows` rows after the header, as the bytes of the file. */
export function generate(rows: number, seed = 14): Uint8Array {
  const encoder = new TextEncoder();
  const parts: Uint8Array[] = [encoder.encode("ts,user,country,product,amount,qty\n")];
  let chunk: string[] = [];
  let broken = 0;
  for (let i = 0; i < rows; i++) {
    const ts = yearStart + Math.floor(unit(i, 0, seed) * yearSeconds);
    const u1 = unit(i, 1, seed);
    const user = 1 + Math.floor(u1 * u1 * 200000);
    const u2 = unit(i, 2, seed);
    const country = countries[Math.floor(u2 * Math.sqrt(u2) * 20)] ?? "US";
    const u3 = unit(i, 3, seed);
    const product = 1 + Math.floor(u3 * u3 * u3 * 5000);
    const amount = 100 + Math.floor(unit(i, 4, seed) * 99900);
    const qty = unit(i, 5, seed) < 0.02 ? 0 : 1 + Math.floor(unit(i, 6, seed) * 5);
    let line = `${String(ts)},${String(user)},${country},${String(product)},${String(amount)},${String(qty)}`;
    // About one row in a thousand is broken, of three kinds in turn.
    if ((hash(i, 7, seed) >>> 0) % 1000 === 0) {
      const kind = broken % 3;
      broken += 1;
      if (kind === 0) {
        line = line.slice(0, line.lastIndexOf(","));
      } else if (kind === 1) {
        line = `${String(ts)},${String(user)},${country},${String(product)},${String(amount)}a,${String(qty)}`;
      } else {
        line = "";
      }
    }
    chunk.push(line);
    if (chunk.length === 65536) {
      parts.push(encoder.encode(chunk.join("\n") + "\n"));
      chunk = [];
    }
  }
  if (chunk.length > 0) parts.push(encoder.encode(chunk.join("\n") + "\n"));
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}
