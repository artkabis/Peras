// B1: five seconds written setTimeout(…, 5).
export function start(done: () => void): ReturnType<typeof setTimeout> {
  return setTimeout(done, 5);
}
