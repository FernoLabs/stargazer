.bail on
.headers on
.mode column

-- Production cleanup for stale critical unknown-instruction rows.
--
-- Run from the backend container/host after taking a DB backup:
--   sqlite3 /data/indexer.db < scripts/cleanup_stale_unknown_instructions_20260707.sql
--
-- The backend now classifies these as known/suppressed:
-- - Meteora DLMM ClaimProtocolFee2: known admin, non-high-impact.
-- - Raydium CPMM InitializeWithPermission: known admin, high-impact.
-- - Raydium CPMM creator fee collection: known admin, non-high-impact.
-- - Raydium CLMM limit-order and customizable-pool instructions: known.
-- - Raydium AMM v4 swap/fee tags: known one-byte legacy tags.
-- - Kamino TopupReserveRewards: known non-signal rewards top-up.
-- - Legacy Anchor IDL staging: hidden staging noise. The retained unknown
--   discriminator is the shared legacy IDL tag; the variant byte is not kept in
--   old program_unknown_instruction payloads. These rows are kept for audit,
--   but removed from critical/unknown feeds.
-- - Plain Squads config executions: retain the events, remove obsolete critical
--   and DeFi-watch tags that predate the current routing rules.

BEGIN IMMEDIATE;

CREATE TEMP TABLE firewatch_stale_unknown_instruction_events (
    id TEXT PRIMARY KEY,
    reason TEXT NOT NULL
);

CREATE TEMP TABLE firewatch_hidden_anchor_idl_unknown_events (
    id TEXT PRIMARY KEY,
    reason TEXT NOT NULL
);

INSERT OR IGNORE INTO firewatch_stale_unknown_instruction_events (id, reason)
SELECT id, 'meteora_dlmm_claim_protocol_fee2'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address = 'LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo'
  AND json_extract(payload_json, '$.discriminator') = 'ebc23645410aec70';

INSERT OR IGNORE INTO firewatch_stale_unknown_instruction_events (id, reason)
SELECT id, 'raydium_cpmm_known_instruction'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address = 'CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C'
  AND json_extract(payload_json, '$.discriminator') IN (
      '1416567bc61cdb84',
      '3f37fe4131b25979'
  );

INSERT OR IGNORE INTO firewatch_stale_unknown_instruction_events (id, reason)
SELECT id, 'raydium_clmm_known_instruction'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address = 'CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK'
  AND json_extract(payload_json, '$.discriminator') IN (
      '2b44d4a7592fa401',
      '4c7c800fd55725fa',
      '759d3c674231a300',
      '9d20dab7471d1293',
      'cd4e74215c691a60'
  );

INSERT OR IGNORE INTO firewatch_stale_unknown_instruction_events (id, reason)
SELECT id, 'raydium_amm_v4_known_tag'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address = '675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8'
  AND lower(substr(json_extract(payload_json, '$.discriminator'), 1, 2)) IN (
      '07',
      '09',
      '0b',
      '10',
      '11'
  );

INSERT OR IGNORE INTO firewatch_stale_unknown_instruction_events (id, reason)
SELECT id, 'kamino_topup_reserve_rewards'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address = 'KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD'
  AND json_extract(payload_json, '$.discriminator') = '3fff82d36ed858ad';

INSERT OR IGNORE INTO firewatch_hidden_anchor_idl_unknown_events (id, reason)
SELECT id, 'legacy_anchor_idl_staging'
FROM events
WHERE kind = 'program_unknown_instruction'
  AND target_address IN (
      'KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD',
      'LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo',
      'whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc'
  )
  AND json_extract(payload_json, '$.discriminator') = '40f4bc78a7e9690a';

SELECT reason, COUNT(*) AS matched_events
FROM firewatch_stale_unknown_instruction_events
GROUP BY reason
ORDER BY reason;

SELECT reason, COUNT(*) AS matched_hidden_anchor_events
FROM firewatch_hidden_anchor_idl_unknown_events
GROUP BY reason
ORDER BY reason;

UPDATE events
SET
    summary = 'Hidden legacy Anchor IDL staging on ' || target_address,
    tags_json = (
        SELECT COALESCE(json_group_array(value), '[]')
        FROM (
            SELECT value
            FROM json_each(events.tags_json)
            WHERE value NOT IN (
                'alert:review_now',
                'impact:high',
                'sector:defi',
                'severity:critical',
                'signal:unknown_instruction',
                'watch:defi'
            )
            ORDER BY key
        )
    ),
    payload_json = json_set(
        payload_json,
        '$.classification', 'hidden',
        '$.high_impact', json('false'),
        '$.instruction_name', 'legacy_anchor_idl_staging',
        '$.severity', 'hidden'
    )
WHERE id IN (SELECT id FROM firewatch_hidden_anchor_idl_unknown_events);

SELECT changes() AS hidden_anchor_events;

CREATE TEMP TABLE firewatch_stale_squads_critical_events AS
SELECT id
FROM events
WHERE kind = 'multisig_config_executed'
  AND tags_json LIKE '%"severity:critical"%'
  AND tags_json LIKE '%"protocol:squads"%'
  AND tags_json LIKE '%"signal:multisig_config_change"%'
  AND tags_json NOT LIKE '%"multisig:project_linked"%';

SELECT COUNT(*) AS matched_squads_critical_events
FROM firewatch_stale_squads_critical_events;

UPDATE events
SET tags_json = (
    SELECT COALESCE(json_group_array(value), '[]')
    FROM (
        SELECT value
        FROM json_each(events.tags_json)
        WHERE value NOT IN (
            'alert:review_now',
            'impact:high',
            'sector:defi',
            'severity:critical',
            'signal:admin_instruction',
            'watch:defi'
        )
        ORDER BY key
    )
)
WHERE id IN (SELECT id FROM firewatch_stale_squads_critical_events);

SELECT changes() AS normalized_squads_events;

DELETE FROM telegram_deliveries
WHERE event_id IN (SELECT id FROM firewatch_stale_unknown_instruction_events);

SELECT changes() AS deleted_telegram_deliveries;

DELETE FROM events
WHERE id IN (SELECT id FROM firewatch_stale_unknown_instruction_events);

SELECT changes() AS deleted_events;

COMMIT;

SELECT COUNT(*) AS remaining_critical_stale_unknown_events
FROM events
WHERE kind = 'program_unknown_instruction'
  AND (
      tags_json LIKE '%"severity:critical"%'
      OR tags_json LIKE '%"signal:unknown_instruction"%'
  )
  AND (
      (
          target_address = 'LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo'
          AND json_extract(payload_json, '$.discriminator') IN (
              '40f4bc78a7e9690a',
              'ebc23645410aec70'
          )
      )
      OR (
          target_address = 'CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C'
          AND json_extract(payload_json, '$.discriminator') IN (
              '1416567bc61cdb84',
              '3f37fe4131b25979'
          )
      )
      OR (
          target_address = 'CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK'
          AND json_extract(payload_json, '$.discriminator') IN (
              '2b44d4a7592fa401',
              '4c7c800fd55725fa',
              '759d3c674231a300',
              '9d20dab7471d1293',
              'cd4e74215c691a60'
          )
      )
      OR (
          target_address = '675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8'
          AND lower(substr(json_extract(payload_json, '$.discriminator'), 1, 2)) IN (
              '07',
              '09',
              '0b',
              '10',
              '11'
          )
      )
      OR (
          target_address = 'KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD'
          AND json_extract(payload_json, '$.discriminator') IN (
              '3fff82d36ed858ad',
              '40f4bc78a7e9690a'
          )
      )
      OR (
          target_address = 'whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc'
          AND json_extract(payload_json, '$.discriminator') = '40f4bc78a7e9690a'
      )
  );

SELECT COUNT(*) AS remaining_squads_critical_events
FROM events
WHERE kind = 'multisig_config_executed'
  AND tags_json LIKE '%"severity:critical"%'
  AND tags_json LIKE '%"protocol:squads"%'
  AND tags_json LIKE '%"signal:multisig_config_change"%'
  AND tags_json NOT LIKE '%"multisig:project_linked"%';
