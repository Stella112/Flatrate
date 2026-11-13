// Wallet login (extra 6): NEAR Wallet Selector with Meteor, MyNearWallet and Intear on testnet.
// Loaded lazily so the guest flow does not pay for it. No modal-ui package: it bundles React 18,
// and this site uses React 19, so the picker is our own list (WalletPanel in App.tsx).
import type { WalletSelector, Wallet, ModuleState, InjectedWallet } from "@near-wallet-selector/core";
import { FLATRATE_ID } from "./config";

let selectorP: Promise<WalletSelector> | null = null;

export function getSelector(): Promise<WalletSelector> {
  if (!selectorP) {
    selectorP = (async () => {
      // Some wallet modules read the global Buffer (Node API); provide the npm polyfill first.
      const g = globalThis as unknown as { Buffer?: unknown; global?: unknown; process?: unknown };
      if (!g.global) g.global = globalThis; // Meteor's SDK reads Node's `global`
      // ...and `process.env.DEFAULT_FINALITY` / `process.nextTick` (its bundled @near-js/accounts).
      if (!g.process) g.process = { env: {}, nextTick: (f: (...a: unknown[]) => void, ...a: unknown[]) => queueMicrotask(() => f(...a)) };
      if (!g.Buffer) g.Buffer = (await import("buffer")).Buffer;
      const [{ setupWalletSelector }, { setupMeteorWallet }, { setupMyNearWallet }, { setupIntearWallet }] = await Promise.all([
        import("@near-wallet-selector/core"),
        import("@near-wallet-selector/meteor-wallet"),
        import("@near-wallet-selector/my-near-wallet"),
        import("@near-wallet-selector/intear-wallet"),
      ]);
      return setupWalletSelector({
        network: "testnet",
        modules: [setupMeteorWallet(), setupMyNearWallet(), setupIntearWallet()],
      });
    })();
  }
  return selectorP;
}

export type WalletChoice = { id: string; name: string; available: boolean };

export async function listWallets(): Promise<WalletChoice[]> {
  const s = await getSelector();
  return s.store.getState().modules.map((m: ModuleState) => ({
    id: m.id,
    name: m.metadata.name,
    available: m.metadata.available !== false,
  }));
}

/** The signed-in wallet account, if any (restored by the selector from its own storage). */
export async function signedInWallet(): Promise<{ accountId: string; wallet: Wallet } | null> {
  const s = await getSelector();
  const st = s.store.getState();
  const acct = st.accounts.find((a) => a.active) || st.accounts[0];
  if (!acct || !st.selectedWalletId) return null;
  return { accountId: acct.accountId, wallet: await s.wallet() };
}

/** Signs in with a function-call key for the flatrate contract (calls without a deposit need no wallet prompt). */
export async function signIn(id: string): Promise<string | null> {
  const s = await getSelector();
  // All three modules are injected wallets (see their setup* return types).
  const w = await s.wallet<InjectedWallet>(id);
  const accounts = await w.signIn({ contractId: FLATRATE_ID, methodNames: [] });
  return accounts[0]?.accountId ?? null;
}

export async function signOut(): Promise<void> {
  const s = await getSelector();
  if (!s.isSignedIn()) return;
  await (await s.wallet()).signOut();
}
