#!/usr/bin/env node
// Local development: starts the Vite dev server with the sponsor account's key taken from the
// near-cli legacy keychain (~/.near-credentials/testnet/<SPONSOR_ACCOUNT_ID>.json), so api/sponsor
// works locally. The key is passed to the child process environment only and never printed.
// Usage: SPONSOR_ACCOUNT_ID=flatrate-sponsor.testnet node scripts/dev-web.mjs
import { readFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { spawn } from "node:child_process";

const id = process.env.SPONSOR_ACCOUNT_ID || "flatrate-sponsor.testnet";
const file = join(homedir(), ".near-credentials", "testnet", `${id}.json`);
const env = { ...process.env };
if (existsSync(file)) {
  const j = JSON.parse(readFileSync(file, "utf8"));
  env.SPONSOR_ACCOUNT_ID = id;
  env.SPONSOR_PRIVATE_KEY = j.private_key;
  console.log(`sponsor: ${id} (key loaded from the local keychain)`);
} else {
  console.log(`sponsor: ${file} not found; /api/sponsor will answer 503`);
}
const child = spawn("npm", ["run", "dev", "--", "--port", process.env.PORT || "5173", "--strictPort"], {
  cwd: new URL("../web/", import.meta.url),
  env,
  stdio: "inherit",
  shell: true,
});
child.on("exit", (code) => process.exit(code ?? 0));
