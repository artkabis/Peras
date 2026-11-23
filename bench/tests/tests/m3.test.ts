// M3 of decision 0040, as Peras is tested today (v0): structures cross as
// objects.
import { expect, test } from "vitest";
import { firstOfEach } from "./MODULE";

const row = (key: string, value: number) => ({ key, value });

test("the first of each key is kept", () => {
  expect(firstOfEach([row("a", 1), row("b", 2), row("a", 3)])).toEqual([1, 2]);
  expect(firstOfEach([])).toEqual([]);
  expect(firstOfEach([row("x", 5)])).toEqual([5]);
  expect(firstOfEach([row("a", 1), row("a", 2), row("a", 3)])).toEqual([1]);
  expect(firstOfEach([row("a", 1), row("b", 2), row("c", 3)])).toEqual([1, 2, 3]);
  expect(firstOfEach([row("b", 1), row("a", 2), row("b", 3), row("a", 4)])).toEqual([1, 2]);
});
