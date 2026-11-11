// A promise used as the value it will give.
import { fetchUser } from "./lib.js";

export function main(): string {
  const user = fetchUser(1);
  return user.name;
}
