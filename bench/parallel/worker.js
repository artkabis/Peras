// A worker of the TypeScript versions on threads (decision 0031): it imports
// a module once, then runs its function over the rows it is given, on views
// of the page's SharedArrayBuffers, and answers when done. What a user
// writes to split a frame between Web Workers.
let module;
onmessage = async ({ data }) => {
  module ??= await import(data.url);
  const views = data.buffers.map(([kind, buffer]) => new globalThis[kind](buffer));
  module[data.name](...views, ...data.args, data.top, data.bottom);
  postMessage(0);
};
