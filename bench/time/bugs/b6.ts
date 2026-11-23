// B6: an instant of performance.now() compared with one of Date.now().
export function expired(createdAt: number): boolean {
  return performance.now() - createdAt > 60_000;
}
export const created = (): number => Date.now();
