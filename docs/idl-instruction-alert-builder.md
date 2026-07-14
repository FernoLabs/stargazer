# IDL-backed instruction alerts

## Goal

Use published program IDLs to recognize every supported instruction and let a user
create an alert for:

- every invocation of an instruction;
- a comparison against a decoded scalar argument; or
- a match against an instruction account or public-key argument.

The IDL supplies the ABI. FireWatch's curated rules and user rules supply the risk
meaning and severity.

## Current audit

The local IDL download contains 259 unique program IDs and 5,882 instructions after
preferring Program Metadata over a duplicate legacy Anchor IDL. The reusable audit is
`scripts/audit_idl_admin_candidates.py`.

The initial classification is intentionally tiered:

- `high_confidence_admin`: 261 instructions;
- `likely_admin`: 398 instructions;
- `manual_review`: 672 instructions;
- `not_admin`: 4,551 instructions.

Only `high_confidence_admin` entries emit automatic global admin signals in the first
release. Likely-admin and manual-review entries remain visible as catalog guidance and
can be selected by a user, but do not generate a global alert. The classifier combines the
instruction name, signer account roles, and sensitive argument names; it does not treat
generic verbs such as `initialize`, `withdraw`, or `close_position` as administrative by
themselves.

## Catalog model

Generate a compact, tracked catalog rather than shipping the raw downloaded IDLs:

```json
{
  "program_id": "...",
  "idl_origin": "program_metadata",
  "idl_sha256": "...",
  "instruction_name": "update_config",
  "discriminator_hex": "...",
  "accounts": [
    { "index": 0, "name": "config", "signer": false, "writable": true },
    { "index": 1, "name": "admin", "signer": true, "writable": false }
  ],
  "arguments": [
    { "index": 0, "name": "borrow_limit", "type": "u64", "category": "integer" }
  ]
}
```

For modern IDLs, use the explicit instruction discriminator. For legacy Anchor IDLs,
derive the discriminator from `sha256("global:" + snake_case_name)[0..8]`. Codama and
non-Anchor schemas must use their declared discriminator strategy rather than assuming
the Anchor hash.

The catalog must retain its IDL hash and source so decoded values can be traced to the
schema used at ingestion time.

## Decoded instruction model

The ingestion layer should produce a transient value before deciding whether to persist
an event:

```text
DecodedIdlInstruction
  program_id
  protocol_slug / protocol_name / program_label
  instruction_name / discriminator
  idl_origin / idl_sha256
  accounts: name -> pubkey
  arguments: name -> typed value
  transaction / instruction path / slot / actor
```

All catalogued instructions pass through this decoder and rule evaluator. FireWatch
should not persist every decoded user action: doing so for high-volume protocols would
create an unbounded event stream. Persist automatic admin signals and matched user
rules; optionally retain aggregate counts or a short-lived observation sample.

## Rule model

Instruction alert rules are user-owned and should not reuse the existing global
`metric_alert_rules` table.

```text
idl_instruction_alert_rules
  id
  user_id
  subscription_id
  label
  program_id
  instruction_name
  discriminator_hex
  idl_sha256
  severity               standard | high | critical
  condition_mode         all | any
  conditions_json
  enabled
  created_at / updated_at
```

An empty condition list means “alert whenever this instruction is invoked.”

Example conditions:

```json
{
  "all": [
    {
      "source": "argument",
      "field": "borrow_limit",
      "value_type": "u64",
      "operator": "gte",
      "value": "1000000000"
    },
    {
      "source": "account",
      "field": "admin",
      "value_type": "pubkey",
      "operator": "eq",
      "value": "AdminPubkey..."
    }
  ]
}
```

Store integer constants and decoded integer values as decimal strings, not SQLite
`REAL` or JSON floating-point numbers. Solana IDLs routinely use `u64`, `i64`, `u128`,
and `i128`, which cannot be compared safely through JavaScript or `f64` numbers.

## MVP operators

| Value | Operators |
|---|---|
| Signed/unsigned integer | `eq`, `ne`, `gt`, `gte`, `lt`, `lte`, `between` |
| Instruction account | `eq`, `ne` |
| Public-key argument | `eq`, `ne` |
| Boolean argument | `eq`, `ne` |
| No condition | always match |

Nested defined types, vectors, byte arrays, strings, enums, and floating-point arguments
should be a later phase. They require recursive type resolution and need clearer UX.

The current IDL set provides:

- 2,117 instructions with at least one integer argument;
- 444 with a public-key argument;
- 287 with a boolean argument;
- 5,837 with named instruction accounts; and
- all 5,882 instructions can support an unconditional severity rule.

## Severity and automatic admin candidates

Automatic classification and user severity are separate:

- A high-confidence automatic rule can create a global admin event after review.
- A likely-admin candidate should default to `high`, but remain reviewable before being
  activated globally.
- A manual-review candidate is discovery metadata only.
- A user may choose `standard`, `high`, or `critical` independently for their own rule.

Changing the IDL or upgrading the program must not silently change an existing rule.
If the stored `idl_sha256`, discriminator, account position, or argument type no longer
matches, mark the rule `needs_review` and stop evaluating its typed conditions until the
user confirms the new schema. An unconditional instruction rule can continue if the
same discriminator/name mapping remains valid.

## Privacy and delivery

The current `events` table is global, while instruction rules belong to individual
users. A rule match must be delivered only to the rule's linked subscription.

The first release stores global events only for high-confidence automatic admin
signals. A user rule match is stored as a hidden `program_instruction_alert` event so
the existing retry queue remains durable. It carries the rule owner and rule ID, and
delivery fails closed unless the subscription has both the same owner and the exact
`rule:<id>` requirement. Hidden rule matches are excluded from event collections, exact
event lookup, preview cards, daily summaries, and public FireWatch links.

## Implementation sequence

Implemented:

1. A deterministic, tracked catalog for 259 programs and 5,882 instructions.
2. Anchor and Codama decoding with exact integer strings, booleans, public keys, and
   named instruction accounts.
3. User-owned rule persistence and evaluator tests.
4. Catalog and rule CRUD APIs scoped through the Telegram config session.
5. A lazy-loaded builder UI with optional all/any conditions and severity selection.
6. Automatic global events for high-confidence IDL admin instructions.
7. IDL hash and discriminator pinning; a rule stops matching after incompatible catalog
   drift.

Follow-up work is to rebuild the catalog automatically after live IDL changes and show
stale rules explicitly as `needs_review` in the config UI.
