// M1 of decision 0040, as Peras is tested today (v0): TypeScript calling
// the exported function; `digit` is private, so it cannot be tested.
import { expect, test } from "vitest";
import { parsePort } from "./MODULE";

test("ports are read", () => {
  expect(parsePort("80")).toEqual({ kind: "Ok", values: [80] });
  expect(parsePort("0")).toEqual({ kind: "Ok", values: [0] });
  expect(parsePort("1")).toEqual({ kind: "Ok", values: [1] });
  expect(parsePort("08080")).toEqual({ kind: "Ok", values: [8080] });
  expect(parsePort("65535").kind).toBe("Ok");
});

test("bad texts are refused", () => {
  expect(parsePort("65536").kind).toBe("Err");
  expect(parsePort("").kind).toBe("Err");
  expect(parsePort("8a").kind).toBe("Err");
});
