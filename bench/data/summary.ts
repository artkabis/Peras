// Decision 0014: the summary every version gives, and its canonical JSON.

export interface Pair {
  country: string;
  category: number;
  count: number;
  revenue: number | bigint;
}

export interface Summary {
  lines: number;
  broken: number;
  kept: number;
  pairs: Pair[];
  customers: { country: string; users: number }[];
  daily: (number | bigint)[];
  best: { product: number; revenue: number | bigint }[];
}

/** The first and last second kept: the second half of 2026. */
export const start = 1782864000;
export const end = 1798761600;
export const days = 184;

const int = (n: number | bigint): string => String(n);

/** The summary as canonical JSON: keys in order, integers in decimal. */
export function canonical(s: Summary): string {
  const pairs = s.pairs.map(
    (p) =>
      `{"country":${JSON.stringify(p.country)},"category":${int(p.category)},"count":${int(p.count)},"revenue":${int(p.revenue)}}`,
  );
  const customers = s.customers.map(
    (c) => `{"country":${JSON.stringify(c.country)},"users":${int(c.users)}}`,
  );
  const best = s.best.map((b) => `{"product":${int(b.product)},"revenue":${int(b.revenue)}}`);
  return (
    `{"lines":${int(s.lines)},"broken":${int(s.broken)},"kept":${int(s.kept)},` +
    `"pairs":[${pairs.join(",")}],"customers":[${customers.join(",")}],` +
    `"daily":[${s.daily.map(int).join(",")}],"best":[${best.join(",")}]}`
  );
}
