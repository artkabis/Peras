// The JavaScript a managed module needs to read JSON today (j0): one
// function per type read; Peras checks what it gives against the type.
export const parseSettings = (text) => JSON.parse(text);
export const parseRecords = (text) => JSON.parse(text);
export const stringifySummary = (s) => JSON.stringify(s);
