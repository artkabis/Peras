// A view adds a listener and nothing removes it when the view closes.
let clicks = 0;

function mount(button: HTMLElement): void {
  button.addEventListener("click", () => {
    clicks += 1;
  });
}

export function main(): string {
  const button = document.getElementById("save");
  if (button === null) return "no button";
  mount(button);
  button.click();
  // The view is closed here; its listener should be gone.
  button.click();
  return String(clicks);
}
