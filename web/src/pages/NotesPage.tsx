import { useState } from "react";
import { href, useApp } from "../App";
import { EXPLORER_TX, FLATRATE_ID } from "../config";
import { callFlat, flat, view } from "../near";
import { longPayout, statusLabel, units, usd } from "../format";
import type { Market, NftToken, Note } from "../types";
import { Card, Field, useAction, useAsync } from "../ui";

type Row = { token: NftToken; note: Note | null; market: Market | null };

export function NotesPage() {
  const { guest, demo, refreshWallet } = useApp();
  const list = useAsync(
    async (): Promise<Row[]> => {
      if (demo) return [];
      const tokens = await view<NftToken[]>(FLATRATE_ID, "nft_tokens_for_owner", { account_id: guest.accountId, limit: 100 });
      return Promise.all(
        tokens.map(async (token) => {
          const note = await flat<Note | null>("note_info", { token_id: token.token_id });
          const market = note ? await flat<Market | null>("get_market", { market_id: note.market_id }) : null;
          return { token, note, market };
        }),
      );
    },
    [guest.accountId, demo],
    demo ? undefined : 15000,
  );
  const reload = () => {
    list.reload();
    refreshWallet();
  };

  return (
    <>
      <Card title="My Cover Notes">
        <p className="small">
          A Cover Note is an NFT (NEP-171) that holds Long or Short claims of one market. Whoever holds it at settlement redeems it
          for fUSD. Wrap claims from a market page; unwrap returns them before settlement.
        </p>
        {demo && (
          <p className="small">
            Scripted demo: market 2 had no notes. The recorded testnet note round trip (market 3) is: wrap{" "}
            <a className="mono" href={EXPLORER_TX + "3HZS2pZHbrUSvvQgzUxS1WrMStxF4RsJmCMgSAnnfGrA"} target="_blank" rel="noreferrer">
              3HZS2pZH…
            </a>
            , transfer to another account{" "}
            <a className="mono" href={EXPLORER_TX + "3d2a5kcu5aPpB74XRKCszLvo2CtgBAVhmutaQbSVNGDJ"} target="_blank" rel="noreferrer">
              3d2a5kcu…
            </a>
            , old owner refused{" "}
            <a className="mono" href={EXPLORER_TX + "EykARtbbnerbUyAwwTnDgzQNeqHCDHKzXiG1CaH3jg86"} target="_blank" rel="noreferrer">
              EykARtbb…
            </a>
            , new owner redeemed{" "}
            <a className="mono" href={EXPLORER_TX + "AV9VT9anMcr4Kem2zQmCFfuoGPN6D4skptqgjfKZHduT"} target="_blank" rel="noreferrer">
              AV9VT9an…
            </a>
            .
          </p>
        )}
        {list.error && <p className="err">{list.error}</p>}
        {!demo && list.data && list.data.length === 0 && <p className="muted">This guest holds no notes.</p>}
      </Card>
      {list.data?.map((r) => <NoteCard key={r.token.token_id} row={r} onDone={reload} />)}
    </>
  );
}

function NoteCard({ row, onDone }: { row: Row; onDone: () => void }) {
  const { guest } = useApp();
  const [to, setTo] = useState("");
  const act = useAction();
  const { token, note, market } = row;
  const lp = market ? longPayout(market.status) : null;
  const value = note && lp !== null ? (BigInt(note.amount) * BigInt(note.side === "Long" ? lp : 1_000_000 - lp)) / 1_000_000n : null;
  const open = market?.status === "Open";

  return (
    <Card title={token.metadata?.title || `Cover Note #${token.token_id}`} right={market && <span className="pill">{statusLabel(market.status)}</span>}>
      {note && (
        <p>
          {units(note.amount)} {note.side} claims of <a href={href(`/market/${note.market_id}`)}>market {note.market_id}</a>
          {market && ` (floor ${usd(market.floor_micros)}, cap ${usd(market.cap_micros)})`}
          {value !== null && ` · redeems for ${units(value)} fUSD`}
        </p>
      )}
      <div className="row">
        <button disabled={act.busy || !open} onClick={() => act.run(() => callFlat(guest, "unwrap_note", { token_id: token.token_id }), onDone)}>
          Unwrap
        </button>
        <button disabled={act.busy || value === null} onClick={() => act.run(() => callFlat(guest, "redeem_note", { token_id: token.token_id }), onDone)}>
          Redeem
        </button>
      </div>
      <div className="form">
        <Field label="Transfer to account">
          <input value={to} onChange={(e) => setTo(e.target.value.trim())} placeholder="account id" />
        </Field>
        <div className="form-actions">
          <button
            disabled={act.busy || !to}
            onClick={() => act.run(() => callFlat(guest, "nft_transfer", { receiver_id: to, token_id: token.token_id }, { gasTgas: 30, deposit: 1n }), onDone)}
          >
            Transfer note
          </button>
        </div>
      </div>
      {act.view}
    </Card>
  );
}
