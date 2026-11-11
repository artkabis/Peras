// The case study, in TypeScript: a table of orders, searched with a delay,
// sorted by a column, with a summary, and removed cleanly.
export interface Order {
  id: number;
  customer: string;
  amount: number;
  paid: boolean;
}

export interface Summary {
  count: number;
  total: number;
  unpaid: number;
  top: string;
}

export function summarize(orders: readonly Order[]): Summary {
  const byCustomer = new Map<string, number>();
  let total = 0;
  let unpaid = 0;
  for (const o of orders) {
    total += o.amount;
    if (!o.paid) unpaid += 1;
    byCustomer.set(o.customer, (byCustomer.get(o.customer) ?? 0) + o.amount);
  }
  let top = "";
  let best = -1;
  for (const [customer, amount] of byCustomer) {
    if (amount > best) {
      best = amount;
      top = customer;
    }
  }
  return { count: orders.length, total, unpaid, top };
}

export function visible(
  orders: readonly Order[],
  query: string,
  byAmount: boolean,
  ascending: boolean,
): Order[] {
  const q = query.trim().toLowerCase();
  const kept = orders.filter((o) => o.customer.toLowerCase().includes(q));
  kept.sort((a, b) => {
    const order = byAmount ? a.amount - b.amount : a.customer.localeCompare(b.customer);
    return ascending ? order : -order;
  });
  return kept;
}

export function mount(root: HTMLElement, orders: readonly Order[]): () => void {
  const input = document.createElement("input");
  input.placeholder = "Customer";
  const sortButton = document.createElement("button");
  sortButton.textContent = "Sort by amount";
  const summary = document.createElement("p");
  const list = document.createElement("ul");
  root.append(input, sortButton, summary, list);

  let query = "";
  let ascending = false;
  let pending: ReturnType<typeof setTimeout> | undefined;

  const render = (): void => {
    const rows = visible(orders, query, true, ascending);
    list.replaceChildren(
      ...rows.map((o) => {
        const li = document.createElement("li");
        li.textContent = `#${String(o.id)} ${o.customer} ${o.amount.toFixed(2)}${o.paid ? "" : " (unpaid)"}`;
        return li;
      }),
    );
    const s = summarize(rows);
    summary.textContent = `${String(s.count)} orders, ${s.total.toFixed(2)} total, ${String(s.unpaid)} unpaid, top: ${s.top}`;
  };

  const onInput = (): void => {
    clearTimeout(pending);
    pending = setTimeout(() => {
      query = input.value;
      render();
    }, 50);
  };
  const onSort = (): void => {
    ascending = !ascending;
    render();
  };
  input.addEventListener("input", onInput);
  sortButton.addEventListener("click", onSort);
  render();

  return () => {
    input.removeEventListener("input", onInput);
    sortButton.removeEventListener("click", onSort);
    clearTimeout(pending);
    root.replaceChildren();
  };
}
