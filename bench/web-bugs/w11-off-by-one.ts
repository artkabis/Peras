// A loop that reads one element past the end.
export function main(): string {
  const names = ["a", "b", "c"];
  let out = "";
  for (let i = 0; i <= names.length; i++) {
    out += names[i].toUpperCase();
  }
  return out;
}
