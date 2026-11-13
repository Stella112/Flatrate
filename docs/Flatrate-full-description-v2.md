Flatrate: full description
Nov 12, 2026 · @Stellamaris · Version 3
Summary
Flatrate lets AI teams and agents fix next week’s GPU cost on NEAR. They buy fully backed claims on the price of one H100 rental hour, and those claims can never be liquidated.
Tagline: Lock next week’s AI compute bill. Fully backed, never liquidated, priced by a feed no operator can set.
•Tracks: Unstoppable DeFi, NFTs That Do Something, Privacy & Private AI. Agent Economy and Autonomous Business are added only if the agent extra ships.
•Network: NEAR testnet.
•Built on: OutLayer’s TEE (Intel TDX enclaves), which fetches the prices; NEAR Intents and Aurora Intents, shown as live quotes for paying in from other chains.
•Status: design agreed. Nothing is built yet, so every feature below is planned.
The problem
AI builders sell at fixed prices but buy compute at floating ones. When the price of GPU hours jumps, their margin disappears.
The price really moves. Each line below needs a source link before submission, or it is deleted:
•H100 rental fell from about $8–10 an hour in early 2024 to $1.80–3.50 by mid-2026. (source link needed)
•Ornn’s index showed $3.30 on 9 November 2026, up about 25% in a month.
•On one day in October, the same H100 hour cost $2.69 on RunPod and $4.17 across Shadeform’s clouds, according to PIRY’s README.
Who is hurt:
•Agents on NEAR’s Agent Market. They quote a fixed price per job, then pay floating prices for GPU time.
•AI startups. They charge customers a fixed monthly price and rent GPUs week to week.
•GPU owners and their lenders. They carry the opposite risk when prices fall.
Why existing markets don’t help them:
•CME’s and ICE’s compute futures are still waiting on regulatory review, and one CME contract covers 730 GPU-hours. (source link needed)
•The crypto versions are leveraged perpetuals. Bernstein describes that liquidity as “dominated by speculative flows”.
•None of them is on NEAR, and none is sized to a small team’s or an agent’s actual bill.
The solution
Flatrate is a small, fully backed price-lock market for GPU time, run by one keyless contract on NEAR.
•Lock a price, not a provider. A team keeps renting GPUs wherever it likes. Flatrate pays it back when the price rises above its band, and charges it when the price falls, so its net cost stays close to the price it locked.
•Fully backed. Every long and short pair is backed by one full dollar, so there is no margin, no leverage and no liquidation.
•Small enough for one agent. Claims are one dollar each, so a team or an agent can cover exactly its own weekly bill.
•No operator. Prices come from several public sources, fetched inside an OutLayer enclave. Once the contract’s keys are deleted, anyone can start the next market, record prices and settle.
•Hedges you can hand on. A Cover Note NFT holds a hedge and pays whoever owns it.
User flow
A team that rents GPUs (buys cover):
1.Opens the app. A guest account and test dollars (fUSD) are ready, with no wallet.
2.Sees the open H100 market: its band (floor and cap), current median price, and time left.
3.Places a sealed bid for long claims during the opening auction, then reveals it.
4.After the auction clears, receives its long claims at the clearing price.
5.Wraps them into a Cover Note NFT.
6.During the week, watches one price recorded per time slot on the Proof page.
7.After the week, anyone settles. The team redeems the note and is paid according to where the average landed.
A GPU owner (sells cover): does the same with short claims, which pay more when prices fall.
Anyone, at any time: records the next slot’s price, settles a finished market, or starts the next market. No permission is needed.
A judge: opens the hosted demo, presses the attack buttons, opens the Proof page, and runs one verify command on the repository.
Features
Feature	What it does	Status
Attested price feed	OutLayer fetches at least three GPU price sources inside an enclave and returns the median	Core
One price per time slot	The contract records at most one price per slot, so nobody can skew the average	Core
Long and short claims	$1 mints one of each; mint, redeem as a pair, transfer	Core
Sealed opening auction	Commit and reveal; one clearing price; unused deposits refunded	Core
Settlement on the period average	Average of filled slots; midpoint if too few prices arrive	Core
Cover Note NFT	NEP-171 token that holds claims and pays its holder	Core
Keyless contract with public roll	Keys deleted; anyone opens the next market, with its band set from the last average	Core
No-wallet hosted demo	Guest account created in the browser, sponsored for gas and test dollars	Core
Proof page and attack buttons	Every price with its OutLayer request; four attacks that fail on-chain	Core
TEE price fetching	The price code runs inside an OutLayer TEE; the Proof page shows the attestation data OutLayer returns	Core
NEAR Intents and Aurora Intents quotes	Live quotes from both for paying in from another chain, shown side by side, labelled quote only	Core
Verify command, claims table, scripted demo	Lets judges re-check every claim without a wallet	Core
Agent auto-hedging	An agent with a key that can only call Flatrate bids from its usage forecast	Extra 1
Private auction	Bids encrypted to an OutLayer enclave and cleared inside it	Extra 2
Model token price market	Same design, priced on AI model token costs	Extra 3
Cross-chain pay-in and payout	Chain signatures on other chains’ test networks	Extra 4
Real mainnet payment	One small pay-in through NEAR Intents and one payout through Aurora Intents	Extra 5
Rhea pool	Claims as tokens, traded in a Rhea pool on testnet, if Rhea’s testnet contract is live	Extra 6
How it works
One contract runs one market type, and each market moves through the same six steps.
Market	What is priced	Price sources	Expiry
GPU hours	One H100 rental hour	At least three public sources, chosen in Phase 0 from RunPod, Akash, Verda, Shadeform and Ornn’s index, and listed on the Proof page	Weekly (6-hour slots), plus a demo market (1-minute slots, settles in about 15 minutes)
1.Mint. One dollar of test stablecoin (fUSD) mints one long claim and one short claim. Balances are kept in the Flatrate contract.
2.Open with a sealed auction. Bids are committed as hashes, then revealed, and clear at one price. Bids are hidden until the auction closes.
3.Trade. Holders mint, redeem and transfer claims directly in the contract.
4.Price. OutLayer fetches every source inside an enclave and returns the median. The contract accepts a price only from OutLayer, for a pinned oracle version, once per time slot.
5.Settle. After the period, anyone can settle. The contract uses the average over the filled time slots, not one snapshot.
6.Redeem. Each long pays according to where the price landed in a fixed band, and each short pays the rest.
Payoff per claim:
long = clamp((average − floor) / (cap − floor), 0, 1)
short = 1 − long
Example: with a band of $2.50 to $4.50 and a settled price of $3.50, a long pays $0.50 and a short pays $0.50.
Why nothing can be liquidated: every pair of claims is backed by one full dollar from the start. A buyer’s largest possible loss is the price paid.
Prices per slot: each period is split into time slots. Anyone can request a price for the current slot; the contract accepts one per slot and frees the slot if the request fails.
No operator: the contract’s access keys are deleted after deployment. After that, anyone can roll the next market, request prices and settle. The next market’s band is set from the last settled average. If too few prices arrive, the market settles at the band’s midpoint once its void time passes.
Cover Note NFT
Each hedge can be wrapped into one NFT that holds the claims and pays whoever owns it at settlement.
•What it holds: the long or short claims, inside the contract, tied to the token.
•What it states: the cover in plain terms, for example “Cover Note #12: 100 Long, H100, period ending 2026-11-21”, with the floor and cap.
•Who gets paid: the current holder. Payout follows the NFT, not the wallet that bought it.
What a holder can do with it:
•Sell or transfer it. A team that no longer needs the compute passes its cover on as one item.
•Show it to a lender. It is on-chain proof that a borrower’s compute cost is fixed.
•Unwrap it before settlement to get the claims back, or redeem it after settlement for the payout.
It follows the NEP-171 standard, so any NEAR wallet or marketplace can show it.
Paying from another chain
The app shows two live quotes for paying in from another chain, side by side:
•NEAR Intents (1Click API): a dry-run quote, so nothing is sent.
•Aurora Intents: a quote from Aurora’s Intents API, using Flatrate’s API key.
Neither has a testnet, so on testnet both are quotes only, and the page says so. One real payment through each is an extra, done only with Maris’s approval because it costs a few dollars.
The TEE: what it does and what it proves
OutLayer runs code inside a trusted execution environment (TEE): an Intel TDX enclave, sealed hardware whose memory its operator cannot read or change.
•What runs inside it: Flatrate’s price component. It calls each price source over HTTPS, takes the median and returns it with an attestation, a hardware-signed record of which code ran, on which input, with which output.
•What that stops: a node operator editing prices in transit, and leaking the price sources’ API keys, which are stored as OutLayer secrets and opened only inside the enclave.
•How the contract trusts it: it accepts a price only from the OutLayer contract, for one pinned version of the price code. It does not check the hardware attestation itself; the Proof page shows the attestation data OutLayer returns so anyone can inspect it.
•What it does not prove: that a source’s price is true. It proves the right code fetched it unchanged.
•Its limits: OutLayer still holds governance keys at this stage (its own docs say so), and Intel TDX has had published hardware attacks. Both are listed in the README.
•Extra 2 uses it further: auction bids encrypted to an OutLayer enclave and cleared inside it, so no bid is ever public.
Privacy and trust
Bids stay hidden until the auction closes, everything else is public and checkable, and OutLayer is the main thing users must trust.
	What it covers
Hidden	Auction bids until reveal; API keys for price sources
Public	Revealed bids, the opening price, every price observation, the settled price, total collateral, every payout, who holds each Cover Note
Trusted	OutLayer’s TEE and its governance keys, and the price sources themselves
Stated plainly in the README:
•The contract does not verify a TEE attestation itself. It accepts results only from the OutLayer contract, for a pinned oracle version. The Proof page shows the attestation data OutLayer returns.
•The price sources are public endpoints. The checks prove format, units and bounds, not that a price is true.
•A price is recorded only if at least three sources succeeded, and the median is used, so one bad source cannot set the result on its own.
•Only one price is accepted per time slot, so nobody can weight the average by requesting many prices at a moment that suits them.
•A hedging market needs both sides. On testnet, we seed both ourselves.
Demo and proof
The demo shows one hedge from auction to payout in about two minutes, and every claim in it can be re-checked.
Demo script:
1.Two bidders commit sealed bids. After reveal, one clearing price appears.
2.A bidder wraps claims into a Cover Note and transfers it.
3.Attack buttons: a price posted by an account that is not OutLayer, a second price in the same time slot, and an early settle. Each fails on-chain.
4.The period ends, the average price settles the market, and the note pays its new holder.
5.Live NEAR Intents and Aurora Intents quotes are shown side by side, labelled quote only.
Proof layer for the judges, including Claude Code and Codex:
•One verify command that re-checks every claim without a wallet or secret, in a few minutes. The slower sandbox tests run under a separate test command.
•A claims table: each claim beside the test or transaction that proves it.
•AGENTS.md: what an AI reviewer should run, what it needs, how long it takes and what it should see, written as “do X, see Y”.
•Scripted demo mode: replays recorded real responses, plays the same way every time with no API key, and shows a “Scripted demo” badge.
•Hosted demo, no wallet: kept online until at least 25 November 2026.
•One fully settled demo market with every transaction linked, and one weekly market kept live through judging.
•“Remove NEAR and it stops” section: what breaks without OutLayer and NEAR’s yield and resume flow.
A feature that is described but does not run costs more than it adds, so nothing appears in the README without a row in the claims table.
Scope
The core must work end to end before any extra is mentioned in the README.
Core (must work)	Extras (only if finished, in this order)
Price feed for GPU hours through OutLayer, one price per time slot	Agent auto-hedging with a key that can only call Flatrate
Long and short claims: mint, redeem, transfer	Auction with bids encrypted to an OutLayer enclave
Sealed opening auction (commit and reveal)	Model token price market
Settlement on the period average, with a void path	Cross-chain pay-in and payout on test networks
Cover Note NFT	One real mainnet payment through NEAR Intents
Anyone can roll the next market after keys are deleted	Trading in a Rhea pool
Hosted demo with no wallet, plus the proof layer	Wallet login
Day-one checks, before any code:
•☐ Official rules read from nearhackathon.com/HACKATHON.md: two projects per person, testnet eligibility, demo post wording and how long the demo must stay up.
•☐ OutLayer on testnet: does a contract callback work, can the component fetch prices over HTTP, and how long and how much does a call take?
•☐ Price sources: do at least three GPU endpoints respond without problems? Record fixtures for demo day.
•☐ NEAR testnet account created and funded from the faucet.
•☐ NEAR Intents quote: one dry-run quote returns.
•☐ Aurora Intents: API key created at portal.intents.aurora.dev; one quote returns.
Deadline: development closes on 14 November 2026 at 23:59 UTC. The repository must be public by then. Features freeze on the evening of 13 November.
Second entry: Maris is also submitting Facedown (Stella112/facedown), a separate project. If the official rules confirm that two projects per person are allowed, each README names the other.
Known limitations
•Testnet only. Collateral is a test stablecoin with a public faucet. No real money is at risk or earned.
•No real counterparties yet. Both sides of the demo markets are seeded by us.
•Revealed bids are public. Bids are hidden only until the auction closes. Fully private bidding is an extra.
•The feed is only as good as its sources. If the sources are wrong together, the settled price is wrong.
•No upgrades after keys are deleted. A bug cannot be fixed, and if a price source changes its format, markets settle at the midpoint until a new contract is deployed.
•Paying from another chain is a quote, not a payment.
•Rhea trading depends on Rhea’s testnet contract. If it is not live, there is no pool and the README says so.
•The demo’s guest accounts are sponsored. When the sponsor budget runs out, the app is read-only and the scripted demo still works.
Where this idea already exists
Each row needs a source link before submission, or it is deleted.
Project	Where	How Flatrate differs
Kalshi, CME, ICE (source link needed)	Regulated venues	Open to small teams and agents, with no minimum size
GPU perpetuals on crypto exchanges (source link needed)	Several venues	No leverage and no liquidation
SealedSwarm, Co-op, Flock (source link needed)	This hackathon	They buy GPU hours. Flatrate fixes the price.
