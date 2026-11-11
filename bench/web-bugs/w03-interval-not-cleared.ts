// A view starts a timer and nothing stops it when the view closes.
let ticks = 0;
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

function start(): void {
  setInterval(() => {
    ticks += 1;
  }, 5);
}

export async function main(): Promise<string> {
  start();
  await sleep(30);
  // The view is closed here; its timer should be stopped.
  const seen = ticks;
  await sleep(30);
  return ticks === seen ? "stopped" : "still ticking";
}
