// A setting read from storage that was never written.
export function main(): string {
  return localStorage.getItem("theme").toUpperCase();
}
