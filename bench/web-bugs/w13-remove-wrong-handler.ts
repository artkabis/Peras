// A listener removed with another function than the one added.
let clicks = 0;

export function main(): string {
  const button = document.getElementById("save");
  if (button === null) return "no button";
  button.addEventListener("click", () => {
    clicks += 1;
  });
  button.click();
  button.removeEventListener("click", () => {
    clicks += 1;
  });
  button.click();
  return String(clicks);
}
