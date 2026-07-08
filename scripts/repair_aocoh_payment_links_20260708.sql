-- Repair AoCoH payment-link state after the June 12 mass-mark bug.
--
-- Run after taking a backup. In Dokploy, the backend runtime image does not
-- include sqlite3, but the backup profile image does and mounts the same DB:
--
--   docker compose --profile backup run --rm firewatch-r2-backup
--   docker compose --profile backup run --rm --entrypoint sqlite3 firewatch-r2-backup /data/indexer.db < scripts/repair_aocoh_payment_links_20260708.sql
--
-- Verified real on-chain payments:
-- - e13d596d-b95a-41a4-9237-5c1c92c14a42 at 2026-06-01T12:55:41Z
-- - de5aa949-38e2-4f1c-916f-bf7186ec3343 at 2026-06-12T07:02:30Z
--
-- False positives from the old mass-mark helper:
-- - b406e925-944a-4a11-8c03-30181eee5758
-- - 61ca09bc-1e5c-4906-91f7-4b0fd1f294cf
-- - 294df2c8-ca02-4e20-b6b7-76932309d30e
-- - 4e546b62-b2a0-4f7b-b9d0-5cabec8557b9
-- - 28dcb836-172f-449b-abba-4b55dabbbe6e

.bail on

BEGIN IMMEDIATE;

CREATE TEMP TABLE firewatch_repair_assertions (
    ok INTEGER NOT NULL CHECK (ok = 1),
    message TEXT NOT NULL
);

INSERT INTO firewatch_repair_assertions
SELECT COUNT(*) = 1, 'expected AoCoH user'
FROM firewatch_users
WHERE id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND lower(telegram_username) = 'aocoh';

INSERT INTO firewatch_repair_assertions
SELECT COUNT(*) = 7, 'expected seven target payment links for AoCoH'
FROM payment_links
WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND id IN (
    'e13d596d-b95a-41a4-9237-5c1c92c14a42',
    'de5aa949-38e2-4f1c-916f-bf7186ec3343',
    'b406e925-944a-4a11-8c03-30181eee5758',
    '61ca09bc-1e5c-4906-91f7-4b0fd1f294cf',
    '294df2c8-ca02-4e20-b6b7-76932309d30e',
    '4e546b62-b2a0-4f7b-b9d0-5cabec8557b9',
    '28dcb836-172f-449b-abba-4b55dabbbe6e'
  );

INSERT INTO firewatch_repair_assertions
SELECT COUNT(*) = 5, 'false-positive links must be paid at the mass timestamp or already reset'
FROM payment_links
WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND id IN (
    'b406e925-944a-4a11-8c03-30181eee5758',
    '61ca09bc-1e5c-4906-91f7-4b0fd1f294cf',
    '294df2c8-ca02-4e20-b6b7-76932309d30e',
    '4e546b62-b2a0-4f7b-b9d0-5cabec8557b9',
    '28dcb836-172f-449b-abba-4b55dabbbe6e'
  )
  AND (
    (status = 'paid' AND paid_at = 1781247764)
    OR (status = 'pending' AND paid_at IS NULL)
  );

INSERT INTO firewatch_repair_assertions
SELECT COUNT(*) = 1, 'June 1 real payment link must be paid and repairable'
FROM payment_links
WHERE id = 'e13d596d-b95a-41a4-9237-5c1c92c14a42'
  AND user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND status = 'paid'
  AND paid_at IN (1781247764, 1780318541);

INSERT INTO firewatch_repair_assertions
SELECT COUNT(*) = 1, 'June 12 real payment link must be paid and repairable'
FROM payment_links
WHERE id = 'de5aa949-38e2-4f1c-916f-bf7186ec3343'
  AND user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND status = 'paid'
  AND paid_at IN (1781247764, 1781247750);

UPDATE payment_links
SET paid_at = 1780318541
WHERE id = 'e13d596d-b95a-41a4-9237-5c1c92c14a42'
  AND user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND status = 'paid'
  AND paid_at = 1781247764;

UPDATE payment_links
SET paid_at = 1781247750
WHERE id = 'de5aa949-38e2-4f1c-916f-bf7186ec3343'
  AND user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND status = 'paid'
  AND paid_at = 1781247764;

UPDATE payment_links
SET status = 'pending',
    paid_at = NULL
WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
  AND status = 'paid'
  AND paid_at = 1781247764
  AND id IN (
    'b406e925-944a-4a11-8c03-30181eee5758',
    '61ca09bc-1e5c-4906-91f7-4b0fd1f294cf',
    '294df2c8-ca02-4e20-b6b7-76932309d30e',
    '4e546b62-b2a0-4f7b-b9d0-5cabec8557b9',
    '28dcb836-172f-449b-abba-4b55dabbbe6e'
  );

INSERT INTO user_entitlements (
    id,
    user_id,
    plan_id,
    source,
    status,
    starts_at,
    expires_at,
    created_at,
    updated_at
)
SELECT
    'repair-aocoh-missing-jun01-month-20260708',
    '3cdb8823-cbfe-4e44-8193-87dba54f8261',
    'paid',
    'solana_pay_repair:aocoh-20260708',
    'active',
    latest.starts_at,
    latest.starts_at + 2592000,
    CAST(strftime('%s', 'now') AS INTEGER),
    CAST(strftime('%s', 'now') AS INTEGER)
FROM (
    SELECT COALESCE(MAX(expires_at), 1783839764) AS starts_at
    FROM user_entitlements
    WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
      AND status = 'active'
      AND plan_id IN ('paid', 'paid-year', 'paid_year', 'year', 'yearly', 'custom', 'pro', 'ferno-member')
      AND expires_at IS NOT NULL
) AS latest
WHERE NOT EXISTS (
    SELECT 1
    FROM user_entitlements existing_repair
    WHERE existing_repair.user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
      AND existing_repair.source = 'solana_pay_repair:aocoh-20260708'
);

COMMIT;

SELECT
    id,
    status,
    datetime(paid_at, 'unixepoch') AS paid_at_utc,
    amount
FROM payment_links
WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
ORDER BY created_at;

SELECT
    id,
    plan_id,
    source,
    status,
    datetime(starts_at, 'unixepoch') AS starts_at_utc,
    datetime(expires_at, 'unixepoch') AS expires_at_utc
FROM user_entitlements
WHERE user_id = '3cdb8823-cbfe-4e44-8193-87dba54f8261'
ORDER BY starts_at;
