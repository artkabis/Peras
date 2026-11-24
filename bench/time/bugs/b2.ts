// B2: thirty seconds added to milliseconds.
export function deadline(): number {
  const start = performance.now();
  return start + 30;
}
