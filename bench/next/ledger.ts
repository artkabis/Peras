// Decision 0046, A2, a ledger: a CSV of transactions read, grouped by month
// and by category, with a running balance and the largest movements. The
// specification, written first; the Peras version translates it.

export interface Entry {
  date: string; // YYYY-MM-DD
  category: string;
  cents: number;
  label: string;
}

/** The entries of a CSV with a header line; a line that does not parse is skipped. */
export function parse(csv: string): Entry[] {
  const entries: Entry[] = [];
  const lines = csv.split("\n");
  for (let i = 1; i < lines.length; i++) {
    const line = lines[i];
    if (line === undefined || line.trim() === "") continue;
    const [date, category, amount, label] = line.split(",");
    if (date === undefined || category === undefined || amount === undefined) continue;
    const value = Number(amount);
    if (Number.isNaN(value)) continue;
    entries.push({ date, category, cents: Math.round(value * 100), label: label ?? "" });
  }
  return entries;
}

export interface Report {
  months: Map<string, number>;
  categories: Map<string, number>;
  lowest: number;
  largest: Entry[];
}

export function report(entries: Entry[]): Report {
  const months = new Map<string, number>();
  const categories = new Map<string, number>();
  let balance = 0;
  let lowest = 0;
  const sorted = [...entries].sort((a, b) => a.date.localeCompare(b.date));
  for (const e of sorted) {
    const month = e.date.slice(0, 7);
    months.set(month, (months.get(month) ?? 0) + e.cents);
    categories.set(e.category, (categories.get(e.category) ?? 0) + e.cents);
    balance += e.cents;
    lowest = Math.min(lowest, balance);
  }
  const largest = [...entries].sort((a, b) => Math.abs(b.cents) - Math.abs(a.cents)).slice(0, 5);
  return { months, categories, lowest, largest };
}

export function money(cents: number): string {
  const sign = cents < 0 ? "-" : "";
  return `${sign}${(Math.abs(cents) / 100).toFixed(2)}`;
}

/** The workload: a CSV of n rows made, read and reported. */
export function run(n: number): string {
  const categories = ["food", "rent", "salary", "travel", "books"];
  let csv = "date,category,amount,label\n";
  let seed = 17;
  for (let i = 0; i < n; i++) {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    const r = Math.floor(seed / 256);
    const month = 1 + (r % 12);
    const day = 1 + (Math.floor(r / 12) % 28);
    const category = categories[Math.floor(r / 336) % 5] ?? "";
    const amount = category === "salary" ? 300 : -((Math.floor(r / 1680) % 200) + (r % 100) / 100);
    csv += `2026-${String(month).padStart(2, "0")}-${String(day).padStart(2, "0")},${category},${amount.toFixed(2)},item ${i}\n`;
  }
  const r = report(parse(csv));
  const months = [...r.months.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  return `${months.map(([m, c]) => `${m}:${money(c)}`).join(",")}|${money(r.lowest)}|${r.largest.map((e) => e.label).join(",")}`;
}
