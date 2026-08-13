# Stargazer web

SvelteKit website for Stargazer by Ferno, a Stellar protocol monitoring and intelligence layer.

The public product page is served at `/`. The product preview is under `/app`, with:

- `/app` — validated G-/M-address wallet lookup;
- `/app/wallet/[address]` — every exact wallet-contract relationship, verified
  protocol attribution where available, C-address placeholders for
  unattributed contracts, a factual coverage report, alert candidates split by
  severity, and source-backed historical security findings;
- `/app/watch/[address]` — choose critical/non-critical policies and create a
  short-lived Telegram pairing link through the separate alert-control service;
- `/app/leaderboard?sort=transactions|protocols` — URL-backed wallet rankings.
- `/app/alerts` — one compact register for verified incidents, exact contract
  upgrades, and successful on-chain operations submitted by monitored Classic
  controllers, with transaction evidence and bounded operation payloads;
- `/app/protocols` — a lightweight directory of identified protocols or
  contract-ID placeholders for unattributed contracts;
- `/app/protocols/[id]` — registered roots, exact executable history, upgrade
  dates, controller policy, and transaction evidence for one protocol.

The grant working page is served at `/scf/application` with `noindex`, `nofollow`, and
`noarchive` headers.

The product data routes use a typed read-only API colocated with the NAS archive
index. Wallet relationships, serving-snapshot rankings, contract usage, and
executable histories are real indexed facts. Historical wallet findings come
from bounded archive replay against verified incident evidence. They are marked
as retrospective and never presented as alerts that Stargazer delivered at the
time. Protocol details also read complete signer, weight, and threshold cycles
from the NAS watcher. Successful operations from monitored controllers are
shown after ledger inclusion; unsigned or partially signed proposals exchanged
off-chain are outside this POC until a coordinator integration is available.

Telegram pairing uses a separate writable control plane. The SvelteKit form
action calls it server-side through `STARGAZER_ALERT_CONTROL_BASE`; the browser
never supplies a Telegram user or chat ID. Pairing links contain a one-time
15-minute bearer token and are rendered with a no-referrer policy. The raw
bearer is held only in a short-lived, HttpOnly, same-site cookie. A same-origin
status route uses it server-side to show waiting, paired, or expired state; the
token is never returned to browser JavaScript or placed in a status URL. Billing
is fail-closed: the UI shows pilot entitlement only when the service explicitly
enables it, and otherwise states that payment is not configured. The alert
delivery worker is not connected in this POC. No placeholder invoice, delivery
claim, or unverified entitlement is created.

Protocol identity marks are served locally from `static/protocols`; the source
and usage notes for every third-party mark are recorded in
[`static/protocols/README.md`](static/protocols/README.md). The UI keeps the
protocol name visible and falls back to initials for unknown IDs, so logos are
decorative context rather than the only identifier.

Unattributed contracts use deterministic local identicons and their on-chain
C-address. They remain explicitly separate from verified protocol identities;
an address placeholder is discovery evidence, not a protocol-name claim.

Production is published at [`stargazer.ferno.ag`](https://stargazer.ferno.ag/).
`stellarwatch.ferno.ag` remains available as a legacy alias. The Cloudflare Pages project
retains its existing `ferno-pulsar` technical identifier.

In production, SvelteKit reaches the NAS through a private Cloudflare service
binding to `stargazer-nas-gateway`. That Worker permits only the read API routes
used by the demo and reaches `127.0.0.1:8787` through the existing Cloudflare
Tunnel and Workers VPC. The NAS read API has no public hostname. Local and NAS
Vite development continue to use `STARGAZER_API_BASE` or the local
`http://127.0.0.1:8787` default.

## Developing

```bash
npm ci
npm run dev
```

Copy `.env.example` and set `STARGAZER_API_BASE` plus
`STARGAZER_ALERT_CONTROL_BASE` for the NAS services.

## Verification

```bash
npm run check
npm run build
```
