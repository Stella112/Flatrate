# Demo script (about 2 minutes)

Follows the "Demo and proof" section of [Flatrate-full-description-v2.md](Flatrate-full-description-v2.md). Screen recording of https://flatrate-gpu.vercel.app/ on testnet, recorded in segments from one real DEMO market (3 min commits, 2 min reveals, 8 one-minute slots, settles about 15 minutes after it opens). Two browser windows: window A (bidder 1, buys Long) and window B (bidder 2, sells cover with Short), each with its own guest account.

Before recording: if the latest DEMO market is settled, press "Roll the next DEMO market" in either window (any account can). Keep the keeper running (it fills one price per slot and settles).

| Time | Screen | Say (short) |
|---|---|---|
| 0:00 | Markets page | "AI teams rent GPUs by the hour, and the price moves. Flatrate locks next week's H100 price on NEAR. One test dollar backs one Long and one Short, so nothing can be liquidated." |
| 0:10 | A and B: "Create guest account" | "No wallet: each window gets a key in the browser and a sponsored testnet account." |
| 0:18 | A commits Long 10 @ 55¢; B commits Short 10 @ 50¢ | "The opening auction is sealed. Only a hash goes on-chain." |
| 0:30 | (cut to reveal window) A and B reveal; one presses "Clear auction" | "After reveals there is one clearing price: 50 cents. Each matched pair mints one Long and one Short." |
| 0:45 | A: "Wrap 10 Long in a Cover Note"; My notes: transfer it to B's account | "A wraps its cover into a Cover Note NFT and hands it on. Whoever holds it at settlement gets paid." |
| 1:00 | Proof page, "Try to break it": Fake price, Second price in the same slot, Early settlement | "A price posted by anyone but OutLayer's callback: refused. A second price in a slot that already has one: refused. Settling before the week ends: refused. Each is a real failed transaction." |
| 1:20 | Market page: chart with one price per slot; Proof page attestation column | "Prices come from five GPU price APIs, fetched inside OutLayer's enclave, one per time slot. Each has an attestation for the pinned code and our input." |
| 1:35 | (cut to after the period) settled market; B on My notes: Redeem | "The week ends, the average of the filled slots settles the market, and the note pays its new holder." |
| 1:48 | Pay from any chain: "Get both quotes" | "Paying in from another chain: live quotes from NEAR Intents and Aurora Intents, quote only on testnet." |
| 1:56 | End card | "Flatrate. Built at the NEAR Hackathon." |

If OutLayer's testnet is slow or failing during recording, slots stay empty and the market may settle void; record a later market instead (see BLOCKERS.md B10).

X post (Maris posts it): "AI teams buy GPU time at floating prices. Flatrate locks next week's H100 price on NEAR: fully backed claims, a sealed opening auction, prices fetched by OutLayer, Cover Note NFTs. Built at NEAR HACKATHON @NEARProtocol #NEARHackathon https://flatrate-gpu.vercel.app/ Which part would you check first?"
