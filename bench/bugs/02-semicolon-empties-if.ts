declare function grantAccess(): void;

export function check(isAdmin: boolean): void {
  if (isAdmin);
  {
    grantAccess();
  }
}
