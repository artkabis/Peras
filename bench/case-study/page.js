// What the case study does in the page, for one version of the table: mount
// it over `count` orders, search, sort and remove it, timing each step and
// reading what the page shows (scripts/bench-case-study.mjs).
export async function measure(url, count) {
  const m = await import(url);
  const names = ["Ada", "Brian", "Chloé", "Dmitri", "Emma", "Farid", "Grace", "Hugo"];
  let seed = 7;
  const random = () => {
    seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    return seed / 4294967296;
  };
  const orders = Array.from({ length: count }, (_, i) => ({
    id: i + 1,
    customer: `${names[Math.floor(random() * names.length)]} ${String(Math.floor(random() * 50))}`,
    amount: Math.round(random() * 100000) / 100,
    paid: random() > 0.2,
  }));
  const app = document.getElementById("app");
  const items = () => app.querySelectorAll("li").length;
  const time = (f) => {
    const t0 = performance.now();
    f();
    return performance.now() - t0;
  };
  let handle;
  const mount = time(() => {
    handle = m.mount(app, orders);
  });
  const input = app.querySelector("input");
  const button = app.querySelector("button");
  input.value = "ada";
  input.dispatchEvent(new Event("input"));
  // Both search after a delay of 50 ms.
  await new Promise((r) => setTimeout(r, 80));
  const shown = items();
  const summary = app.querySelector("p")?.textContent ?? "";
  const sort = time(() => button.click());
  const firstSorted = app.querySelector("li")?.textContent ?? "";
  // Removed: is the page emptied, and does anything still react?
  if (typeof handle === "function") handle();
  else handle.release();
  const left = items();
  input.value = "zzz";
  input.dispatchEvent(new Event("input"));
  button.click();
  await new Promise((r) => setTimeout(r, 80));
  const reacts =
    items() !== left ||
    (app.querySelector("li")?.textContent ?? "") !== (left > 0 ? firstSorted : "");
  // Two tables in one page, each sorted once: with their own state, both
  // turn ascending; with one shared, the second click turns it back.
  const one = document.createElement("div");
  const two = document.createElement("div");
  document.body.append(one, two);
  const few = orders.slice(0, 20);
  const handles = [m.mount(one, few), m.mount(two, few)];
  const firstOf = (root) => root.querySelector("li")?.textContent ?? "";
  const before = firstOf(two);
  one.querySelector("button").click();
  two.querySelector("button").click();
  const twice = firstOf(two) !== before && firstOf(two) === firstOf(one);
  for (const h of handles) {
    if (typeof h === "function") h();
    else h.release();
  }
  return { mount, sort, shown, firstSorted, summary, left, reacts, twice };
}
