// A typo in an id: the element does not exist.
export function main(): string {
  const total = document.getElementById("totl");
  total.textContent = "3";
  return "shown";
}
