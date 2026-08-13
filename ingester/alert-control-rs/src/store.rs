use crate::{
    normalize_wallet_address, pairing_token_hash, unix_timestamp, BillingMode, PolicyState,
    PAIRING_TTL_SECONDS,
};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

const SCHEMA_VERSION: &str = "1";

#[derive(Clone, Debug)]
pub struct Store {
    path: Arc<PathBuf>,
    billing_mode: BillingMode,
    pilot_days: u32,
}

#[derive(Clone, Debug)]
pub struct NewPairingClaim {
    pub token_hash: [u8; 32],
    pub wallet_address: String,
    pub critical_alerts: bool,
    pub non_critical_alerts: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingClaimStatus {
    pub wallet_address: String,
    pub critical_alerts: bool,
    pub non_critical_alerts: bool,
    pub created_at: i64,
    pub expires_at: i64,
    pub claimed_at: Option<i64>,
    pub policy_state: Option<PolicyState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TelegramIdentity {
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClaimPairingOutcome {
    Claimed {
        wallet_address: String,
        policy_state: PolicyState,
    },
    AlreadyClaimed,
    Expired,
    Unknown,
    IdentityConflict,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicySummary {
    pub wallet_address: String,
    pub critical_alerts: bool,
    pub non_critical_alerts: bool,
    pub state: PolicyState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PausePolicyOutcome {
    Paused,
    NotFound,
    IdentityConflict,
}

impl Store {
    pub fn initialize(
        path: impl AsRef<Path>,
        billing_mode: BillingMode,
        pilot_days: u32,
    ) -> Result<Self> {
        if pilot_days == 0 || pilot_days > 365 {
            bail!("pilot days must be between 1 and 365");
        }
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "creating alert-control database directory {}",
                    parent.display()
                )
            })?;
        }
        let store = Self {
            path: Arc::new(path),
            billing_mode,
            pilot_days,
        };
        let connection = store.open_connection()?;
        initialize_schema(&connection)?;
        store.reconcile_entitlements(unix_timestamp()?)?;
        Ok(store)
    }

    pub fn billing_mode(&self) -> BillingMode {
        self.billing_mode
    }

    pub fn database_path(&self) -> &Path {
        self.path.as_ref()
    }

    pub fn create_pairing_claim(&self, claim: &NewPairingClaim) -> Result<i64> {
        if !claim.critical_alerts && !claim.non_critical_alerts {
            bail!("at least one alert severity must be enabled");
        }
        let wallet_address = normalize_wallet_address(&claim.wallet_address)?;
        let expires_at = claim
            .created_at
            .checked_add(PAIRING_TTL_SECONDS)
            .context("pairing expiration timestamp overflow")?;
        let connection = self.open_connection()?;
        connection.execute(
            "DELETE FROM pairing_claims
             WHERE claimed_at IS NULL AND expires_at < ?1",
            params![claim.created_at.saturating_sub(86_400)],
        )?;
        connection.execute(
            "INSERT INTO pairing_claims(
                 token_hash, wallet_address, critical_enabled,
                 non_critical_enabled, created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &claim.token_hash[..],
                wallet_address,
                claim.critical_alerts,
                claim.non_critical_alerts,
                claim.created_at,
                expires_at,
            ],
        )?;
        Ok(expires_at)
    }

    pub fn pairing_claim_status(
        &self,
        raw_token: &str,
        now: i64,
    ) -> Result<Option<PairingClaimStatus>> {
        let token_hash = pairing_token_hash(raw_token)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.reconcile_entitlements_in_transaction(&transaction, now)?;
        let status = transaction
            .query_row(
                "SELECT c.wallet_address, c.critical_enabled,
                        c.non_critical_enabled, c.created_at, c.expires_at,
                        c.claimed_at, p.status
                 FROM pairing_claims c
                 LEFT JOIN alert_policies p ON p.id = c.alert_policy_id
                 WHERE c.token_hash = ?1 AND c.expires_at > ?2",
                params![&token_hash[..], now],
                |row| {
                    let policy_state: Option<String> = row.get(6)?;
                    Ok(PairingClaimStatus {
                        wallet_address: row.get(0)?,
                        critical_alerts: row.get(1)?,
                        non_critical_alerts: row.get(2)?,
                        created_at: row.get(3)?,
                        expires_at: row.get(4)?,
                        claimed_at: row.get(5)?,
                        policy_state: policy_state
                            .as_deref()
                            .map(PolicyState::from_db)
                            .transpose()
                            .map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    6,
                                    rusqlite::types::Type::Text,
                                    error.into(),
                                )
                            })?,
                    })
                },
            )
            .optional()
            .context("reading pairing claim")?;
        transaction.commit()?;
        Ok(status)
    }

    pub fn claim_pairing(
        &self,
        raw_token: &str,
        identity: &TelegramIdentity,
        now: i64,
    ) -> Result<ClaimPairingOutcome> {
        validate_telegram_identity(identity)?;
        let token_hash = pairing_token_hash(raw_token)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

        let claim = transaction
            .query_row(
                "SELECT wallet_address, critical_enabled, non_critical_enabled,
                        expires_at, claimed_at
                 FROM pairing_claims WHERE token_hash = ?1",
                params![&token_hash[..]],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, bool>(1)?,
                        row.get::<_, bool>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<i64>>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((wallet_address, critical_alerts, non_critical_alerts, expires_at, claimed_at)) =
            claim
        else {
            transaction.commit()?;
            return Ok(ClaimPairingOutcome::Unknown);
        };
        if claimed_at.is_some() {
            transaction.commit()?;
            return Ok(ClaimPairingOutcome::AlreadyClaimed);
        }
        if expires_at <= now {
            transaction.commit()?;
            return Ok(ClaimPairingOutcome::Expired);
        }

        let existing_chat = transaction
            .query_row(
                "SELECT id, telegram_user_id FROM telegram_links
                 WHERE telegram_chat_id = ?1",
                params![identity.chat_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        let existing_user = transaction
            .query_row(
                "SELECT id, telegram_chat_id FROM telegram_links
                 WHERE telegram_user_id = ?1",
                params![identity.user_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        if existing_chat
            .as_ref()
            .is_some_and(|(_, user_id)| *user_id != identity.user_id)
            || existing_user
                .as_ref()
                .is_some_and(|(_, chat_id)| *chat_id != identity.chat_id)
        {
            transaction.commit()?;
            return Ok(ClaimPairingOutcome::IdentityConflict);
        }

        let telegram_link_id = match existing_chat {
            Some((id, _)) => {
                transaction.execute(
                    "UPDATE telegram_links
                     SET telegram_username = ?1, active = 1, updated_at = ?2
                     WHERE id = ?3",
                    params![identity.username, now, id],
                )?;
                id
            }
            None => {
                transaction.execute(
                    "INSERT INTO telegram_links(
                         telegram_chat_id, telegram_user_id, telegram_username,
                         active, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                    params![identity.chat_id, identity.user_id, identity.username, now],
                )?;
                transaction.last_insert_rowid()
            }
        };

        let policy_state = self.billing_mode.policy_state();
        transaction.execute(
            "INSERT INTO alert_policies(
                 telegram_link_id, wallet_address, critical_enabled,
                 non_critical_enabled, delivery, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, 'telegram', ?5, ?6, ?6)
             ON CONFLICT(telegram_link_id, wallet_address) DO UPDATE SET
                 critical_enabled = excluded.critical_enabled,
                 non_critical_enabled = excluded.non_critical_enabled,
                 status = excluded.status,
                 updated_at = excluded.updated_at",
            params![
                telegram_link_id,
                wallet_address,
                critical_alerts,
                non_critical_alerts,
                policy_state.as_db(),
                now,
            ],
        )?;
        let policy_id: i64 = transaction.query_row(
            "SELECT id FROM alert_policies
             WHERE telegram_link_id = ?1 AND wallet_address = ?2",
            params![telegram_link_id, wallet_address],
            |row| row.get(0),
        )?;

        match self.billing_mode {
            BillingMode::Pilot => {
                let ends_at = now
                    .checked_add(i64::from(self.pilot_days) * 86_400)
                    .context("pilot entitlement timestamp overflow")?;
                transaction.execute(
                    "INSERT INTO entitlements(
                         alert_policy_id, kind, status, source,
                         starts_at, ends_at, created_at, updated_at
                     ) VALUES (?1, 'pilot', 'active', 'operator_config', ?2, ?3, ?2, ?2)
                     ON CONFLICT(alert_policy_id, kind) DO UPDATE SET
                         status = 'active', source = 'operator_config',
                         starts_at = excluded.starts_at, ends_at = excluded.ends_at,
                         updated_at = excluded.updated_at",
                    params![policy_id, now, ends_at],
                )?;
            }
            BillingMode::Unconfigured => {
                transaction.execute(
                    "UPDATE entitlements SET status = 'revoked', updated_at = ?1
                     WHERE alert_policy_id = ?2 AND status = 'active'",
                    params![now, policy_id],
                )?;
            }
        }

        let updated = transaction.execute(
            "UPDATE pairing_claims
             SET claimed_at = ?1, telegram_link_id = ?2, alert_policy_id = ?3
             WHERE token_hash = ?4 AND claimed_at IS NULL AND expires_at > ?1",
            params![now, telegram_link_id, policy_id, &token_hash[..]],
        )?;
        if updated != 1 {
            bail!("pairing claim lost its atomic claim race");
        }
        transaction.commit()?;

        Ok(ClaimPairingOutcome::Claimed {
            wallet_address,
            policy_state,
        })
    }

    pub fn list_policies(
        &self,
        identity: &TelegramIdentity,
        now: i64,
    ) -> Result<Vec<PolicySummary>> {
        validate_telegram_identity(identity)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.reconcile_entitlements_in_transaction(&transaction, now)?;
        let mut statement = transaction.prepare(
            "SELECT p.wallet_address, p.critical_enabled,
                    p.non_critical_enabled, p.status
             FROM alert_policies p
             JOIN telegram_links t ON t.id = p.telegram_link_id
             WHERE t.telegram_chat_id = ?1 AND t.telegram_user_id = ?2
             ORDER BY p.updated_at DESC, p.wallet_address
             LIMIT 50",
        )?;
        let policies = statement
            .query_map(params![identity.chat_id, identity.user_id], |row| {
                let state: String = row.get(3)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                    state,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let summaries = policies
            .into_iter()
            .map(
                |(wallet_address, critical_alerts, non_critical_alerts, state)| {
                    Ok(PolicySummary {
                        wallet_address,
                        critical_alerts,
                        non_critical_alerts,
                        state: PolicyState::from_db(&state)?,
                    })
                },
            )
            .collect::<Result<Vec<_>>>()?;
        drop(statement);
        transaction.commit()?;
        Ok(summaries)
    }

    pub fn pause_policy(
        &self,
        identity: &TelegramIdentity,
        wallet_address: &str,
        now: i64,
    ) -> Result<PausePolicyOutcome> {
        validate_telegram_identity(identity)?;
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let link_id = transaction
            .query_row(
                "SELECT id FROM telegram_links
                 WHERE telegram_chat_id = ?1 AND telegram_user_id = ?2",
                params![identity.chat_id, identity.user_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(link_id) = link_id else {
            let chat_exists = transaction
                .query_row(
                    "SELECT 1 FROM telegram_links WHERE telegram_chat_id = ?1",
                    params![identity.chat_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            transaction.commit()?;
            return Ok(if chat_exists {
                PausePolicyOutcome::IdentityConflict
            } else {
                PausePolicyOutcome::NotFound
            });
        };
        let policy_id = transaction
            .query_row(
                "SELECT id FROM alert_policies
                 WHERE telegram_link_id = ?1 AND wallet_address = ?2",
                params![link_id, wallet_address],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let Some(policy_id) = policy_id else {
            transaction.commit()?;
            return Ok(PausePolicyOutcome::NotFound);
        };
        transaction.execute(
            "UPDATE alert_policies SET status = 'paused', updated_at = ?1 WHERE id = ?2",
            params![now, policy_id],
        )?;
        transaction.execute(
            "UPDATE entitlements SET status = 'revoked', updated_at = ?1
             WHERE alert_policy_id = ?2 AND status = 'active'",
            params![now, policy_id],
        )?;
        transaction.commit()?;
        Ok(PausePolicyOutcome::Paused)
    }

    pub fn reconcile_entitlements(&self, now: i64) -> Result<()> {
        let mut connection = self.open_connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        self.reconcile_entitlements_in_transaction(&transaction, now)?;
        transaction.commit()?;
        Ok(())
    }

    fn reconcile_entitlements_in_transaction(
        &self,
        transaction: &rusqlite::Transaction<'_>,
        now: i64,
    ) -> Result<()> {
        match self.billing_mode {
            BillingMode::Pilot => {
                transaction.execute(
                    "UPDATE entitlements
                     SET status = 'expired', updated_at = ?1
                     WHERE status = 'active' AND ends_at <= ?1",
                    params![now],
                )?;
            }
            BillingMode::Unconfigured => {
                transaction.execute(
                    "UPDATE entitlements
                     SET status = 'revoked', updated_at = ?1
                     WHERE status = 'active'",
                    params![now],
                )?;
            }
        }
        transaction.execute(
            "UPDATE alert_policies
             SET status = 'awaiting_entitlement', updated_at = ?1
             WHERE status = 'active'
               AND NOT EXISTS (
                   SELECT 1 FROM entitlements e
                   WHERE e.alert_policy_id = alert_policies.id
                     AND e.status = 'active'
                     AND e.starts_at <= ?1
                     AND e.ends_at > ?1
               )",
            params![now],
        )?;
        Ok(())
    }

    fn open_connection(&self) -> Result<Connection> {
        let connection = Connection::open_with_flags(
            self.path.as_ref(),
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("opening alert-control database {}", self.path.display()))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(connection)
    }
}

fn validate_telegram_identity(identity: &TelegramIdentity) -> Result<()> {
    if identity.chat_id <= 0 || identity.user_id <= 0 || identity.chat_id != identity.user_id {
        bail!("Telegram pairing requires a private user chat");
    }
    if let Some(username) = identity.username.as_deref() {
        if username.is_empty()
            || username.len() > 32
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            bail!("invalid Telegram username");
        }
    }
    Ok(())
}

fn initialize_schema(connection: &Connection) -> Result<()> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS alert_control_meta(
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS telegram_links(
             id INTEGER PRIMARY KEY,
             telegram_chat_id INTEGER NOT NULL UNIQUE,
             telegram_user_id INTEGER NOT NULL UNIQUE,
             telegram_username TEXT,
             active INTEGER NOT NULL CHECK(active IN (0, 1)),
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS alert_policies(
             id INTEGER PRIMARY KEY,
             telegram_link_id INTEGER NOT NULL REFERENCES telegram_links(id),
             wallet_address TEXT NOT NULL,
             critical_enabled INTEGER NOT NULL CHECK(critical_enabled IN (0, 1)),
             non_critical_enabled INTEGER NOT NULL CHECK(non_critical_enabled IN (0, 1)),
             delivery TEXT NOT NULL CHECK(delivery = 'telegram'),
             status TEXT NOT NULL CHECK(status IN (
                 'awaiting_entitlement', 'active', 'paused'
             )),
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             UNIQUE(telegram_link_id, wallet_address),
             CHECK(critical_enabled = 1 OR non_critical_enabled = 1)
         );
         CREATE TABLE IF NOT EXISTS entitlements(
             id INTEGER PRIMARY KEY,
             alert_policy_id INTEGER NOT NULL REFERENCES alert_policies(id),
             kind TEXT NOT NULL CHECK(kind = 'pilot'),
             status TEXT NOT NULL CHECK(status IN ('active', 'expired', 'revoked')),
             source TEXT NOT NULL CHECK(source = 'operator_config'),
             starts_at INTEGER NOT NULL,
             ends_at INTEGER NOT NULL,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL,
             UNIQUE(alert_policy_id, kind),
             CHECK(ends_at > starts_at)
         );
         CREATE TABLE IF NOT EXISTS pairing_claims(
             token_hash BLOB PRIMARY KEY CHECK(length(token_hash) = 32),
             wallet_address TEXT NOT NULL,
             critical_enabled INTEGER NOT NULL CHECK(critical_enabled IN (0, 1)),
             non_critical_enabled INTEGER NOT NULL CHECK(non_critical_enabled IN (0, 1)),
             created_at INTEGER NOT NULL,
             expires_at INTEGER NOT NULL,
             claimed_at INTEGER,
             telegram_link_id INTEGER REFERENCES telegram_links(id),
             alert_policy_id INTEGER REFERENCES alert_policies(id),
             CHECK(critical_enabled = 1 OR non_critical_enabled = 1),
             CHECK(expires_at > created_at),
             CHECK(
                 (claimed_at IS NULL AND telegram_link_id IS NULL AND alert_policy_id IS NULL)
                 OR
                 (claimed_at IS NOT NULL AND telegram_link_id IS NOT NULL AND alert_policy_id IS NOT NULL)
             )
         );
         CREATE INDEX IF NOT EXISTS pairing_claims_expiration
             ON pairing_claims(expires_at) WHERE claimed_at IS NULL;
         CREATE INDEX IF NOT EXISTS alert_policies_by_link
             ON alert_policies(telegram_link_id, updated_at DESC);",
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO alert_control_meta(key, value)
         VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION],
    )?;
    let version: String = connection.query_row(
        "SELECT value FROM alert_control_meta WHERE key = 'schema_version'",
        [],
        |row| row.get(0),
    )?;
    if version != SCHEMA_VERSION {
        bail!("unsupported alert-control schema version {version}; expected {SCHEMA_VERSION}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{normalize_wallet_address, pairing_token_hash};
    use stellar_xdr::{MuxedAccount, Uint256};
    use tempfile::TempDir;

    fn fixture(mode: BillingMode) -> (TempDir, Store, String) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("control.sqlite");
        let store = Store::initialize(&path, mode, 30).unwrap();
        let wallet =
            normalize_wallet_address(&MuxedAccount::Ed25519(Uint256([9; 32])).to_string()).unwrap();
        (temp, store, wallet)
    }

    fn insert_claim(store: &Store, wallet: &str, token: &str, now: i64) -> i64 {
        store
            .create_pairing_claim(&NewPairingClaim {
                token_hash: pairing_token_hash(token).unwrap(),
                wallet_address: wallet.to_owned(),
                critical_alerts: true,
                non_critical_alerts: false,
                created_at: now,
            })
            .unwrap()
    }

    fn identity() -> TelegramIdentity {
        TelegramIdentity {
            chat_id: 42,
            user_id: 42,
            username: Some("stargazer_user".to_owned()),
        }
    }

    #[test]
    fn database_stores_only_the_token_hash() {
        let (_temp, store, wallet) = fixture(BillingMode::Unconfigured);
        let token = "12".repeat(32);
        insert_claim(&store, &wallet, &token, 1_000);

        let connection = Connection::open(store.database_path()).unwrap();
        let stored: Vec<u8> = connection
            .query_row("SELECT token_hash FROM pairing_claims", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, pairing_token_hash(&token).unwrap());
        let raw_occurrences: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM pairing_claims WHERE CAST(token_hash AS TEXT) = ?1",
                params![token],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw_occurrences, 0);
    }

    #[test]
    fn pilot_claim_is_atomic_and_issues_a_bounded_entitlement() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let token = "34".repeat(32);
        let expires_at = insert_claim(&store, &wallet, &token, 1_000);
        assert_eq!(expires_at, 1_000 + PAIRING_TTL_SECONDS);

        assert_eq!(
            store.claim_pairing(&token, &identity(), 1_001).unwrap(),
            ClaimPairingOutcome::Claimed {
                wallet_address: wallet.clone(),
                policy_state: PolicyState::Active,
            }
        );
        assert_eq!(
            store.claim_pairing(&token, &identity(), 1_002).unwrap(),
            ClaimPairingOutcome::AlreadyClaimed
        );
        let status = store.pairing_claim_status(&token, 1_002).unwrap().unwrap();
        assert_eq!(status.policy_state, Some(PolicyState::Active));

        let connection = Connection::open(store.database_path()).unwrap();
        let entitlement: (String, String, i64) = connection
            .query_row(
                "SELECT kind, status, ends_at - starts_at FROM entitlements",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            entitlement,
            ("pilot".to_owned(), "active".to_owned(), 30 * 86_400)
        );
    }

    #[test]
    fn unconfigured_billing_never_invents_an_entitlement() {
        let (_temp, store, wallet) = fixture(BillingMode::Unconfigured);
        let token = "56".repeat(32);
        insert_claim(&store, &wallet, &token, 1_000);
        let outcome = store.claim_pairing(&token, &identity(), 1_001).unwrap();
        assert_eq!(
            outcome,
            ClaimPairingOutcome::Claimed {
                wallet_address: wallet,
                policy_state: PolicyState::AwaitingEntitlement,
            }
        );
        let connection = Connection::open(store.database_path()).unwrap();
        let entitlements: i64 = connection
            .query_row("SELECT COUNT(*) FROM entitlements", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entitlements, 0);
    }

    #[test]
    fn expired_and_wrong_identity_claims_are_rejected() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let expired = "78".repeat(32);
        insert_claim(&store, &wallet, &expired, 1_000);
        assert_eq!(
            store
                .claim_pairing(&expired, &identity(), 1_000 + PAIRING_TTL_SECONDS)
                .unwrap(),
            ClaimPairingOutcome::Expired
        );

        let valid = "9a".repeat(32);
        insert_claim(&store, &wallet, &valid, 2_000);
        let invalid_identity = TelegramIdentity {
            chat_id: 99,
            user_id: 100,
            username: None,
        };
        assert!(store
            .claim_pairing(&valid, &invalid_identity, 2_001)
            .is_err());
    }

    #[test]
    fn claimed_pairing_status_is_hidden_at_expiry_boundary() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let token = "ab".repeat(32);
        let expires_at = insert_claim(&store, &wallet, &token, 1_000);
        store.claim_pairing(&token, &identity(), 1_001).unwrap();

        let before_expiry = store
            .pairing_claim_status(&token, expires_at - 1)
            .unwrap()
            .unwrap();
        assert_eq!(before_expiry.wallet_address, wallet);
        assert_eq!(before_expiry.policy_state, Some(PolicyState::Active));
        assert!(store
            .pairing_claim_status(&token, expires_at)
            .unwrap()
            .is_none());
    }

    #[test]
    fn pilot_entitlement_is_valid_through_end_minus_one_and_expires_at_end() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let token = "cd".repeat(32);
        insert_claim(&store, &wallet, &token, 1_000);
        store.claim_pairing(&token, &identity(), 1_001).unwrap();
        let connection = Connection::open(store.database_path()).unwrap();
        let ends_at: i64 = connection
            .query_row("SELECT ends_at FROM entitlements", [], |row| row.get(0))
            .unwrap();
        drop(connection);

        assert_eq!(
            store.list_policies(&identity(), ends_at - 1).unwrap()[0].state,
            PolicyState::Active
        );
        assert_eq!(
            store.list_policies(&identity(), ends_at).unwrap()[0].state,
            PolicyState::AwaitingEntitlement
        );
        let connection = Connection::open(store.database_path()).unwrap();
        let entitlement_status: String = connection
            .query_row("SELECT status FROM entitlements", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entitlement_status, "expired");
    }

    #[test]
    fn reopening_in_unconfigured_mode_revokes_pilot_and_demotes_active_policy() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("control.sqlite");
        let pilot = Store::initialize(&path, BillingMode::Pilot, 30).unwrap();
        let wallet =
            normalize_wallet_address(&MuxedAccount::Ed25519(Uint256([10; 32])).to_string())
                .unwrap();
        let token = "de".repeat(32);
        let now = unix_timestamp().unwrap();
        insert_claim(&pilot, &wallet, &token, now);
        pilot.claim_pairing(&token, &identity(), now + 1).unwrap();
        drop(pilot);

        let unconfigured = Store::initialize(&path, BillingMode::Unconfigured, 30).unwrap();
        assert_eq!(
            unconfigured.list_policies(&identity(), now + 2).unwrap()[0].state,
            PolicyState::AwaitingEntitlement
        );
        let connection = Connection::open(unconfigured.database_path()).unwrap();
        let entitlement_status: String = connection
            .query_row("SELECT status FROM entitlements", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entitlement_status, "revoked");
    }

    #[test]
    fn reconciliation_never_reactivates_or_demotes_a_paused_policy() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let token = "ef".repeat(32);
        insert_claim(&store, &wallet, &token, 1_000);
        store.claim_pairing(&token, &identity(), 1_001).unwrap();
        store.pause_policy(&identity(), &wallet, 1_002).unwrap();

        store.reconcile_entitlements(i64::MAX).unwrap();
        assert_eq!(
            store.list_policies(&identity(), i64::MAX).unwrap()[0].state,
            PolicyState::Paused
        );
    }

    #[test]
    fn unwatch_pauses_the_policy_and_revokes_pilot() {
        let (_temp, store, wallet) = fixture(BillingMode::Pilot);
        let token = "bc".repeat(32);
        insert_claim(&store, &wallet, &token, 1_000);
        store.claim_pairing(&token, &identity(), 1_001).unwrap();
        assert_eq!(
            store.pause_policy(&identity(), &wallet, 1_002).unwrap(),
            PausePolicyOutcome::Paused
        );
        assert_eq!(
            store.list_policies(&identity(), 1_003).unwrap()[0].state,
            PolicyState::Paused
        );
        let connection = Connection::open(store.database_path()).unwrap();
        let entitlement: String = connection
            .query_row("SELECT status FROM entitlements", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entitlement, "revoked");
    }
}
