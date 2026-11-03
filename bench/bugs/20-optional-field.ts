export interface User {
  name: string;
  email?: string;
}

export function domain(user: User): string {
  return user.email ?? "";
}
