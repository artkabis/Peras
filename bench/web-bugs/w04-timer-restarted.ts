// A module-level timer is restarted without stopping the previous one.
let ticks = 0;
let timer: ReturnType<typeof setInterval> | undefined;
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

function start(): void {
  timer = setInterval(() => {
    ticks += 1;
  }, 5);
}

function stop(): void {
  clearInterval(timer);
}

export async function main(): Promise<string> {
  start();
  start();
  stop();
  await sleep(20);
  const seen = ticks;
  await sleep(30);
  return ticks === seen ? "stopped" : "still ticking";
}
