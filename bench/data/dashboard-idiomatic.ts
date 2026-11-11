// Decision 0014, version A: the sales dashboard as a web developer writes it,
// without optimizing.
import { days, end, start, type Summary } from "./summary.ts";

const integer = /^[+-]?[0-9]+$/;
const countryCode = /^[A-Z]{2}$/;

function parseInt32(text: string): number | undefined {
  if (!integer.test(text)) return undefined;
  const n = Number(text);
  return n >= -2147483648 && n <= 2147483647 ? n : undefined;
}

interface Row {
  ts: number;
  user: number;
  country: string;
  product: number;
  amount: number;
  qty: number;
}

function parseRow(line: string): Row | undefined {
  const f = line.split(",");
  if (f.length !== 6) return undefined;
  const [ts, user, country, product, amount, qty] = [
    parseInt32(f[0] ?? ""),
    parseInt32(f[1] ?? ""),
    f[2] ?? "",
    parseInt32(f[3] ?? ""),
    parseInt32(f[4] ?? ""),
    parseInt32(f[5] ?? ""),
  ];
  if (
    ts === undefined ||
    user === undefined ||
    product === undefined ||
    amount === undefined ||
    qty === undefined ||
    !countryCode.test(country)
  ) {
    return undefined;
  }
  return { ts, user, country, product, amount, qty };
}

export function summarize(bytes: Uint8Array): Summary {
  const text = new TextDecoder().decode(bytes);
  const lines = text.split("\n").slice(1);
  if (text.endsWith("\n")) lines.pop();

  let broken = 0;
  let kept = 0;
  const pairs = new Map<
    string,
    { country: string; category: number; count: number; revenue: number }
  >();
  const users = new Map<string, Set<number>>();
  const daily = new Array<number>(days).fill(0);
  const products = new Map<number, number>();

  for (const line of lines) {
    const row = parseRow(line);
    if (row === undefined) {
      broken++;
      continue;
    }
    if (row.qty <= 0 || row.ts < start || row.ts >= end) continue;
    kept++;
    const revenue = row.amount * row.qty;
    const category = row.product % 12;
    const key = `${row.country}|${String(category)}`;
    const pair = pairs.get(key) ?? { country: row.country, category, count: 0, revenue: 0 };
    pair.count++;
    pair.revenue += revenue;
    pairs.set(key, pair);
    const seen = users.get(row.country) ?? new Set<number>();
    seen.add(row.user);
    users.set(row.country, seen);
    const day = Math.floor((row.ts - start) / 86400);
    daily[day] = (daily[day] ?? 0) + revenue;
    products.set(row.product, (products.get(row.product) ?? 0) + revenue);
  }

  const byCode = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);
  return {
    lines: lines.length,
    broken,
    kept,
    pairs: [...pairs.values()].sort(
      (a, b) => byCode(a.country, b.country) || a.category - b.category,
    ),
    customers: [...users.entries()]
      .map(([country, seen]) => ({ country, users: seen.size }))
      .sort((a, b) => byCode(a.country, b.country)),
    daily,
    best: [...products.entries()]
      .map(([product, revenue]) => ({ product, revenue }))
      .sort((a, b) => b.revenue - a.revenue || a.product - b.product)
      .slice(0, 10),
  };
}
