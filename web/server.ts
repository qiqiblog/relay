// Bun-native dev server with HMR + API proxy to relay-master.
// Run with: bun --hot run server.ts
import index from "./src/index.html";

const PORT = Number(process.env.WEB_PORT ?? 5173);
const MASTER = process.env.MASTER_URL ?? "http://127.0.0.1:7080";

const server = Bun.serve({
  port: PORT,
  development: { hmr: true, console: true },
  routes: {
    "/api/*": async (req) => {
      const u = new URL(req.url);
      const target = MASTER + u.pathname + u.search;
      try {
        return await fetch(target, {
          method: req.method,
          headers: req.headers,
          body: req.method === "GET" || req.method === "HEAD" ? undefined : req.body,
        });
      } catch (err) {
        return new Response(`master unreachable: ${err}`, { status: 502 });
      }
    },
    "/*": index,
  },
});

console.log(`relay web dev → http://localhost:${server.port}  (proxy /api → ${MASTER})`);
