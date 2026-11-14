// Private auction (extra): seal a bid to the enclave's X25519 key with NaCl box (tweetnacl), the
// format auction-enclave/src/lib.rs opens: base64(ephemeral public key || nonce || box).
// The plaintext never leaves the browser except as this ciphertext.
import nacl from "tweetnacl";
import type { Side } from "./types";

export type SealedPlain = { market_id: string; bidder: string; side: Side; qty: number; limit_cents: number };

function hexToBytes(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(2 * i, 2 * i + 2), 16);
  return out;
}

function toBase64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

export function sealBid(enclavePubkeyHex: string, plain: SealedPlain): string {
  if (!/^[0-9a-f]{64}$/i.test(enclavePubkeyHex)) throw new Error("bad enclave key");
  const eph = nacl.box.keyPair();
  const nonce = nacl.randomBytes(nacl.box.nonceLength);
  const box = nacl.box(new TextEncoder().encode(JSON.stringify(plain)), nonce, hexToBytes(enclavePubkeyHex), eph.secretKey);
  const out = new Uint8Array(eph.publicKey.length + nonce.length + box.length);
  out.set(eph.publicKey, 0);
  out.set(nonce, eph.publicKey.length);
  out.set(box, eph.publicKey.length + nonce.length);
  return toBase64(out);
}
