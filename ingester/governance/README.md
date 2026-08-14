# Governance watcher

This dependency-free Node 22 watcher polls Horizon for eight source-verified,
high-signal Classic controller accounts: the Aquarius root Admin, Phoenix
weighted Admin, five Reflector root Admins, and the YieldBlox Governor council.
It deliberately does not infer arbitrary Soroban contract governance.

Run one complete cycle:

```sh
node ingester/governance/watcher.mjs --once \
  --output-dir /var/lib/stargazer/governance/state
```

Omit `--once` to poll every 60 seconds. `--horizon-url`, `--interval-ms`,
`--timeout-ms`, `--registry`, and `--max-alerts` are configurable; matching
`STARGAZER_GOVERNANCE_*` environment variables are also supported. See
`node ingester/governance/watcher.mjs --help`.

The watcher validates each `G...` StrKey and checksum, fetches every account,
then canonicalizes low/medium/high thresholds, master weight, and the complete
sorted signer set. Policies remain explicitly classified as `strict`,
`weighted`, `single-key`, or `locked`; Phoenix is therefore never flattened to
a misleading 2-of-3 label. A newly tracked controller must match its curated
baseline classification. Once baselined, a transition to another valid
classification is recorded as a policy-change alert instead of disabling the
watcher.

Every fully successful cycle atomically replaces `current.json`. A semantic
policy transition after the first baseline creates a `critical` entry in the
bounded `alerts.json`. The watcher also checkpoints each account's Horizon
operation cursor. After that baseline, a successful operation whose effective
source is a tracked `strict` or `weighted` controller creates a `high` activity
alert containing the transaction hash, operation type, and a bounded normalized
operation payload, unless it matches an explicit routine-activity rule. Incoming
operations are checkpointed but do not create a controller alert.

Routine-activity suppression is fail-open and payload-based. The current rule
applies only to the three Reflector oracle-admin roles when every operation in
the transaction is an `invoke_host_function` whose canonical XDR function
symbol is exactly `set_price`. Those scheduled oracle publications advance the
cursor but are not retained as controller alerts. Other protocols, Reflector
roles, functions, mixed-operation transactions, and malformed or ambiguous
payloads remain alertable. Each cycle also removes already-retained records that
match the same rule, while preserving all other alert types and activity.

This is executed-transaction monitoring, not a pending multisig proposal feed.
Stellar only exposes a transaction to this watcher after it reaches Horizon;
unsigned or partially signed envelopes exchanged off-chain remain invisible.
The alert therefore says that the tracked controller sourced an operation that
was included successfully. It does not infer which named signers approved it.

Fetch errors, malformed payloads, or incomplete account/operation pages abort
the whole cycle without changing either file. Deterministic policy-transition
and controller-transaction IDs prevent duplicate alerts after a retry. The
first upgraded cycle only establishes activity cursors, so deploying this
feature cannot flood the alert register with historical transactions.

Point the read API at the same output directory to expose the current monitor
state on `/api/protocols`:

```sh
stargazer-read-api \
  --db /path/to/stargazer-one-year-compact.sqlite \
  --serving-db /path/to/stargazer-serving.sqlite \
  --governance-dir /var/lib/stargazer/governance/state
```

The equivalent environment variable is `STARGAZER_GOVERNANCE_DIR`. Missing
watcher output is reported as unavailable; incomplete or malformed output is
rejected instead of being served as a healthy baseline.

Run the tests with:

```sh
node --test ingester/governance/watcher.test.mjs
```
