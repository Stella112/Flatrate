import { useRef, useState } from "react";
import { href, useApp } from "../App";
import { PriceChart } from "../Chart";
import { ONE } from "../config";
import { getBids, getMarket, getSlotInfo } from "../data";
import { bidHash, callFlat, flat, randomSalt, transferCall, type Guest } from "../near";
import { longPayout, parseUnits, phase, shortAcct, statusLabel, timeLeft, units, usd, utc } from "../format";
import type { Market, Side } from "../types";
import { Card, Field, useAction, useAsync, useTick } from "../ui";

type SavedBid = { market: string; side: Side; qty: string; limit: number; salt: string; escrow: string; hash: string; revealed?: boolean };

const bidsKey = (g: Guest) => `flatrate.bids.${g.accountId}`;
function loadBids(g: Guest): SavedBid[] {
  try {
    return JSON.parse(localStorage.getItem(bidsKey(g)) || "[]");
  } catch {
    return [];
  }
}
function saveBids(g: Guest, b: SavedBid[]) {
  try {
    localStorage.setItem(bidsKey(g), JSON.stringify(b));
  } catch {
    /* storage blocked: the salt is still shown on screen for backup */
  }
}

export function MarketPage({ id }: { id: string }) {
  const { demo, step, guest, refreshWallet } = useApp();
  useTick(1000);
  const m = useAsync(() => getMarket(demo, step, id), [demo, step, id], demo ? undefined : 10000);
  const bids = useAsync(() => getBids(demo, step, id), [demo, step, id], demo ? undefined : 15000);
  const mine = useAsync(
    async () => {
      if (demo) return { long: "0", short: "0", refund: "0" };
      const [long, short, refund] = await Promise.all([
        flat<string>("claim_balance_of", { account_id: guest.accountId, market_id: id, side: "Long" }),
        flat<string>("claim_balance_of", { account_id: guest.accountId, market_id: id, side: "Short" }),
        flat<string>("refund_balance_of", { account_id: guest.accountId }),
      ]);
      return { long, short, refund };
    },
    [demo, id, guest.accountId],
    demo ? undefined : 10000,
  );
  // Keep the auction panel (and the clear_auction result) on screen after the market opens.
  const sawAuction = useRef(false);
  if (m.data?.status === "Auction") sawAuction.current = true;
  const reloadAll = () => {
    m.reload();
    bids.reload();
    mine.reload();
    refreshWallet();
  };

  if (m.error) return <Card title={`Market ${id}`}><p className="err">{m.error}</p></Card>;
  if (!m.data) return <Card title={`Market ${id}`}><p className="muted">{m.loading ? "Loading…" : "No such market."}</p></Card>;
  const mk = m.data;
  const ph = phase(mk);
  const lp = longPayout(mk.status);

  return (
    <>
      <p>
        <a href={href("/")}>← Markets</a>
      </p>
      <Card
        title={`Market ${mk.id} (${mk.template}): H100 per GPU-hour`}
        right={<span className={`pill pill-${statusLabel(mk.status).toLowerCase()}`}>{statusLabel(mk.status)}</span>}
      >
        <div className="grid4">
          <Stat k="Floor" v={usd(mk.floor_micros)} />
          <Stat k="Cap" v={usd(mk.cap_micros)} />
          <Stat k="Opening price (Long)" v={mk.opening_price_cents !== null ? `${mk.opening_price_cents}¢` : "—"} />
          <Stat k="Pairs / collateral" v={`${units(mk.pairs)} / ${units(mk.collateral)} fUSD`} />
          <Stat k="Commits close" v={utc(mk.auction_commit_end)} />
          <Stat k="Reveals close" v={utc(mk.auction_reveal_end)} />
          <Stat k="Period" v={`${utc(mk.period_start)} → ${utc(mk.period_end)}`} />
          <Stat
            k="Time slots"
            v={`${mk.filled_slots} of ${mk.num_slots} filled (need ${mk.min_filled_slots}), ${Number(BigInt(mk.slot_ns) / 60_000_000_000n)} min each; void after ${utc(mk.void_after)}`}
          />
        </div>
        {lp !== null && (
          <p>
            {mk.settled_average_micros && <>Settled average {usd(mk.settled_average_micros, 4)} · </>}
            <strong>Payout per claim:</strong> Long {usd(lp, 6)} · Short {usd(1_000_000 - lp, 6)}
            {mk.status === "Void" && " (void: too few observations)"}
          </p>
        )}
        <p className="muted small">
          Oracle version (wasm sha256) pinned for this market: <span className="mono break">{mk.oracle_version_key}</span> ·
          created by <span className="mono">{shortAcct(mk.created_by)}</span>
        </p>
      </Card>

      <Card title="Observed median price">
        <PriceChart obs={mk.observations} floor={Number(mk.floor_micros)} cap={Number(mk.cap_micros)} />
        <p className="muted small">
          Each point is one OutLayer run. See <a href={href("/proof")}>Proof</a> for request ids, transactions and per-source prices.
        </p>
      </Card>

      {(mk.status === "Auction" || sawAuction.current) && <AuctionCard mk={mk} ph={ph} onDone={reloadAll} />}

      <Card title="Sealed bids">
        {!bids.data?.length ? (
          <p className="muted">No bids.</p>
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>#</th>
                  <th>Bidder</th>
                  <th>Commitment</th>
                  <th>Escrow</th>
                  <th>Revealed</th>
                  <th>Filled</th>
                </tr>
              </thead>
              <tbody>
                {bids.data.map((b) => (
                  <tr key={b.index}>
                    <td>{b.index}</td>
                    <td className="mono">{shortAcct(b.bidder)}</td>
                    <td className="mono">{b.hash.slice(0, 12)}…</td>
                    <td>{units(b.escrow)}</td>
                    <td>{b.revealed ? `${b.revealed.side} ${b.revealed.qty} @ ${b.revealed.limit_cents}¢` : "sealed"}</td>
                    <td>{mk.status === "Auction" ? "—" : `${b.filled_qty} (paid ${units(b.cost)})`}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <PositionCard mk={mk} mine={mine.data} ph={ph} onDone={reloadAll} />
    </>
  );
}

const Stat = ({ k, v }: { k: string; v: string }) => (
  <div className="stat">
    <small className="muted">{k}</small>
    <div>{v}</div>
  </div>
);

function AuctionCard({ mk, ph, onDone }: { mk: Market; ph: string; onDone: () => void }) {
  const { guest, demo } = useApp();
  const [side, setSide] = useState<Side>("Long");
  const [qty, setQty] = useState("10");
  const [limit, setLimit] = useState("50");
  const [extra, setExtra] = useState("0");
  const [saved, setSaved] = useState<SavedBid[]>(() => loadBids(guest).filter((b) => b.market === mk.id));
  const act = useAction();

  const q = /^\d+$/.test(qty) ? BigInt(qty) : 0n;
  const l = Number(limit);
  const ex = parseUnits(extra) ?? 0n;
  const valid = q > 0n && Number.isInteger(l) && l >= 1 && l <= 99;
  const escrow = valid ? q * BigInt(l) * 10_000n + ex : 0n;

  const commit = () =>
    act.run(async () => {
      const salt = randomSalt();
      const hash = await bidHash(Number(mk.id), side, q, l, salt);
      const rec: SavedBid = { market: mk.id, side, qty: q.toString(), limit: l, salt, escrow: escrow.toString(), hash };
      // Save the salt before sending, so a crash after sending cannot lose it.
      const all = loadBids(guest).concat(rec);
      saveBids(guest, all);
      setSaved(all.filter((x) => x.market === mk.id));
      return transferCall(guest, escrow, { action: "commit_bid", market_id: mk.id, hash });
    }, onDone);

  const reveal = (b: SavedBid) =>
    act.run(async () => {
      const r = await callFlat(guest, "reveal_bid", { market_id: mk.id, side: b.side, qty: b.qty, limit_cents: b.limit, salt: b.salt }, { gasTgas: 30 });
      if (r.ok && !r.error) {
        const all = loadBids(guest).map((x) => (x.hash === b.hash ? { ...x, revealed: true } : x));
        saveBids(guest, all);
        setSaved(all.filter((x) => x.market === mk.id));
      }
      return r;
    }, onDone);

  return (
    <Card
      title="Sealed opening auction"
      right={
        <small className="muted">
          {ph === "commit" && `commits close in ${timeLeft(mk.auction_commit_end)}`}
          {ph === "reveal" && `reveals close in ${timeLeft(mk.auction_reveal_end)}`}
          {ph === "clear" && "ready to clear"}
          {mk.status !== "Auction" && (mk.opening_price_cents !== null ? `cleared at ${mk.opening_price_cents}¢` : "cleared, nothing matched")}
        </small>
      }
    >
      <p className="small">
        Bid for Long or Short claims with a price limit in cents per claim. Only a hash of the bid goes on-chain until the reveal
        window. At clearing one price P is chosen: Long buyers pay P, Short buyers pay 100 − P, and each matched pair mints one Long
        and one Short. Unused escrow, including unrevealed bids, is refunded.
      </p>
      {ph === "commit" && (
        <div className="form">
          <Field label="Side">
            <select value={side} onChange={(e) => setSide(e.target.value as Side)}>
              <option>Long</option>
              <option>Short</option>
            </select>
          </Field>
          <Field label="Claims">
            <input value={qty} onChange={(e) => setQty(e.target.value)} inputMode="numeric" />
          </Field>
          <Field label="Limit (¢ per claim)">
            <input value={limit} onChange={(e) => setLimit(e.target.value)} inputMode="numeric" />
          </Field>
          <Field label="Extra deposit (fUSD)" hint="optional: hides the bid size">
            <input value={extra} onChange={(e) => setExtra(e.target.value)} inputMode="decimal" />
          </Field>
          <div className="form-actions">
            <button disabled={demo || act.busy || !valid} onClick={commit}>
              {act.busy ? "Sending…" : `Commit sealed bid (escrow ${units(escrow)} fUSD)`}
            </button>
          </div>
        </div>
      )}
      {ph === "clear" && (
        <button disabled={demo || act.busy} onClick={() => act.run(() => callFlat(guest, "clear_auction", { market_id: mk.id }, { gasTgas: 300 }), onDone)}>
          Clear auction
        </button>
      )}
      {saved.length > 0 && (
        <>
          <h3>Your bids in this browser</h3>
          <p className="muted small">Back up the salts: without the salt a bid cannot be revealed (it is then refunded at clearing).</p>
          <ul className="saved">
            {saved.map((b) => (
              <li key={b.hash}>
                <span>
                  {b.side} {b.qty} @ {b.limit}¢, escrow {units(b.escrow)} fUSD
                </span>
                <span className="mono small break">salt {b.salt}</span>
                {b.revealed ? (
                  <span className="pill">revealed</span>
                ) : (
                  <button disabled={demo || act.busy || ph !== "reveal"} onClick={() => reveal(b)}>
                    Reveal
                  </button>
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {demo && <p className="muted small">Actions are disabled in scripted demo mode.</p>}
      {act.view}
    </Card>
  );
}

function PositionCard({
  mk,
  mine,
  ph,
  onDone,
}: {
  mk: Market;
  mine?: { long: string; short: string; refund: string };
  ph: string;
  onDone: () => void;
}) {
  const { guest, demo } = useApp();
  const [amount, setAmount] = useState("10");
  const [to, setTo] = useState("");
  const [side, setSide] = useState<Side>("Long");
  const act = useAction();
  const amt = parseUnits(amount);
  const ok = !!amt && amt > 0n && !demo;
  const settledish = mk.status !== "Auction" && mk.status !== "Open";

  const slot = useAsync(() => getSlotInfo(demo, mk.id), [demo, mk.id, mk.filled_slots], demo ? undefined : 10000);
  const requestObservation = () =>
    // No deposit: the contract pays OutLayer from its observation budget.
    act.run(() => callFlat(guest, "request_observation", { market_id: mk.id }, { gasTgas: 300 }), () => {
      onDone();
      slot.reload();
    });
  const s = slot.data;
  const slotFree = !!s && s.current_slot !== null && !s.current_filled && !s.current_pending;

  return (
    <Card title="Your position">
      <div className="grid4">
        <Stat k="Long claims" v={mine ? units(mine.long) : "—"} />
        <Stat k="Short claims" v={mine ? units(mine.short) : "—"} />
        <Stat k="Auction refund owed" v={mine ? `${units(mine.refund)} fUSD` : "—"} />
        <Stat k="Value if settled now" v={settledish && mine ? `${units(redeemValue(mk, mine))} fUSD` : "—"} />
      </div>
      {mine && BigInt(mine.refund) > 0n && (
        <button disabled={demo || act.busy} onClick={() => act.run(() => callFlat(guest, "withdraw_refund", {}, { gasTgas: 50 }), onDone)}>
          Withdraw {units(mine.refund)} fUSD refund
        </button>
      )}

      {mk.status === "Open" && (
        <>
          <div className="form">
            <Field label="Amount (claims / fUSD)">
              <input value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
            </Field>
            <div className="form-actions">
              <button disabled={!ok || act.busy} onClick={() => act.run(() => transferCall(guest, amt!, { action: "mint_pair", market_id: mk.id }), onDone)}>
                Mint pairs ({amount} fUSD → {amount} Long + {amount} Short)
              </button>
              <button disabled={!ok || act.busy} onClick={() => act.run(() => callFlat(guest, "redeem_pair", { market_id: mk.id, amount: amt!.toString() }), onDone)}>
                Redeem pairs
              </button>
            </div>
          </div>
          <div className="form">
            <Field label="Side">
              <select value={side} onChange={(e) => setSide(e.target.value as Side)}>
                <option>Long</option>
                <option>Short</option>
              </select>
            </Field>
            <Field label="Send to account">
              <input value={to} onChange={(e) => setTo(e.target.value.trim())} placeholder="account id" />
            </Field>
            <div className="form-actions">
              <button
                disabled={!ok || !to || act.busy}
                onClick={() => act.run(() => callFlat(guest, "claim_transfer", { receiver_id: to, market_id: mk.id, side, amount: amt!.toString() }), onDone)}
              >
                Transfer {amount} {side}
              </button>
              <button
                disabled={!ok || act.busy}
                onClick={() => act.run(() => callFlat(guest, "wrap_note", { market_id: mk.id, side, amount: amt!.toString() }, { gasTgas: 50 }), onDone)}
              >
                Wrap {amount} {side} in a Cover Note
              </button>
            </div>
          </div>
          <div className="row">
            <button disabled={demo || act.busy || ph !== "period" || !slotFree} onClick={requestObservation}>
              Request the price for this slot
            </button>
            <small className="muted">
              {s && s.current_slot !== null
                ? `Slot ${s.current_slot + 1} of ${s.num_slots}: ${s.current_filled ? "price recorded" : s.current_pending ? "request in flight" : "no price yet"}. `
                : ""}
              One price per slot; anyone can ask. The contract pays OutLayer's fee from its budget.
            </small>
          </div>
        </>
      )}

      {mk.status === "Open" && (
        <div className="row">
          <button disabled={demo || act.busy} onClick={() => act.run(() => callFlat(guest, "settle", { market_id: mk.id }), onDone)}>
            Settle
          </button>
          <small className="muted">
            {ph === "settle" ? "The period has ended: anyone can settle." : `Allowed after the period ends (${timeLeft(mk.period_end)}).`}
          </small>
        </div>
      )}

      {settledish && (
        <div className="row">
          <button
            disabled={demo || act.busy}
            onClick={() => act.run(() => callFlat(guest, "roll_market", { template: mk.template }, { gasTgas: 50 }), onDone)}
          >
            Roll the next {mk.template} market
          </button>
          <small className="muted">Anyone can, once the latest {mk.template} market is settled or void. Its band is centred on this market's average.</small>
        </div>
      )}
      {settledish && mine && (BigInt(mine.long) > 0n || BigInt(mine.short) > 0n) && (
        <button disabled={demo || act.busy} onClick={() => act.run(() => callFlat(guest, "redeem", { market_id: mk.id }), onDone)}>
          Redeem claims for fUSD
        </button>
      )}
      {demo && <p className="muted small">Actions are disabled in scripted demo mode.</p>}
      {act.view}
    </Card>
  );
}

function redeemValue(mk: Market, mine: { long: string; short: string }): bigint {
  const lp = BigInt(longPayout(mk.status) ?? 0);
  return (BigInt(mine.long) * lp + BigInt(mine.short) * (ONE - lp)) / ONE;
}
