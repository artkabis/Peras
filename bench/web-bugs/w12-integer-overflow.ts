// A total of file sizes, in bytes, above 2 GiB.
function total(sizes: number[]): number {
  let sum = 0;
  for (const s of sizes) {
    sum += s;
  }
  return sum;
}

export function main(): string {
  return String(total([1500000000, 1500000000]));
}
