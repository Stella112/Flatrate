#!/usr/bin/env node
// Chain-signature demo (extra "cross-chain pay-in and payout", reduced; see BLOCKERS.md B13).
//
// A NEAR testnet account controls an address on Base Sepolia through NEAR's MPC signer
// (v1.signer-prod.testnet). The address is derived from (NEAR account, path); only that NEAR
// account can ask the MPC network to sign for it.
//   - "pay-in":  anyone can send test ETH to the derived address (shown by `address`);
//   - "payout":  the NEAR account signs and sends a Base Sepolia transfer from it (`send`).
// No fUSD or claim crosses chains: there is no bridge (CLAUDE.md: no bridges we write ourselves).
//
// Usage:
//   NEAR_ACCOUNT=holder.flatrate-maris.testnet node scripts/chainsig/sepolia.mjs address
//   NEAR_ACCOUNT=holder.flatrate-maris.testnet NEAR_KEY_FILE=~/.near-credentials/testnet/holder.flatrate-maris.testnet.json \
//     node scripts/chainsig/sepolia.mjs send <0x recipient> <amount in wei>
// Env: CHAINSIG_PATH (default "flatrate-payout"), BASE_SEPOLIA_RPC (default https://sepolia.base.org).

import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { chainAdapters, contracts } from "chainsig.js";
import { createPublicClient, http, formatEther } from "viem";
import { baseSepolia } from "viem/chains";
import { Account } from "@near-js/accounts";
import { JsonRpcProvider } from "@near-js/providers";
import { KeyPairSigner } from "@near-js/signers";
import { KeyPair } from "@near-js/crypto";

const ACCOUNT = process.env.NEAR_ACCOUNT;
const PATH = process.env.CHAINSIG_PATH || "flatrate-payout";
const MPC = "v1.signer-prod.testnet";
const [cmd, to, wei] = process.argv.slice(2);
if (!ACCOUNT || !["address", "send"].includes(cmd)) {
  console.error("usage: NEAR_ACCOUNT=<id> node scripts/chainsig/sepolia.mjs address | send <to> <wei>");
  process.exit(2);
}

const contract = new contracts.ChainSignatureContract({ networkId: "testnet", contractId: MPC });
const publicClient = createPublicClient({ chain: baseSepolia, transport: http(process.env.BASE_SEPOLIA_RPC || "https://sepolia.base.org") });
const evm = new chainAdapters.evm.EVM({ publicClient, contract });

const { address } = await evm.deriveAddressAndPublicKey(ACCOUNT, PATH);
const { balance } = await evm.getBalance(address);
console.log(`NEAR account ${ACCOUNT}, path "${PATH}" -> Base Sepolia ${address}`);
console.log(`balance ${formatEther(balance)} ETH (https://sepolia.basescan.org/address/${address})`);
if (cmd === "address") process.exit(0);

if (!/^0x[0-9a-fA-F]{40}$/.test(to || "") || !/^[0-9]+$/.test(wei || "")) {
  console.error("send needs <0x recipient> <amount in wei>");
  process.exit(2);
}
const keyFile = (process.env.NEAR_KEY_FILE || `~/.near-credentials/testnet/${ACCOUNT}.json`).replace(/^~(?=\/|\\)/, homedir());
const { private_key } = JSON.parse(readFileSync(keyFile, "utf8"));
const provider = new JsonRpcProvider({ url: process.env.NEAR_RPC || "https://rpc.testnet.fastnear.com" });
const near = new Account(ACCOUNT, provider, new KeyPairSigner(KeyPair.fromString(private_key)));
const signerAccount = {
  accountId: ACCOUNT,
  signAndSendTransactions: async ({ transactions }) => {
    const out = [];
    for (const t of transactions) {
      const r = await near.signAndSendTransaction({ receiverId: t.receiverId, actions: t.actions });
      console.log(`MPC sign request on NEAR: https://testnet.nearblocks.io/txns/${r.transaction.hash}`);
      out.push(r);
    }
    return out;
  },
};

const { transaction, hashesToSign } = await evm.prepareTransactionForSigning({ from: address, to, value: BigInt(wei) });
const rsv = await contract.sign({ payloads: hashesToSign, path: PATH, keyType: "Ecdsa", signerAccount });
const signed = evm.finalizeTransactionSigning({ transaction, rsvSignatures: rsv });
const { hash } = await evm.broadcastTx(signed);
console.log(`Base Sepolia tx: https://sepolia.basescan.org/tx/${hash}`);
const receipt = await publicClient.waitForTransactionReceipt({ hash });
console.log(`status ${receipt.status}, block ${receipt.blockNumber}, from ${receipt.from}`);
