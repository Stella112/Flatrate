#!/usr/bin/env node
// Writes a new ed25519 key file in near-cli's access-key-file format and prints only the public key.
// Usage: node scripts/agent/new-key.mjs <account_id> <out-file>
import { generateKeyPairSync } from "node:crypto";
import { writeFileSync, existsSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const [accountId, out] = process.argv.slice(2);
if (!accountId || !out) { console.error("usage: new-key.mjs <account_id> <out-file>"); process.exit(2); }
if (existsSync(out)) { console.error(`${out} exists; not overwriting`); process.exit(1); }

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
function b58(bytes) {
  let n = BigInt("0x" + (Buffer.from(bytes).toString("hex") || "0"));
  let s = "";
  while (n > 0n) { s = B58[Number(n % 58n)] + s; n /= 58n; }
  for (const b of bytes) { if (b === 0) s = "1" + s; else break; }
  return s;
}

const { publicKey, privateKey } = generateKeyPairSync("ed25519");
const pub = publicKey.export({ format: "jwk" }).x;
const priv = privateKey.export({ format: "jwk" }).d;
const pubBytes = Buffer.from(pub, "base64url");
const seed = Buffer.from(priv, "base64url");
const file = {
  account_id: accountId,
  public_key: "ed25519:" + b58(pubBytes),
  private_key: "ed25519:" + b58(Buffer.concat([seed, pubBytes])),
};
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(file), { mode: 0o600 });
console.log(file.public_key);
