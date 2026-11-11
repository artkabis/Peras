// A library says it returns a number, and returns a string.
import { score } from "./lib.js";

export function main(): string {
  return String(score() + 1);
}
