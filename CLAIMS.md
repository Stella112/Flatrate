# CLAIMS

Every claim in README.md and AGENTS.md has a row here. Explorer: `https://testnet.nearblocks.io/txns/<hash>`.

| Claim | How to check | Evidence |
|---|---|---|
| Payoff: Long gets `clamp((avg-floor)/(cap-floor),0,1)` dollars, Short the rest, and Long + Short is always exactly $1 | `cargo test -p flatrate-shared` (`payoff_*`, `prop_long_plus_short_is_one_dollar`) | PROGRESS.md Phase 1 |
| The opening auction is sealed (commit-reveal) and uniform-price, filling by better limit then earlier commit | sandbox `auction_two_bid_match`, `auction_tie_break`, `auction_partial_fill` | PROGRESS.md Phase 2; testnet market 2 commit [FtGfietX](https://testnet.nearblocks.io/txns/FtGfietXW3iEQEWd1nqt24YSrL3KcJ3h4unuaBzvaP5c), reveal [CFBVocs3](https://testnet.nearblocks.io/txns/CFBVocs334kHjBYt5FCANjttTeYToxa5beZniovAmBUq), clear [36ay9uLT](https://testnet.nearblocks.io/txns/36ay9uLTur5W25J1yuDngkVDE73Wf9xQTXKxb1vpJVry) |
| Unrevealed bids and unused escrow are refunded | sandbox `auction_unrevealed_bid_refunded` | testnet market 0 clear [EBRPG4uR](https://testnet.nearblocks.io/txns/EBRPG4uRDbFk62MUHN7JrrnHi6oc1Dp4PjuvEY6BSabQ), refund 11 fUSD [A4QFJEnF](https://testnet.nearblocks.io/txns/A4QFJEnFNwrtBaMk3AzHMT2XP3RtBR9yFDzy8N19DR5f) |
| 200 bids clear in one transaction | sandbox `auction_200_bids_clear_within_gas` | 35.77 TGas (PROGRESS.md Phase 2) |
| Prices are fetched inside OutLayer from 5 public APIs and the median is posted on-chain | `get_market` on market 2 shows 3 observations with OutLayer request ids | [J5eEofdJ](https://testnet.nearblocks.io/txns/J5eEofdJEPKpk1BMe5zy8Rj6rbV8mR5PhhoNUrVFAshk), [3Wed41tV](https://testnet.nearblocks.io/txns/3Wed41tV6aTaJxWHpHUfjgsWYdTyGikrn6sxfsx7kSZd), [4zsSXYWo](https://testnet.nearblocks.io/txns/4zsSXYWoaxW37xZoiTj2QUv5kZV1khHt9s4AEr7494wb) |
| The oracle version is pinned by wasm sha256 | `get_config` / `get_market` -> `oracle_version_key`; OutLayer `get_version` | FACTS.md 9.1, 9.2 |
| Only the contract's own OutLayer callback can record a price | sandbox `oracle_non_outlayer_caller_rejected` (`ERR_NOT_OUTLAYER`), `oracle_unknown_request_id_rejected` (`ERR_UNKNOWN_REQUEST`) | PROGRESS.md Phase 1 |
| Results with fewer than 3 sources, or a price outside (0, 10 x cap), are not recorded | sandbox `oracle_fewer_than_3_sources_rejected`, `oracle_out_of_range_price_rejected` | PROGRESS.md Phase 1 |
| Anyone can settle after the period; settlement is final | sandbox `settle_too_early_refused`, `too_few_observations_waits_then_voids_then_second_settle_refused` | testnet market 2 settle [DrfwCuCv](https://testnet.nearblocks.io/txns/DrfwCuCv7UV3mmkokj1pn8NHf7hHjYnj2UtJA878iZjF) |
| Too few observations: the market waits, then settles void at 0.50/0.50 | sandbox `too_few_observations_waits_then_voids_then_second_settle_refused` | testnet market 0 void [3ibECLWe](https://testnet.nearblocks.io/txns/3ibECLWeZ7G4JT7MJ9ZFkTUAV5qyL1Xio13Fz1V8BmRL) |
| Holders redeem claims for fUSD after settlement; a second redeem pays nothing | sandbox `observe_settle_redeem_and_double_redeem` | testnet market 2 redeem [457EHofV](https://testnet.nearblocks.io/txns/457EHofVsV1WBohX2a4GZnd9zN8VcYey3KbKX8wxTF4P) |
