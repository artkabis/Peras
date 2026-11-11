// Decision 0014, version B: the sales dashboard optimized by hand: the bytes
// read without making strings, keys packed into integers, counts in typed
// arrays, distinct users in bitmaps.
import { days, end, start, type Summary } from "./summary.ts";

const NL = 10;
const COMMA = 44;
const A = 65;
const Z = 90;
const CODES = 26 * 26;
/** Users below this go to a bitmap per country, others to a set. */
const BITS = 1 << 18;
/** Products below this are counted in an array, others in a map. */
const DENSE = 1 << 13;

export function summarize(bytes: Uint8Array): Summary {
  const n = bytes.length;
  let i = bytes.indexOf(NL) + 1;
  if (i === 0) i = n;

  let lines = 0;
  let broken = 0;
  let kept = 0;
  // A category is product % 12, from -11 to 11: stored at category + 11.
  const count = new Int32Array(CODES * 23);
  const revenue = new Float64Array(CODES * 23);
  const seenCode = new Uint8Array(CODES);
  const bitmaps: (Uint32Array | undefined)[] = new Array<Uint32Array | undefined>(CODES);
  const users = new Int32Array(CODES);
  const others: (Set<number> | undefined)[] = new Array<Set<number> | undefined>(CODES);
  const daily = new Float64Array(days);
  const denseRevenue = new Float64Array(DENSE);
  const touched = new Uint8Array(DENSE);
  const sparse = new Map<number, number>();
  const field = new Float64Array(6);

  while (i < n) {
    lines++;
    // The six fields: five strict integers and a code, or the row is broken.
    let f = 0;
    let ok = true;
    let code = -1;
    while (true) {
      if (f === 2) {
        const c0 = bytes[i] ?? 0;
        const c1 = bytes[i + 1] ?? 0;
        if (c0 >= A && c0 <= Z && c1 >= A && c1 <= Z) {
          code = (c0 - A) * 26 + (c1 - A);
          i += 2;
        } else {
          ok = false;
        }
      } else {
        let sign = 1;
        const c = bytes[i];
        if (c === 43 || c === 45) {
          if (c === 45) sign = -1;
          i++;
        }
        let digits = 0;
        let v = 0;
        while (i < n) {
          const d = (bytes[i] ?? 0) - 48;
          if (d < 0 || d > 9) break;
          v = v * 10 + d;
          digits++;
          i++;
        }
        v *= sign;
        if (digits === 0 || v < -2147483648 || v > 2147483647) ok = false;
        field[f] = v;
      }
      const at = i < n ? bytes[i] : NL;
      if (!ok || f === 5 || at !== COMMA) {
        if (at !== NL || f !== 5) ok = false;
        break;
      }
      f++;
      i++;
    }
    // On to the next line.
    if (!ok) {
      while (i < n && bytes[i] !== NL) i++;
      broken++;
      i++;
      continue;
    }
    i++;
    const ts = field[0] ?? 0;
    const qty = field[5] ?? 0;
    if (qty <= 0 || ts < start || ts >= end) continue;
    kept++;
    const user = field[1] ?? 0;
    const product = field[3] ?? 0;
    const r = (field[4] ?? 0) * qty;
    const slot = code * 23 + (product % 12) + 11;
    count[slot] = (count[slot] ?? 0) + 1;
    revenue[slot] = (revenue[slot] ?? 0) + r;
    seenCode[code] = 1;
    if (user >= 0 && user < BITS) {
      let bits = bitmaps[code];
      if (bits === undefined) {
        bits = new Uint32Array(BITS >>> 5);
        bitmaps[code] = bits;
      }
      const word = user >>> 5;
      const mask = 1 << (user & 31);
      const w = bits[word] ?? 0;
      if ((w & mask) === 0) {
        bits[word] = w | mask;
        users[code] = (users[code] ?? 0) + 1;
      }
    } else {
      let set = others[code];
      if (set === undefined) {
        set = new Set<number>();
        others[code] = set;
      }
      if (!set.has(user)) {
        set.add(user);
        users[code] = (users[code] ?? 0) + 1;
      }
    }
    const day = Math.floor((ts - start) / 86400);
    daily[day] = (daily[day] ?? 0) + r;
    if (product >= 0 && product < DENSE) {
      denseRevenue[product] = (denseRevenue[product] ?? 0) + r;
      touched[product] = 1;
    } else {
      sparse.set(product, (sparse.get(product) ?? 0) + r);
    }
  }

  const name = (c: number): string => String.fromCharCode(A + Math.floor(c / 26), A + (c % 26));
  const pairs: Summary["pairs"] = [];
  const customers: Summary["customers"] = [];
  for (let c = 0; c < CODES; c++) {
    if (seenCode[c] === 0) continue;
    customers.push({ country: name(c), users: users[c] ?? 0 });
    for (let k = 0; k < 23; k++) {
      const s = c * 23 + k;
      const n = count[s] ?? 0;
      if (n > 0)
        pairs.push({ country: name(c), category: k - 11, count: n, revenue: revenue[s] ?? 0 });
    }
  }
  const best: { product: number; revenue: number }[] = [];
  for (let p = 0; p < DENSE; p++) {
    if (touched[p] === 1) best.push({ product: p, revenue: denseRevenue[p] ?? 0 });
  }
  for (const [product, r] of sparse) best.push({ product, revenue: r });
  best.sort((a, b) => b.revenue - a.revenue || a.product - b.product);
  return { lines, broken, kept, pairs, customers, daily: [...daily], best: best.slice(0, 10) };
}
