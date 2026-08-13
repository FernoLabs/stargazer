# Stargazer alert control plane

This is a separate writable service for Telegram pairing, alert policies, and
entitlements. It does **not** open or modify the compact ledger index, the
wallet serving snapshot, or `stargazer-read-api`. Give it its own SQLite file
and operating-system credentials.

The implemented billing modes are intentionally narrow:

- `unconfigured` (default) links Telegram and records a policy with
  `awaitingEntitlement`; it collects no payment and creates no entitlement;
- `pilot` creates an operator-configured, time-bounded pilot entitlement when
  the Telegram user claims the setup link.

There is no paid mode, checkout endpoint, price, settlement watcher, or claim
that payment works. Add a real Stellar USDC settlement adapter and its tests
before presenting paid activation as available.

There is also no alert event outbox or delivery worker in this crate. The bot
implements pairing and policy control commands only. Even a policy whose state
is `active` means “pilot entitlement granted”; it does not mean monitoring
events are being dispatched to Telegram. `GET /health` therefore reports
`alertDeliveryConfigured: false` unconditionally in this POC.

## Pairing flow

1. The frontend submits a normalized policy to `POST /api/alert-pairings`.
2. The service creates a cryptographically random 64-character lowercase-hex
   token valid for exactly 15 minutes. SQLite receives only
   `SHA-256(token)`, never the token itself.
3. The frontend keeps `claimToken` in a server-side session and sends the user
   to the returned `https://t.me/<bot>?start=<token>` link.
4. In a private Telegram chat, `/start <token>` atomically consumes the claim,
   binds the Telegram chat/user identity, and creates or updates the policy.
5. The frontend polls `GET /api/alert-pairings/status` with
   `Authorization: Bearer <claimToken>`. The bearer is deliberately not placed
   in the status URL. At `expiresAt` the bearer is invalid even if Telegram
   already claimed it; the endpoint returns the same generic HTTP 404 used for
   an unknown token and discloses no wallet or policy metadata.

The raw claim token and Telegram deep-link query string are secrets. Do not put
them in client analytics, error reporting, reverse-proxy access logs, or
referrer headers. Every API response sets `Cache-Control: no-store` and
`Referrer-Policy: no-referrer`. The frontend should perform creation and status
polling in server-side actions, not store the bearer in browser persistence.

## HTTP contract

The default listener is `127.0.0.1:8788`.

`POST /api/alert-pairings` accepts:

```json
{
  "walletAddress": "G...",
  "criticalAlerts": true,
  "nonCriticalAlerts": true
}
```

It returns HTTP 201:

```json
{
  "claimToken": "<64 lowercase hex characters>",
  "telegramUrl": "https://t.me/stargazer_bot?start=<token>",
  "expiresAt": 1785254400,
  "status": "pendingTelegram",
  "walletAddress": "G...",
  "policy": {
    "criticalAlerts": true,
    "nonCriticalAlerts": true,
    "state": "pendingTelegram"
  },
  "billing": {
    "mode": "unconfigured",
    "paymentState": "unconfigured",
    "entitlementState": "notConfigured"
  }
}
```

In pilot mode, billing is
`{"mode":"pilot","paymentState":"notRequired","entitlementState":"grantedOnPairing"}`.
Before `expiresAt`, the status endpoint returns the same coverage, policy, and
billing fields but omits `claimToken` and `telegramUrl`; its top-level status is
`pendingTelegram` or `paired`. At and after `expiresAt`, it returns a generic
HTTP 404 instead of an `expired` payload. Policy state is one of
`pendingTelegram`, `awaitingEntitlement`, `active`, or `paused`.

Pilot entitlement intervals are start-inclusive and end-exclusive. A pilot is
valid at `endsAt - 1` and becomes `expired` exactly at `endsAt`; an active
policy without a currently valid entitlement is moved to
`awaitingEntitlement`. Reopening the database in `unconfigured` mode revokes
every active entitlement and demotes active policies. Explicitly paused
policies remain paused. Reconciliation runs at startup and before pairing
status or Telegram policy-list reads.

`GET /health` reports whether a bot poller was configured, explicitly reports
`alertDeliveryConfigured: false`, and includes the billing mode. When no bot
token is configured, health stays available but pairing creation fails closed
with HTTP 503. Error responses use
`{"error":{"code":"snake_case_code","message":"..."}}`.

## Telegram bot

The bot verifies `getMe`, refuses to start if the configured username does not
match the token, and refuses to long-poll while a webhook remains configured.
It registers and supports:

- `/start <token>` — atomically claim a 15-minute pairing token;
- `/help` — show commands;
- `/list` — list up to 50 policies, with Telegram output bounded to 15;
- `/settings` — review policies and return to the configured web app;
- `/unwatch G...` — pause exactly one full wallet address and revoke an active
  pilot entitlement.

Pairing and policy mutations are accepted only from private chats where the
Telegram chat ID equals the sender user ID. `/unwatch` requires the complete
checksum-valid address to prevent broad accidental removal.

## Run locally

Create a bot with BotFather, ensure it has no webhook, and export the variables
from `../.env.example`. Keep the token in the environment rather than process
arguments:

```bash
cargo run --release --manifest-path alert-control-rs/Cargo.toml
```

The service checks the token and registers commands before it opens the HTTP
listener. For an API-only health check, omit `STARGAZER_TELEGRAM_BOT_TOKEN`;
pairing POSTs will return 503.

Before internet exposure, put an authenticated rate-limiting reverse proxy in
front of the service, redact pairing secrets from all logs, terminate HTTPS,
back up the dedicated SQLite database, and keep the Telegram bot token in a
secret manager. CORS is not authentication.

## Verification

```bash
cargo fmt --manifest-path alert-control-rs/Cargo.toml -- --check
cargo check --offline --manifest-path alert-control-rs/Cargo.toml
cargo test --offline --manifest-path alert-control-rs/Cargo.toml
```

The tests cover wallet normalization, token format and hash-only storage,
15-minute bearer expiry without post-expiry metadata disclosure, atomic
one-time claiming, identity checks, fail-closed mode changes, pilot entitlement
boundary reconciliation, paused-policy preservation, unwatch revocation, API
security headers, bearer-only status polling, and Telegram command parsing.
