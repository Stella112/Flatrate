# Demo script (about 2 minutes)

Screen recording of https://flatrate-gpu.vercel.app/ in a fresh browser window. Testnet only. Times are targets.

The plan follows the project brief in CLAUDE.md. The Flatrate description page linked there could not be opened from this machine (BLOCKERS.md B9), so check its Demo section against this script before recording.

| Time | Screen | Say (short) |
|---|---|---|
| 0:00 | Markets page | "AI teams rent GPUs by the hour, and the price moves. Flatrate lets them lock next week's H100 price on NEAR. Long pays more if the price rises, Short if it falls. One test dollar backs one of each." |
| 0:12 | Click "Create guest account" | "No wallet: the page makes a key in the browser, and a sponsor funds its testnet account with NEAR and 100 fUSD." |
| 0:20 | Open the newest short market (create one beforehand: 3 min commits, 3 min reveals, 6 min period). Commit Long 10 @ 55¢ and Short 10 @ 50¢ | "The opening auction is sealed. Only a hash goes on-chain. The salt stays in this browser." |
| 0:40 | (cut to after the commit window) Reveal both, then Clear auction | "After reveals, one clearing price: here 50 cents. Each matched pair mints one Long and one Short." |
| 0:55 | Request a price observation | "Anyone can ask for a price. The contract calls OutLayer, which runs our pinned oracle: five public GPU price APIs, median on-chain." |
| 1:05 | Proof page: the table with request ids, ✓, per-source prices | "Every observation links to its transaction, shows each source, and checks that what the contract stored equals OutLayer's output. The oracle version is a wasm hash anyone can rebuild." |
| 1:20 | Proof page: "Try to break it": Fake price, Steal a note | "A made-up price is refused, ERR_NOT_OUTLAYER. Redeeming someone else's note is refused, ERR_NOT_OWNER." |
| 1:30 | Market page: Wrap 5 Long in a Cover Note, My notes, Transfer | "Claims can be wrapped in a Cover Note NFT and sent to anyone. Whoever holds it at settlement gets paid." |
| 1:42 | (cut to after the period) Settle, Redeem | "After the period anyone settles. Long pays where the average fell between floor and cap; Short gets the rest." |
| 1:52 | Terminal: `make verify` table | "Judges can check it all: make verify runs the tests and reads the deployed contracts." |
| 1:58 | End card | "Flatrate. Built at the NEAR Hackathon." |

X post (Maris posts it): "Flatrate: lock next week's GPU price on NEAR. Sealed auction, prices from 5 GPU APIs attested by OutLayer, Cover Note NFTs. Built at the NEAR Hackathon. @NEARProtocol #NEARHackathon https://flatrate-gpu.vercel.app/"
