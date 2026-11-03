export function delays(): number[] {
  const out: number[] = [];
  for (var i = 0; i < 3; i++) {
    out.push(i * 100);
  }
  return out;
}
