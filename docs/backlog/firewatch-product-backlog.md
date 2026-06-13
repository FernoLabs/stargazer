# FireWatch Backlog

## Monetization and Account Entitlements

Status: in progress

Goal: keep FireWatch free for Ferno members while making non-member custom bot setup, arbitrary program/watch rules, and pro response access paid.

Why it matters:
- Custom alert setup creates support and infrastructure cost.
- A paid account gives teams a clear path from "watch this surface" to a private bot with tested delivery.
- Pro response can package reviewed signals and Smart Account actions without confusing them with the free shared watch.

Implementation ideas:
- Free Shared Watch: curated protocols, exact program commands, and public activity.
- Paid Account: dedicated bot, custom program/multisig/wallet rules, chosen alert thresholds, private test alert.
- Superteam Promo: admin-created code that grants 12 months of free product access.
- Pro Response: on-demand reviewed signal access plus authorized Smart Account withdrawal and redeploy workflow.

Open questions:
- Which billing provider and account identity model should own entitlement checks?
- Should paid accounts be self-serve, manually provisioned, or both?
- What policy approvals are required before Smart Account fund movement is enabled?

## Wallet to Protocol Detection

Status: backlog

Goal: let a user paste one or more wallet addresses and have FireWatch infer the protocol surfaces they likely use, then generate the exact Telegram commands to watch those programs.

Why it matters:
- It removes the biggest setup burden for non-technical users.
- It makes FireWatch feel personalized without asking users to understand every program ID first.
- It creates a clean bridge from "I use these DeFi apps" to "I am protected for protocol-risk events."

Implementation ideas:
- Start with read-only wallet analysis based on recent instructions, token accounts, and known protocol program touches.
- Score protocol confidence instead of pretending attribution is perfect.
- Generate copy-ready bot commands grouped by protocol or exact program surface.
- Keep the first version conservative and explain why a protocol was inferred.

Open questions:
- How far back should FireWatch inspect wallet activity by default?
- Should protocol detection focus on direct user interaction only, or also routed activity?
- When should we ask the user to confirm inferred protocols before creating watch commands?

## Community Review for Program Upgrades

Status: backlog

Goal: let trusted FireWatch members review program upgrades, add annotations, and vote on whether an event looks routine, risky, or suspicious.

Why it matters:
- Program upgrades are high-signal, but they still benefit from human context.
- Shared review can turn FireWatch into a stronger operator console rather than just an alert feed.
- A small invite-only community can improve trust without opening the door to spam immediately.

Implementation ideas:
- A dedicated upgrade review page listing tracked programs, current versions, and recent upgrade events.
- Annotation stream per upgrade event with concise operator notes.
- Lightweight vote model: review now / looks routine / suspicious.
- Show reviewer count and consensus level directly on the event page.

Access model ideas:
- Start invite-only for FireWatch members.
- Later consider token-gated or NFT-gated access if it fits the community shape.
- Keep moderation centralized at first to preserve signal quality.

Open questions:
- What identity system should reviewers use?
- Should comments be immutable or allow edits?
- How do we separate authoritative FireWatch notes from community notes in the UI?

## Exploit-Oriented DeFi Runtime Alerts

Status: proposed

Goal: add generic runtime alerts that would have made the June 2026 Raydium legacy AMM V3 exploit visible even though it did not involve a current Raydium program upgrade or known admin instruction.

Why it matters:
- The Raydium incident drained value from old live pools that were no longer used through the official UI.
- FireWatch should detect suspicious economic movement even when protocol-specific instruction semantics are incomplete.
- Deprecated status is often hard to know up front, so dormancy should become a generic risk signal.

Implementation ideas:
- Pool reserve drain alerts: track configured token vaults and alert when a pool/vault loses a large share of reserves in one transaction or over a short window.
- Dollar-aware thresholds: start with simple percentage thresholds, then enrich with token pricing for rules like "more than 25k USD or more than 30% of vault value."
- Dormant surface wakeup alerts: alert when a watched program, pool, or vault has activity after a long quiet period, for example no observed transactions for 30, 60, or 90 days.
- Severity composition: escalate when a dormant surface wakeup and reserve drain happen in the same transaction or short window.
- Curated seed lists: keep manual labels for known deprecated pools/programs when we have them, but do not require a surface to be labeled deprecated before alerting.

Raydium lesson:
- A generic dormant wakeup rule could have flagged the old AMM V3 surface becoming active again.
- A pool reserve drain rule could have flagged the actual outflow from the affected pools.
- Together, those signals would likely have produced a high-severity "old liquidity surface woke up and drained" alert without needing full Raydium-specific exploit decoding.
- Backfill check: program-level dormancy alone is too coarse for Raydium-style incidents because legacy farm/staking programs can keep receiving unrelated activity. The dormant unit should move down to pool/vault/token-account surfaces, then compose with reserve-drain detection.

Deferred deeper work:
- Protocol invariant alerts: decode protocol-specific instructions and verify semantic invariants such as "withdraw uses the configured LP mint" or "burned LP supply is proportional to reserves removed."
- Hand-forged instruction detection: compare decoded account layouts and instruction accounts against expected protocol schemas to catch fake mints, fake vaults, or suspicious account substitutions.
- This is likely the best long-term path for detecting failed hack attempts, but it requires per-protocol parsers and should follow the generic reserve/dormancy alerts.

Open questions:
- What is the first asset class to support: AMM pool vaults, lending reserves, treasury wallets, or all watched token accounts?
- Should dormant wakeup use FireWatch's observed event history only, or query chain/RPC history for a stronger last-activity baseline?
- What default thresholds avoid noise during legitimate migrations and pool wind-downs?
