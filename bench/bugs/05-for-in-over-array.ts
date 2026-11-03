export function joinNames(names: string[]): string {
  let out = "";
  for (const name in names) {
    out += name;
  }
  return out;
}
