// The declarations TypeScript trusts, as a package's .d.ts would give them.
export declare function score(): number;
export declare function parseId(text: string): number;
export declare function fetchUser(id: number): Promise<{ id: number; name: string }>;
export declare function parseUser(text: string): { id: number; name: string };
