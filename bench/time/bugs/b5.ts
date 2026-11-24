// B5: a timeout of some minutes, a zero missing.
export function remind(minutes: number, done: () => void): ReturnType<typeof setTimeout> {
  return setTimeout(done, minutes * 60 * 100);
}
