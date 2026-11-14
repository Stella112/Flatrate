//! Flatrate private auction, run inside an OutLayer enclave.
//!
//! Bidders encrypt `{market_id, bidder, side, qty, limit_cents}` to an X25519 public key with NaCl
//! `box` (tweetnacl `nacl.box`): ciphertext = ephemeral public key (32) || nonce (24) || box. The
//! matching secret key exists only inside the enclave: it is `sha256(KEY_DOMAIN || sig)`, where
//! `sig` is the enclave's ed25519 signature over `KEY_MESSAGE` with the OutLayer signing key
//! `auction` (`caller: "predecessor"`, `bind: "project"`). ed25519 signatures are deterministic,
//! so every run of this project called by the same contract derives the same key, and no run
//! called by anyone else can (the predecessor is part of the key). The module never outputs the
//! secret, a signature, or an unfilled bid.
//!
//! `clear` opens every bid, keeps the ones whose plaintext names this market and the bidder who
//! committed it and whose escrow covers `qty x limit`, runs the same uniform-price clearing as the
//! public auction (`flatrate_shared::clear_auction`) and returns only the filled bids.

use crypto_box::aead::Aead;
use crypto_box::{Nonce, PublicKey, SalsaBox, SecretKey};
use flatrate_shared::{self as math, AuctionBid, BidSide};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Message the enclave signs to derive its X25519 secret. Composed by the module, never input.
pub const KEY_MESSAGE: &[u8] = b"flatrate-auction-x25519-v1";
/// Domain prefix for hashing the signature into the X25519 secret.
pub const KEY_DOMAIN: &[u8] = b"flatrate-auction-x25519-secret-v1:";
/// Largest ciphertext accepted (base64 characters). A bid plaintext is under 200 bytes.
pub const MAX_CIPHERTEXT_B64: usize = 1024;

const EPH_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const TAG_LEN: usize = 16;

/// X25519 secret key bytes from the enclave's signature over `KEY_MESSAGE`.
pub fn secret_from_signature(sig: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(KEY_DOMAIN);
    h.update(sig);
    h.finalize().into()
}

/// Hex of the X25519 public key for a secret.
pub fn public_key_hex(secret: &[u8; 32]) -> String {
    let sk = SecretKey::from(*secret);
    hex::encode(sk.public_key().as_bytes())
}

/// Opens one sealed bid. `None` for anything malformed or not sealed to this key.
pub fn open(secret: &[u8; 32], ciphertext_b64: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    if ciphertext_b64.len() > MAX_CIPHERTEXT_B64 {
        return None;
    }
    let raw = base64::engine::general_purpose::STANDARD.decode(ciphertext_b64).ok()?;
    if raw.len() < EPH_LEN + NONCE_LEN + TAG_LEN {
        return None;
    }
    let eph: [u8; 32] = raw[..EPH_LEN].try_into().ok()?;
    let nonce = Nonce::from_slice(&raw[EPH_LEN..EPH_LEN + NONCE_LEN]);
    let sk = SecretKey::from(*secret);
    SalsaBox::new(&PublicKey::from(eph), &sk).decrypt(nonce, &raw[EPH_LEN + NONCE_LEN..]).ok()
}

/// One committed bid as the contract sends it.
#[derive(Deserialize, Clone, Debug)]
pub struct SealedBid {
    pub index: u32,
    pub bidder: String,
    /// Escrow in fUSD micro-units, decimal string.
    pub escrow: String,
    /// base64(ephemeral public key || nonce || box).
    pub ciphertext: String,
}

#[derive(Deserialize, Debug)]
pub struct Input {
    pub op: String,
    #[serde(default)]
    pub market_id: Option<String>,
    #[serde(default)]
    pub bids: Vec<SealedBid>,
}

/// What a bidder encrypts.
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Plain {
    pub market_id: String,
    pub bidder: String,
    pub side: String,
    pub qty: u64,
    pub limit_cents: u8,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct FillOut {
    pub index: u32,
    pub side: String,
    pub filled_qty: u64,
}

#[derive(Serialize, Debug)]
pub struct ClearOutput {
    pub op: String,
    pub market_id: String,
    pub price_cents: Option<u8>,
    pub matched: u64,
    /// Filled bids only, by commit index.
    pub fills: Vec<FillOut>,
    /// Bids that opened and passed every check.
    pub valid: u32,
    /// Bids that did not open or failed a check (refunded in full by the contract).
    pub invalid: u32,
}

/// Opens and checks every bid, then clears. Bids that fail are left out (full refund).
pub fn clear(secret: &[u8; 32], market_id: &str, bids: &[SealedBid]) -> ClearOutput {
    let mut auction: Vec<AuctionBid> = Vec::new();
    let mut index_of: Vec<u32> = Vec::new();
    let mut invalid = 0u32;
    for b in bids {
        let ok = (|| {
            let escrow: u128 = b.escrow.parse().ok()?;
            let plain: Plain = serde_json::from_slice(&open(secret, &b.ciphertext)?).ok()?;
            if plain.market_id != market_id || plain.bidder != b.bidder {
                return None;
            }
            let side = match plain.side.as_str() {
                "Long" => BidSide::Long,
                "Short" => BidSide::Short,
                _ => return None,
            };
            let bid = AuctionBid { id: b.index as u64, side, qty: plain.qty, limit_cents: plain.limit_cents, escrow_micros: escrow };
            bid.is_valid().then_some(bid)
        })();
        match ok {
            Some(bid) => {
                index_of.push(b.index);
                auction.push(bid);
            }
            None => invalid += 1,
        }
    }
    let c = math::clear_auction(&auction);
    let fills = c
        .fills
        .iter()
        .filter(|f| f.filled_qty > 0)
        .map(|f| FillOut {
            index: f.id as u32,
            side: match f.side {
                BidSide::Long => "Long".to_string(),
                BidSide::Short => "Short".to_string(),
            },
            filled_qty: f.filled_qty,
        })
        .collect();
    ClearOutput {
        op: "clear".to_string(),
        market_id: market_id.to_string(),
        price_cents: c.price_cents,
        matched: c.matched,
        fills,
        valid: index_of.len() as u32,
        invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use crypto_box::aead::{AeadCore, OsRng};

    /// What the browser does: NaCl box from a fresh ephemeral key to the enclave key.
    fn seal(enclave_pk_hex: &str, plain: &Plain) -> String {
        let pk = PublicKey::from(<[u8; 32]>::try_from(hex::decode(enclave_pk_hex).unwrap()).unwrap());
        let eph = SecretKey::generate(&mut OsRng);
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let ct = SalsaBox::new(&pk, &eph).encrypt(&nonce, serde_json::to_vec(plain).unwrap().as_slice()).unwrap();
        let mut out = eph.public_key().as_bytes().to_vec();
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ct);
        base64::engine::general_purpose::STANDARD.encode(out)
    }

    fn bid(index: u32, bidder: &str, escrow: u128, ct: String) -> SealedBid {
        SealedBid { index, bidder: bidder.into(), escrow: escrow.to_string(), ciphertext: ct }
    }

    fn plain(m: &str, who: &str, side: &str, qty: u64, limit: u8) -> Plain {
        Plain { market_id: m.into(), bidder: who.into(), side: side.into(), qty, limit_cents: limit }
    }

    #[test]
    fn same_signature_same_key_other_signature_other_key() {
        let a = secret_from_signature(&[7u8; 64]);
        assert_eq!(a, secret_from_signature(&[7u8; 64]));
        assert_ne!(a, secret_from_signature(&[8u8; 64]));
        assert_eq!(public_key_hex(&a).len(), 64);
    }

    #[test]
    fn two_bids_match_at_one_price_and_unfilled_details_stay_out() {
        let sk = secret_from_signature(&[1u8; 64]);
        let pk = public_key_hex(&sk);
        let bids = vec![
            bid(0, "alice.testnet", 6_000_000, seal(&pk, &plain("20", "alice.testnet", "Long", 10, 55))),
            bid(1, "bob.testnet", 5_000_000, seal(&pk, &plain("20", "bob.testnet", "Short", 10, 50))),
            // Never matches (Long limit 5c): must not appear in the output.
            bid(2, "carol.testnet", 1_000_000, seal(&pk, &plain("20", "carol.testnet", "Long", 10, 5))),
        ];
        let out = clear(&sk, "20", &bids);
        assert_eq!(out.price_cents, Some(50));
        assert_eq!(out.matched, 10);
        assert_eq!(out.valid, 3);
        assert_eq!(out.invalid, 0);
        assert_eq!(
            out.fills,
            vec![
                FillOut { index: 0, side: "Long".into(), filled_qty: 10 },
                FillOut { index: 1, side: "Short".into(), filled_qty: 10 }
            ]
        );
        let text = serde_json::to_string(&out).unwrap();
        assert!(!text.contains("carol") && !text.contains("55") && !text.contains("\"limit"));
    }

    #[test]
    fn wrong_key_copied_ciphertext_wrong_market_and_short_escrow_are_invalid() {
        let sk = secret_from_signature(&[1u8; 64]);
        let pk = public_key_hex(&sk);
        let other_pk = public_key_hex(&secret_from_signature(&[2u8; 64]));
        let alice = seal(&pk, &plain("20", "alice.testnet", "Long", 10, 55));
        let bids = vec![
            // Sealed to another key.
            bid(0, "alice.testnet", 6_000_000, seal(&other_pk, &plain("20", "alice.testnet", "Long", 10, 55))),
            // Alice's ciphertext committed again by mallory: the plaintext names alice.
            bid(1, "mallory.testnet", 6_000_000, alice),
            // Another market's bid.
            bid(2, "bob.testnet", 6_000_000, seal(&pk, &plain("19", "bob.testnet", "Short", 10, 50))),
            // Escrow below qty x limit (10 x 55c = 5.5 fUSD).
            bid(3, "dan.testnet", 5_000_000, seal(&pk, &plain("20", "dan.testnet", "Long", 10, 55))),
            // Not base64.
            bid(4, "eve.testnet", 6_000_000, "%%%".into()),
        ];
        let out = clear(&sk, "20", &bids);
        assert_eq!(out.valid, 0);
        assert_eq!(out.invalid, 5);
        assert_eq!(out.price_cents, None);
        assert!(out.fills.is_empty());
    }

    /// A bid sealed in Node by tweetnacl (the site's and `scripts/seal-bid.mjs`'s library) opens
    /// here, and both sides derive the same public key from the same secret.
    #[test]
    fn opens_a_bid_sealed_by_tweetnacl() {
        let f: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/enclave/tweetnacl-sealed-bid.json")).unwrap();
        let sk = secret_from_signature(&[f["signature_byte"].as_u64().unwrap() as u8; 64]);
        assert_eq!(public_key_hex(&sk), f["x25519_pubkey"].as_str().unwrap());
        let plain: Plain = serde_json::from_slice(&open(&sk, f["ciphertext"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!((plain.market_id.as_str(), plain.bidder.as_str(), plain.side.as_str(), plain.qty, plain.limit_cents), ("20", "alice.testnet", "Long", 10, 55));
        let out = clear(&sk, "20", &[bid(0, "alice.testnet", 5_500_000, f["ciphertext"].as_str().unwrap().into())]);
        assert_eq!((out.valid, out.invalid), (1, 0));
    }

    #[test]
    fn partial_fill_follows_the_public_rule() {
        let sk = secret_from_signature(&[3u8; 64]);
        let pk = public_key_hex(&sk);
        let bids = vec![
            bid(0, "a.testnet", 10_000_000, seal(&pk, &plain("21", "a.testnet", "Long", 15, 60))),
            bid(1, "b.testnet", 5_000_000, seal(&pk, &plain("21", "b.testnet", "Short", 10, 50))),
        ];
        let out = clear(&sk, "21", &bids);
        assert_eq!(out.matched, 10);
        assert_eq!(out.fills[0], FillOut { index: 0, side: "Long".into(), filled_qty: 10 });
    }
}
