import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import type { IncomingMessage, ServerResponse } from "node:http";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

/**
 * Local development only: serves api/*.js (the Vercel functions) under /api so the guest flow
 * can be tried with `npm run dev`. On Vercel the platform runs those files itself.
 */
function localApi(): Plugin {
  return {
    name: "flatrate-local-api",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use(async (req: IncomingMessage, res: ServerResponse, next) => {
        const m = req.url?.match(/^\/api\/([a-z]+)(?:\?.*)?$/);
        if (!m) return next();
        const chunks: Buffer[] = [];
        for await (const c of req) chunks.push(c as Buffer);
        const raw = Buffer.concat(chunks).toString();
        const vreq = Object.assign(req, { body: raw ? JSON.parse(raw) : {} });
        const vres = Object.assign(res, {
          status(code: number) {
            res.statusCode = code;
            return vres;
          },
          json(obj: unknown) {
            res.setHeader("Content-Type", "application/json");
            res.end(JSON.stringify(obj));
            return vres;
          },
        });
        try {
          const file = pathToFileURL(resolve(__dirname, `api/${m[1]}.js`)).href + `?t=${Date.now()}`;
          const mod = await import(/* @vite-ignore */ file);
          await mod.default(vreq, vres);
        } catch (e) {
          res.statusCode = 500;
          res.end(JSON.stringify({ error: String(e) }));
        }
      });
    },
  };
}

export default defineConfig({
  plugins: [react(), localApi()],
  // Wallet Selector's Meteor module needs Node's `buffer` in the browser: use the npm package.
  resolve: { alias: { buffer: "buffer/" } },
  // Recorded fixtures (demo market, 1Click quote) are imported from ../fixtures.
  server: { fs: { allow: [".."] } },
});
