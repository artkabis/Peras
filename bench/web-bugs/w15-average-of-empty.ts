// The average of an empty list.
function average(counts: number[]): number {
  let total = 0;
  for (const c of counts) {
    total += c;
  }
  return total / counts.length;
}

export function main(): string {
  return `average: ${String(average([]))}`;
}
