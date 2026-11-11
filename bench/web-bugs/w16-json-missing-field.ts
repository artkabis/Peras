// Data from the server without a field the code expects.
interface User {
  id: number;
  name: string;
}

export function main(): string {
  const user = JSON.parse('{"id": 7}') as User;
  const greeting = `Hello ${user.name.toUpperCase()}`;
  return greeting;
}
