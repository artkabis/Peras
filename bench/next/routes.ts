// Decision 0046, A4, a route finder: the shortest path between stops of a
// network read from JSON, a missing stop reported, a priority queue. The
// specification, written first; the Peras version translates it.

export interface Network {
  stops: string[];
  links: { from: string; to: string; minutes: number }[];
}

interface Edge {
  to: number;
  minutes: number;
}

/** A binary heap of (minutes, stop), the smallest first. */
class Queue {
  private items: [number, number][] = [];

  push(minutes: number, stop: number): void {
    this.items.push([minutes, stop]);
    let i = this.items.length - 1;
    while (i > 0) {
      const parent = (i - 1) >> 1;
      if (this.items[parent]![0] <= this.items[i]![0]) break;
      [this.items[parent], this.items[i]] = [this.items[i]!, this.items[parent]!];
      i = parent;
    }
  }

  pop(): [number, number] | undefined {
    const top = this.items[0];
    const last = this.items.pop();
    if (top === undefined || last === undefined) return undefined;
    if (this.items.length > 0) {
      this.items[0] = last;
      let i = 0;
      for (;;) {
        const l = i * 2 + 1;
        const r = l + 1;
        let m = i;
        if (l < this.items.length && this.items[l]![0] < this.items[m]![0]) m = l;
        if (r < this.items.length && this.items[r]![0] < this.items[m]![0]) m = r;
        if (m === i) break;
        [this.items[m], this.items[i]] = [this.items[i]!, this.items[m]!];
        i = m;
      }
    }
    return top;
  }
}

export type Route = { minutes: number; stops: string[] } | { missing: string };

/** The shortest route from one stop to another, or the stop that does not exist. */
export function route(network: Network, from: string, to: string): Route | undefined {
  const index = new Map(network.stops.map((s, i) => [s, i] as const));
  const start = index.get(from);
  if (start === undefined) return { missing: from };
  const end = index.get(to);
  if (end === undefined) return { missing: to };
  const edges: Edge[][] = network.stops.map(() => []);
  for (const l of network.links) {
    const a = index.get(l.from);
    const b = index.get(l.to);
    if (a === undefined || b === undefined) continue;
    edges[a]!.push({ to: b, minutes: l.minutes });
    edges[b]!.push({ to: a, minutes: l.minutes });
  }
  const best: number[] = network.stops.map(() => Infinity);
  const previous: (number | undefined)[] = network.stops.map(() => undefined);
  const queue = new Queue();
  best[start] = 0;
  queue.push(0, start);
  for (let next = queue.pop(); next !== undefined; next = queue.pop()) {
    const [minutes, stop] = next;
    if (minutes > best[stop]!) continue;
    if (stop === end) break;
    for (const e of edges[stop]!) {
      const m = minutes + e.minutes;
      if (m < best[e.to]!) {
        best[e.to] = m;
        previous[e.to] = stop;
        queue.push(m, e.to);
      }
    }
  }
  if (best[end] === Infinity) return undefined;
  const stops: string[] = [];
  for (let at: number | undefined = end; at !== undefined; at = previous[at]) {
    stops.push(network.stops[at]!);
  }
  return { minutes: best[end]!, stops: stops.reverse() };
}

/** The workload: a grid network of n by n stops as JSON, read, then routes asked. */
export function run(n: number): string {
  const stops: string[] = [];
  const links: Network["links"] = [];
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      stops.push(`s${x}-${y}`);
      if (x > 0)
        links.push({
          from: `s${x - 1}-${y}`,
          to: `s${x}-${y}`,
          minutes: 1 + ((x * 7 + y * 3) % 9),
        });
      if (y > 0)
        links.push({
          from: `s${x}-${y - 1}`,
          to: `s${x}-${y}`,
          minutes: 1 + ((x * 5 + y * 11) % 9),
        });
    }
  }
  const network = JSON.parse(JSON.stringify({ stops, links })) as Network;
  const out: string[] = [];
  for (const [a, b] of [
    ["s0-0", `s${n - 1}-${n - 1}`],
    [`s${n - 1}-0`, `s0-${n - 1}`],
    ["s0-0", "nowhere"],
  ] as const) {
    const r = route(network, a, b);
    if (r === undefined) out.push("none");
    else if ("missing" in r) out.push(`missing ${r.missing}`);
    else out.push(`${r.minutes}:${r.stops.length}`);
  }
  return out.join("|");
}
