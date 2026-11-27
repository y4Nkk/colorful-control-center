import { createServer } from "vite";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const server = await createServer({ root, cacheDir: `${root}/target/ui-check/vite-cache`,
  server: { host: "127.0.0.1", port: 1450, strictPort: true, watch: null, hmr: false } });
await server.listen();
for (const signal of ["SIGINT", "SIGTERM"]) process.once(signal, async () => { await server.close(); process.exit(0); });
