// A library the cases call, as an npm package would be. Some of its
// functions do not do what their declarations say: that is the bug.
export const score = () => "42";
export const parseId = (text) => Number(JSON.parse(text).id);
export const fetchUser = async (id) => ({ id, name: `user ${String(id)}` });
export const parseUser = (text) => JSON.parse(text);
