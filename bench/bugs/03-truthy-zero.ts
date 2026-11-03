export function describe(stock: number): string {
  if (stock) {
    return "available";
  }
  return "sold out";
}
