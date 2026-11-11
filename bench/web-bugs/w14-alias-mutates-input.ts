// A "sorted copy" that sorts the caller's array.
function sortedCopy(items: number[]): number[] {
  const sorted = items;
  sorted.sort((a, b) => a - b);
  return sorted;
}

export function main(): string {
  const xs = [3, 1, 2];
  sortedCopy(xs);
  return xs.join(",");
}
