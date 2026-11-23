// B4: milliseconds read as seconds: minutes are the elapsed time over 60.
export function late(start: number): boolean {
  const minutes = (performance.now() - start) / 60;
  return minutes > 5;
}
