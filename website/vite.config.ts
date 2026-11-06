import { watch } from "node:fs";
import { join } from "node:path";
import { defineConfig, type Plugin } from "vite";
import { out, render } from "./render.ts";

/** In development, pages are rendered again when a document changes. */
function watchDocs(): Plugin {
  return {
    name: "peras-docs",
    configureServer(server) {
      const docs = join(import.meta.dirname, "..", "docs");
      let pending: ReturnType<typeof setTimeout> | undefined;
      watch(docs, { recursive: true }, () => {
        clearTimeout(pending);
        pending = setTimeout(() => {
          render()
            .then(() => {
              server.ws.send({ type: "full-reload" });
            })
            .catch((error: unknown) => {
              server.config.logger.error(String(error));
            });
        }, 100);
      });
    },
  };
}

export default defineConfig(async () => {
  const pages = await render();
  return {
    root: out,
    // Relative asset URLs: the site works at any address, such as /Peras/ on GitHub Pages.
    base: "./",
    build: {
      outDir: join(import.meta.dirname, "dist"),
      emptyOutDir: true,
      rolldownOptions: { input: pages },
    },
    plugins: [watchDocs()],
  };
});
