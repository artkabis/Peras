// M2 of decision 0040, as Peras is tested today (v0): a Duration crosses
// to JavaScript as its milliseconds.
import { expect, test } from "vitest";
import { countdown } from "./MODULE";

test("a countdown is written", () => {
  expect(countdown(125 * 60_000)).toBe("2 h 05 min");
  expect(countdown(0)).toBe("0 h 00 min");
  expect(countdown(59.9 * 60_000)).toBe("0 h 59 min");
  expect(countdown(3_600_000)).toBe("1 h 00 min");
  expect(countdown(10 * 60_000)).toBe("0 h 10 min");
  expect(countdown(36_000_000)).toBe("10 h 00 min");
});
