import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { EXPLORER_ACCOUNT, FLATRATE_ID, FUSD_ID, isScriptedDemo } from "./config";
import { accountExists, call, forgetGuest, loadOrCreateGuest, nearBalance, sponsor, view, type Guest } from "./near";
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
export const href = (path: string) => `${isScriptedDemo() ? "?demo=scripted" : ""}#${path}`;

export function DemoBadge() {
  return (
    <div className="demo-badge" role="status">
      Scripted demo · replaying recorded testnet market 2 ({demoRecordedAt.slice(0, 10)}) · no transactions are sent ·{" "}
      <a href="/">leave demo</a>
    </div>
  );
}

function WalletPanel() {
  const { guest, demo, funded, refreshWallet } = useApp();
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
  const faucet = useAction();

  if (demo) return <div className="wallet mono">guest: (none in scripted demo)</div>;
  const b = bal.data;
  return (
    <div className="wallet">
      <div>
        <span className="muted">Guest account </span>
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
      {b && !b.exists && (
        <div className="row">
          <button
            disabled={sponsoring}
            onClick={async () => {
              setSponsoring(true);
              setSponsorMsg(null);
              try {
                const r = await sponsor(guest.accountId);
                setSponsorMsg(r.message);
                if (r.ok) setTimeout(() => refreshWallet(), 2500);
              } finally {
                setSponsoring(false);
              }
            }}
          >
            {sponsoring ? "Creating…" : "Create guest account"}
          </button>
          <small className="muted">Funded with 0.5 testnet NEAR and 100 fUSD by the sponsor. The key stays in this browser.</small>
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
        </div>
      )}
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
  const guest = useMemo(() => loadOrCreateGuest(), []);
  const [step, setStep] = useState(0);
  const [funded, setFunded] = useState(false);
  const route = useRoute();
  const ctx: Ctx = { guest, demo, step, setStep, funded, refreshWallet: () => setFunded((f) => !f) };

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
