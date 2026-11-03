declare function log(value: number): void;

export function total(prices: number[]): number {
  let sum = 0;
  for (const price of prices) {
    let sum = price * 1.2;
    log(sum);
  }
  return sum;
}
