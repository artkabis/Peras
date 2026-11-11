// An async save that is not awaited.
let saved = false;
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

async function save(): Promise<void> {
  await sleep(5);
  saved = true;
}

export async function main(): Promise<string> {
  save();
  await sleep(0);
  return saved ? "saved" : "not saved yet";
}
