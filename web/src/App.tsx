import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { EXPLORER_ACCOUNT, FLATRATE_ID, FUSD_ID, INDEX, INDEXES, isScriptedDemo } from "./config";
import { accountExists, call, forgetGuest, loadOrCreateGuest, nearBalance, sponsor, view, walletUser, type Guest } from "./near";
import { listWallets, signIn, signOut, signedInWallet, type WalletChoice } from "./wallet";
import { near, shortAcct, units } from "./format";
import { demoRecordedAt, demoSteps } from "./demo";
import { TxView, useAction, useAsync } from "./ui";
import { MarketsPage } from "./pages/MarketsPage";
import { MarketPage } from "./pages/MarketPage";
import { NotesPage } from "./pages/NotesPage";
import { ProofPage } from "./pages/ProofPage";
import { PayPage } from "./pages/PayPage";

type Ctx = {
  guest: Guest;
  demo: boolean;
  step: number;
  setStep: (n: number) => void;
  funded: boolean;
  refreshWallet: () => void;
  setUser: (g: Guest | null) => void;
};
const AppCtx = createContext<Ctx | null>(null);
export const useApp = () => useContext(AppCtx)!;

function useRoute(): string[] {
  const [hash, setHash] = useState(window.location.hash);
  useEffect(() => {
    const on = () => setHash(window.location.hash);
    window.addEventListener("hashchange", on);
    return () => window.removeEventListener("hashchange", on);
  }, []);
  return hash.replace(/^#\/?/, "").split("/").filter(Boolean);
}

/** Keeps `?demo=scripted` on every link in demo mode. */
export const href = (path: string) =>
  `${isScriptedDemo() ? "?demo=scripted" : INDEX.key !== "h100" ? `?index=${INDEX.key}` : ""}#${path}`;

/** Switches between the per-index contracts (a full page load: every view reads FLATRATE_ID). */
function IndexSwitch() {
  if (isScriptedDemo()) return null;
  return (
    <div className="index-switch" role="group" aria-label="Index">
      {INDEXES.map((i) => (
        <a key={i.key} href={i.key === "h100" ? "/#/" : `/?index=${i.key}#/`} className={i.key === INDEX.key ? "active" : ""}>
          {i.name}
        </a>
      ))}
    </div>
  );
}

export function DemoBadge() {
  return (
    <div className="demo-badge" role="status">
      Scripted demo · replaying recorded testnet market 2 ({demoRecordedAt.slice(0, 10)}) · no transactions are sent ·{" "}
      <a href="/">leave demo</a>
    </div>
  );
}

const LOGIN_KEY = "flatrate.login";
const remember = (v: string | null) => {
  try {
    if (v) localStorage.setItem(LOGIN_KEY, v);
    else localStorage.removeItem(LOGIN_KEY);
  } catch {
    /* ignore */
  }
};

function WalletLogin() {
  const { setUser } = useApp();
  const [open, setOpen] = useState(false);
  const [choices, setChoices] = useState<WalletChoice[] | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <div className="row">
      {!open ? (
        <button
          className="ghost"
          onClick={async () => {
            setOpen(true);
            setMsg(null);
            try {
              setChoices(await listWallets());
            } catch (e) {
              setMsg(String((e as Error).message || e));
            }
          }}
        >
          Sign in with a NEAR wallet
        </button>
      ) : (
        <>
          <small className="muted">Testnet wallets:</small>
          {!choices && !msg && <small className="muted">loading…</small>}
          {choices?.map((c) => (
            <button
              key={c.id}
              disabled={busy || !c.available}
              title={c.available ? "" : "not available in this browser"}
              onClick={async () => {
                setBusy(true);
                setMsg(null);
                try {
                  const id = await signIn(c.id);
                  const w = await signedInWallet();
                  if (id && w) {
                    remember("wallet");
                    setUser(walletUser(w.accountId, w.wallet, c.name));
                  } else setMsg("The wallet did not return an account (a redirect wallet finishes after it sends you back).");
                } catch (e) {
                  setMsg(String((e as Error).message || e));
                } finally {
                  setBusy(false);
                }
              }}
            >
              {c.name}
            </button>
          ))}
          <button className="ghost" onClick={() => setOpen(false)}>
            Cancel
          </button>
        </>
      )}
      {msg && <small className="err">{msg}</small>}
    </div>
  );
}

function WalletPanel() {
  const { guest, demo, funded, refreshWallet, setUser } = useApp();
  const isWallet = !!guest.wallet;
  const bal = useAsync(
    async () => {
      if (demo) return null;
      if (!(await accountExists(guest.accountId))) return { exists: false as const };
      const [n, f] = await Promise.all([
        nearBalance(guest.accountId),
        view<string>(FUSD_ID, "ft_balance_of", { account_id: guest.accountId }),
      ]);
      return { exists: true as const, near: n, fusd: f };
    },
    [guest.accountId, demo, funded],
    15000,
  );
  const [sponsorMsg, setSponsorMsg] = useState<string | null>(null);
  const [sponsoring, setSponsoring] = useState(false);
  const [sponsored, setSponsored] = useState(false);
  useEffect(() => setSponsored(false), [guest.accountId]); // a new guest key starts unfunded
  const faucet = useAction();

  if (demo) return <div className="wallet mono">guest: (none in scripted demo)</div>;
  const b = bal.data;
  return (
    <div className="wallet">
      <div>
        <span className="muted">{isWallet ? `Wallet account (${guest.walletName}) ` : "Guest account "}</span>
        <a className="mono" href={EXPLORER_ACCOUNT + guest.accountId} target="_blank" rel="noreferrer" title={guest.accountId}>
          {shortAcct(guest.accountId)}
        </a>
        {b?.exists && (
          <span className="mono">
            {" "}
            · {near(b.near)} NEAR · {units(b.fusd)} fUSD
          </span>
        )}
      </div>
      {!isWallet && b && !b.exists && !sponsored && (
        <div className="row">
          <button
            disabled={sponsoring}
            onClick={async () => {
              setSponsoring(true);
              setSponsorMsg(null);
              try {
                const r = await sponsor(guest.accountId);
                setSponsorMsg(r.message);
                if (r.ok) {
                  // Hide the button at once; the balance shows when a final-state read sees the account.
                  setSponsored(true);
                  for (const ms of [2500, 6000, 12000]) setTimeout(() => refreshWallet(), ms);
                }
              } finally {
                setSponsoring(false);
              }
            }}
          >
            {sponsoring ? "Creating…" : "Create guest account"}
          </button>
          <small className="muted">Funded with 0.1 testnet NEAR and 100 fUSD by the sponsor. The key stays in this browser.</small>
        </div>
      )}
      {b?.exists && (
        <div className="row">
          <button
            disabled={faucet.busy}
            onClick={() => faucet.run(() => call(guest, FUSD_ID, "faucet", { amount: "100000000" }, { gasTgas: 30 }), refreshWallet)}
          >
            {faucet.busy ? "…" : "Faucet: +100 fUSD"}
          </button>
          {isWallet ? (
            <button
              className="ghost"
              onClick={async () => {
                await signOut().catch(() => undefined);
                remember(null);
                setUser(null);
              }}
            >
              Sign out
            </button>
          ) : (
          <button
            className="ghost"
            onClick={() => {
              if (confirm("Forget this guest key? The account and its balances stay on testnet but this browser loses access.")) {
                forgetGuest();
                window.location.reload();
              }
            }}
          >
            New guest
          </button>
          )}
        </div>
      )}
      {!isWallet && <WalletLogin />}
      {sponsorMsg && <small className="mono">{sponsorMsg}</small>}
      {faucet.view}
      {bal.error && <small className="err">{bal.error}</small>}
      <TxView r={null} />
    </div>
  );
}

function Nav({ route }: { route: string[] }) {
  const items: [string, string][] = [
    ["Markets", "/"],
    ["My notes", "/notes"],
    ["Proof", "/proof"],
    ["Pay from any chain", "/pay"],
  ];
  const current = "/" + (route[0] && route[0] !== "market" ? route[0] : "");
  return (
    <nav className="nav">
      {items.map(([label, path]) => (
        <a key={path} href={href(path)} className={current === path ? "active" : ""}>
          {label}
        </a>
      ))}
    </nav>
  );
}

function DemoStepper() {
  const { step, setStep } = useApp();
  const s = demoSteps[step];
  return (
    <div className="stepper">
      <button disabled={step === 0} onClick={() => setStep(step - 1)}>
        ◀
      </button>
      <div className="stepper-body">
        <div>
          Step {step + 1} of {demoSteps.length}: <strong>{s.label}</strong>
        </div>
        <small className="muted">
          recorded tx{" "}
          <a className="mono" href={`https://testnet.nearblocks.io/txns/${s.hash}`} target="_blank" rel="noreferrer">
            {s.hash.slice(0, 10)}…
          </a>{" "}
          · {s.events.map((e) => e.event).join(", ")}
        </small>
      </div>
      <button disabled={step === demoSteps.length - 1} onClick={() => setStep(step + 1)}>
        ▶
      </button>
    </div>
  );
}

export function App() {
  const demo = isScriptedDemo();
  const localGuest = useMemo(() => loadOrCreateGuest(), []);
  const [user, setUser] = useState<Guest | null>(null);
  const guest = user ?? localGuest;
  // Restore a wallet session only if this browser signed in before (keeps the selector off the guest path).
  useEffect(() => {
    let wanted = false;
    try {
      wanted = localStorage.getItem(LOGIN_KEY) === "wallet";
    } catch {
      /* ignore */
    }
    if (!wanted || isScriptedDemo()) return;
    signedInWallet()
      .then(async (w) => {
        if (!w) return remember(null);
        const name = (await listWallets()).find((c) => c.id === w.wallet.id)?.name || w.wallet.id;
        setUser(walletUser(w.accountId, w.wallet, name));
      })
      .catch(() => undefined);
  }, []);
  const [step, setStep] = useState(0);
  const [funded, setFunded] = useState(false);
  const route = useRoute();
  const ctx: Ctx = { guest, demo, step, setStep, funded, refreshWallet: () => setFunded((f) => !f), setUser };

  let page: ReactNode;
  if (route[0] === "market" && route[1]) page = <MarketPage id={route[1]} />;
  else if (route[0] === "notes") page = <NotesPage />;
  else if (route[0] === "proof") page = <ProofPage />;
  else if (route[0] === "pay") page = <PayPage />;
  else page = <MarketsPage />;

  return (
    <AppCtx.Provider value={ctx}>
      {demo && <DemoBadge />}
      <header className="top">
        <a href={href("/")} className="brand">
          Flatrate <span className="tag">testnet</span>
        </a>
        <Nav route={route} />
      </header>
      <IndexSwitch />
      <WalletPanel />
      {demo && <DemoStepper />}
      <main>{page}</main>
      <footer className="foot muted">
        Contract{" "}
        <a className="mono" href={EXPLORER_ACCOUNT + FLATRATE_ID} target="_blank" rel="noreferrer">
          {FLATRATE_ID}
        </a>{" "}
        · test stablecoin fUSD · prices attested by OutLayer · built at the NEAR Hackathon ·{" "}
        {demo ? <a href="/">live mode</a> : <a href="?demo=scripted#/">scripted demo</a>}
      </footer>
    </AppCtx.Provider>
  );
}
