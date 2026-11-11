// A new status is added and one function does not handle it.
type Status = "idle" | "loading" | "failed" | "done";

function label(s: Status): string {
  switch (s) {
    case "idle":
      return "Idle";
    case "loading":
      return "Loading";
    case "failed":
      return "Failed";
  }
}

export function main(): string {
  return label("done");
}
