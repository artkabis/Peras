// Decision 0046, A1, notes: a list of notes kept in the page's storage as
// JSON, searched as the user types, sorted by date, with tags. The
// specification, written first; the Peras version translates it.

export interface Note {
  id: number;
  title: string;
  body: string;
  tags: string[];
  updated: number;
}

/** The notes in a stored text, or none when it is missing or broken. */
export function load(text: string | null): Note[] {
  if (text === null) return [];
  try {
    const parsed = JSON.parse(text) as { notes: Note[] };
    return parsed.notes;
  } catch {
    return [];
  }
}

export function save(notes: Note[]): string {
  return JSON.stringify({ notes });
}

/** The notes whose title, body or tags hold every word of the query, newest first. */
export function search(notes: Note[], query: string): Note[] {
  const words = query
    .toLowerCase()
    .split(" ")
    .filter((w) => w.length > 0);
  const found = notes.filter((n) => {
    const text = `${n.title} ${n.body} ${n.tags.join(" ")}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
  return found.sort((a, b) => b.updated - a.updated);
}

/** Each tag, with how many notes carry it, the most used first. */
export function tagCounts(notes: Note[]): [string, number][] {
  const counts = new Map<string, number>();
  for (const n of notes) {
    for (const t of n.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
  }
  return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

export function add(
  notes: Note[],
  title: string,
  body: string,
  tags: string[],
  now: number,
): Note[] {
  const id = notes.reduce((m, n) => Math.max(m, n.id), 0) + 1;
  return [...notes, { id, title, body, tags, updated: now }];
}

/** The workload: n notes made, saved, loaded back, then searched and counted. */
export function run(n: number): string {
  let notes: Note[] = [];
  const topics = ["work", "home", "ideas", "books", "travel"];
  for (let i = 0; i < n; i++) {
    const tags = [topics[i % 5] ?? "", topics[(i * 3) % 5] ?? ""];
    notes = add(notes, `Note ${i}`, `remember item ${i % 97} for ${topics[i % 5]}`, tags, 1000 + i);
  }
  const again = load(save(notes));
  const hits = search(again, "item 4 work");
  const counts = tagCounts(again);
  return `${again.length}|${hits.length}|${hits[0]?.id ?? -1}|${counts.map(([t, c]) => `${t}:${c}`).join(",")}`;
}

/** The page: a search field, the list, the tags; typing searches after a pause. */
export function mount(root: HTMLElement): () => void {
  let notes = load(localStorage.getItem("notes"));
  const field = document.createElement("input");
  const list = document.createElement("ul");
  root.append(field, list);
  const show = (): void => {
    list.replaceChildren(
      ...search(notes, field.value).map((n) => {
        const li = document.createElement("li");
        li.textContent = `${n.title} (${n.tags.join(", ")})`;
        return li;
      }),
    );
  };
  let timer: number | undefined;
  const onInput = (): void => {
    clearTimeout(timer);
    timer = setTimeout(show, 150);
  };
  field.addEventListener("input", onInput);
  notes = add(notes, "Welcome", "first note", ["home"], Date.now());
  localStorage.setItem("notes", save(notes));
  show();
  return () => {
    clearTimeout(timer);
    field.removeEventListener("input", onInput);
    root.replaceChildren();
  };
}
