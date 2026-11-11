// A 64-bit id from the server, read as a JavaScript number.
import { parseId } from "./lib.js";

export function main(): string {
  return String(parseId('{"id": 9007199254740993}'));
}
