# FireWatch Backlog

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
