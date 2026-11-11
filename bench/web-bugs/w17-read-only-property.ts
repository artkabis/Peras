// A property the browser sets, assigned.
export function main(): string {
  const e = new Event("click");
  e.type = "submit";
  return e.type;
}
