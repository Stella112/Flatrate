import type { MarketStatus, Market } from "./types";

export const usd = (micros: string | number | bigint, digits = 2): string => {
  const n = Number(micros) / 1e6;
  return `$${n.toFixed(digits)}`;
};

/** Claim or fUSD units (6 decimals) as a plain number string, trailing zeros trimmed. */
export const units = (u: string | number | bigint): string => {
  const n = BigInt(u);
  const whole = n / 1_000_000n;
  const frac = n % 1_000_000n;
  if (frac === 0n) return whole.toString();
  return `${whole}.${frac.toString().padStart(6, "0").replace(/0+$/, "")}`;
};

/** Parses "12.5" into 6-decimal units; null if invalid. */
export const parseUnits = (s: string): bigint | null => {
  const m = s.trim().match(/^(\d+)(?:\.(\d{0,6}))?$/);
  if (!m) return null;
  return BigInt(m[1]) * 1_000_000n + BigInt((m[2] || "").padEnd(6, "0") || "0");
};

export const nsToDate = (ns: string | number): Date => new Date(Number(BigInt(ns) / 1_000_000n));

export const utc = (ns: string | number): string => nsToDate(ns).toISOString().replace("T", " ").slice(0, 16) + " UTC";

export const near = (yocto: bigint, digits = 3): string => (Number(yocto / 10n ** 16n) / 1e8).toFixed(digits);

export function statusLabel(s: MarketStatus): string {
  if (typeof s === "string") return s;
  return "Settled";
}

export function longPayout(s: MarketStatus): number | null {
  if (typeof s === "object" && "Settled" in s) return s.Settled.long_payout_micros;
  if (s === "Void") return 500_000;
  return null;
}

/** The next deadline that matters for this market, with its name. */
export function nextDeadline(m: Market, nowMs = Date.now()): { label: string; ns: string } | null {
  if (m.status !== "Auction" && m.status !== "Open") return null;
  const now = BigInt(nowMs) * 1_000_000n;
  const steps: [string, string][] = [
    ["commits close", m.auction_commit_end],
    ["reveals close", m.auction_reveal_end],
    ["period starts", m.period_start],
    ["period ends", m.period_end],
    ["void after", m.void_after],
  ];
  for (const [label, ns] of steps) if (BigInt(ns) > now) return { label, ns };
  return null;
}

export function timeLeft(ns: string, nowMs = Date.now()): string {
  let s = Math.max(0, Math.floor((Number(BigInt(ns) / 1_000_000n) - nowMs) / 1000));
  const d = Math.floor(s / 86400);
  s -= d * 86400;
  const h = Math.floor(s / 3600);
  s -= h * 3600;
  const m = Math.floor(s / 60);
  s -= m * 60;
  if (d) return `${d}d ${h}h`;
  if (h) return `${h}h ${m}m`;
  if (m) return `${m}m ${s}s`;
  return `${s}s`;
}

export function phase(m: Market, nowMs = Date.now()): "commit" | "reveal" | "clear" | "period" | "settle" | "redeem" {
  const now = BigInt(nowMs) * 1_000_000n;
  if (m.status === "Auction") {
    if (now < BigInt(m.auction_commit_end)) return "commit";
    if (now < BigInt(m.auction_reveal_end)) return "reveal";
    return "clear";
  }
  if (m.status === "Open") return now < BigInt(m.period_end) ? "period" : "settle";
  return "redeem";
}

export const shortHash = (h: string) => (h.length > 12 ? `${h.slice(0, 8)}…` : h);
export const shortAcct = (a: string) => (a.length > 24 ? `${a.slice(0, 6)}…${a.slice(-4)}` : a);
