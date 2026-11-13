// Reads that work in both live mode (chain) and scripted demo mode (recorded fixture).
import { flat } from "./near";
import { demoBids, demoConfig, demoMarket } from "./demo";
import type { Bid, Config, Market, SlotInfo } from "./types";

export async function getMarkets(demo: boolean, step: number): Promise<Market[]> {
  if (demo) return [demoMarket(step)];
  const all = await flat<Market[]>("list_markets", { limit: 100 });
  return all.slice().reverse(); // newest first
}

export async function getMarket(demo: boolean, step: number, id: string): Promise<Market | null> {
  if (demo) return id === "2" ? demoMarket(step) : null;
  return flat<Market | null>("get_market", { market_id: id });
}

export async function getBids(demo: boolean, step: number, id: string): Promise<Bid[]> {
  if (demo) return id === "2" ? demoBids(step) : [];
  return flat<Bid[]>("get_bids", { market_id: id, limit: 200 });
}

export async function getConfig(demo: boolean): Promise<Config> {
  return demo ? demoConfig : flat<Config>("get_config");
}

export async function getSlotInfo(demo: boolean, id: string): Promise<SlotInfo | null> {
  if (demo) return null;
  return flat<SlotInfo>("slot_info", { market_id: id });
}
