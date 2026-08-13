//! Versioned historical incident correlations over the compact archive.
//!
//! These rules do not claim that Stargazer fired a notification when the
//! incident happened. They reproduce a known, source-backed incident against
//! retained archive facts and deliberately keep delivery disabled.

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use stellar_xdr::{ContractId, MuxedAccount};

const YIELDBLOX_RULE_ID: &str = "yieldblox-oracle-manipulation-v1";
const YIELDBLOX_POOL: &str = "CCCCIQSDILITHMM7PBSLVDT5MISSY7R26MNZXCX4H7J5JQ5FPIYOGYFS";
const REFLECTOR_ORACLE: &str = "CALI2BYU2JE6WVRUFYTS6MSBNEHGJ35P4AVCZYF3B6QOE3QKOB2PLE6M";
const PRINCIPAL_BORROWER: &str = "GBO7VUL2TOKPWFAWKATIW7K3QYA7WQ63VDY5CAE6AFUUX6BHZBOC2WXC";
const RULE_START_LEDGER: u32 = 61_339_940;
const FINDING_LEDGER: u32 = 61_340_384;
const RULE_END_LEDGER: u32 = 61_340_408;
const FINDING_TIMESTAMP: &str = "2026-02-22T00:22:09Z";
// The compact schema currently has no transaction-hash index. These surrogate
// coordinates are bound to this one-year index layout and always verified
// against the canonical hashes before the rule can enable itself.
const SETUP_TRANSACTION_ID: i64 = 1_131_357_966;
const SETUP_TRANSACTION: &str = "09e1a9d1197c9bf0af4e87da328c4f2d5eb49b487630aa61991fb5c1c4637cdb";
const FINDING_TRANSACTION_ID: i64 = 1_131_484_622;
const FINDING_TRANSACTION: &str =
    "ae721cacee382bdecac8d2c47286ecd42cb4711f658bb2aec7cba60dc64a31ff";

const CHECKPOINTS: &[(u32, i64)] = &[
    (61_339_940, 1_771_717_131),
    (61_340_263, 1_771_719_021),
    (61_340_315, 1_771_719_322),
    (61_340_381, 1_771_719_711),
    (61_340_384, 1_771_719_729),
    (61_340_405, 1_771_719_850),
    (61_340_408, 1_771_719_867),
];

const DELIVERY_REASON: &str = "Historical replay is read-only; no notification was sent or queued.";

#[derive(Debug, Clone)]
pub struct HistoricalIncidentRules {
    available: bool,
    reason: String,
    pool_id: [u8; 32],
    principal_borrower: [u8; 32],
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletHistoricalFindings {
    pub available: bool,
    pub reason: String,
    pub delivery_available: bool,
    pub delivery_reason: &'static str,
    pub coverage: Option<HistoricalFindingCoverage>,
    pub items: Vec<HistoricalWalletFinding>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalFindingCoverage {
    pub from_ledger: u32,
    pub to_ledger: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalWalletFinding {
    pub id: String,
    pub kind: &'static str,
    pub severity: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub incident_at: &'static str,
    pub ledger_sequence: u32,
    pub protocol_id: &'static str,
    pub protocol_name: &'static str,
    pub contract_address: &'static str,
    pub transaction_hash: &'static str,
    pub origin: &'static str,
    pub match_type: &'static str,
    pub sources: Vec<HistoricalFindingSource>,
    pub deliveries: Vec<HistoricalFindingDelivery>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HistoricalFindingSource {
    pub label: &'static str,
    pub url: &'static str,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HistoricalFindingDelivery {
    pub channel: &'static str,
    pub status: &'static str,
}

impl HistoricalIncidentRules {
    pub fn load(
        connection: &Connection,
        indexed_ledger_start: u32,
        indexed_ledger_end: u32,
    ) -> Result<Self> {
        let pool_id = contract_bytes(YIELDBLOX_POOL)?;
        let oracle_id = contract_bytes(REFLECTOR_ORACLE)?;
        let principal_borrower = account_bytes(PRINCIPAL_BORROWER)?;
        let unavailable = |reason: String| Self {
            available: false,
            reason,
            pool_id,
            principal_borrower,
        };

        if indexed_ledger_start > RULE_START_LEDGER || indexed_ledger_end < RULE_END_LEDGER {
            return Ok(unavailable(format!(
                "The indexed range does not cover the complete YieldBlox replay window {RULE_START_LEDGER}-{RULE_END_LEDGER}."
            )));
        }

        let mut statement = connection.prepare(
            "SELECT sequence,close_time FROM compact_ledgers
             WHERE sequence IN (?1,?2,?3,?4,?5,?6,?7)
             ORDER BY sequence",
        )?;
        let observed = statement
            .query_map(
                params![
                    CHECKPOINTS[0].0,
                    CHECKPOINTS[1].0,
                    CHECKPOINTS[2].0,
                    CHECKPOINTS[3].0,
                    CHECKPOINTS[4].0,
                    CHECKPOINTS[5].0,
                    CHECKPOINTS[6].0,
                ],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, i64>(1)?)),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if observed.as_slice() != CHECKPOINTS {
            return Ok(unavailable(
                "The YieldBlox replay checkpoints or their close times are incomplete in this index."
                    .to_owned(),
            ));
        }

        if !contract_was_active_at(connection, &pool_id, RULE_END_LEDGER)?
            || !contract_was_active_at(connection, &oracle_id, FINDING_LEDGER)?
        {
            return Ok(unavailable(
                "The affected pool or oracle activity required by the YieldBlox rule is absent."
                    .to_owned(),
            ));
        }
        if !transaction_matches(
            connection,
            SETUP_TRANSACTION_ID,
            RULE_START_LEDGER,
            SETUP_TRANSACTION,
        )? || !transaction_matches(
            connection,
            FINDING_TRANSACTION_ID,
            FINDING_LEDGER,
            FINDING_TRANSACTION,
        )? {
            return Ok(unavailable(
                "The exact source-backed YieldBlox setup or USDC-drain transaction was not reproduced."
                    .to_owned(),
            ));
        }
        if !transaction_wallet_contract_matches(
            connection,
            FINDING_TRANSACTION_ID,
            &principal_borrower,
            &pool_id,
        )? || !transaction_contract_matches(connection, FINDING_TRANSACTION_ID, &pool_id)?
            || !transaction_contract_matches(connection, FINDING_TRANSACTION_ID, &oracle_id)?
        {
            return Ok(unavailable(
                "The exact borrower, affected-pool, or oracle edge required by the YieldBlox drain checkpoint is absent."
                    .to_owned(),
            ));
        }
        if wallet_contract_first_ledger(connection, &principal_borrower, &pool_id)?
            .is_none_or(|ledger| ledger > RULE_END_LEDGER)
        {
            return Ok(unavailable(
                "The source-backed principal borrower to affected-pool relationship was not reproduced."
                    .to_owned(),
            ));
        }

        Ok(Self {
            available: true,
            reason: "A versioned historical YieldBlox rule is evaluated against exact wallet-to-contract relationships."
                .to_owned(),
            pool_id,
            principal_borrower,
        })
    }

    pub fn available(&self) -> bool {
        self.available
    }

    pub fn findings_for_wallet(
        &self,
        connection: &Connection,
        account_id: &[u8; 32],
    ) -> Result<WalletHistoricalFindings> {
        if !self.available {
            return Ok(WalletHistoricalFindings {
                available: false,
                reason: self.reason.clone(),
                delivery_available: false,
                delivery_reason: DELIVERY_REASON,
                coverage: None,
                items: Vec::new(),
            });
        }

        let first_pool_ledger =
            wallet_contract_first_ledger(connection, account_id, &self.pool_id)?;
        let mut items = Vec::new();
        if first_pool_ledger.is_some_and(|ledger| ledger <= RULE_END_LEDGER) {
            let participant = account_id == &self.principal_borrower;
            let match_type = if participant {
                "verified-incident-participant"
            } else {
                "contract-relationship"
            };
            let (severity, title, summary) = if participant {
                (
                    "critical",
                    "Known-incident IOC match: YieldBlox USDC drain",
                    "This wallet is the source-backed principal borrower and has the exact borrower-to-pool edge in the listed drain transaction. The match proves on-chain participation, not who controlled the key. The incident is attributed to YieldBlox pool/oracle configuration, not Blend core or an unauthorized Reflector update.",
                )
            } else {
                (
                    "info",
                    "Historical relationship to the affected YieldBlox pool",
                    "An exact wallet-to-contract relationship was first observed by the time the incident ended. It can include a failed attempt and does not prove an open position, balance, financial loss, or exploit participation. The incident is attributed to YieldBlox pool/oracle configuration, not Blend core.",
                )
            };
            items.push(HistoricalWalletFinding {
                id: stable_finding_id(account_id, match_type),
                kind: "historical-security-incident",
                severity,
                title,
                summary,
                incident_at: FINDING_TIMESTAMP,
                ledger_sequence: FINDING_LEDGER,
                protocol_id: "yieldblox-blend-v2-pool",
                protocol_name: "YieldBlox pool (Blend v2 deployment)",
                contract_address: YIELDBLOX_POOL,
                transaction_hash: FINDING_TRANSACTION,
                origin: "historical-replay",
                match_type,
                sources: vec![
                    HistoricalFindingSource {
                        label: "BlockSec forensic report",
                        url: "https://blocksec.com/blog/yieldblox-dao-incident-on-stellar-oracle-misconfiguration-enabled-a-10m-drain",
                    },
                    HistoricalFindingSource {
                        label: "Blockaid containment report",
                        url: "https://www.blockaid.io/blog/73-quarantined-how-blockaid-and-stellar-validators-contained-a-10m-price-manipulation-attack",
                    },
                ],
                deliveries: Vec::new(),
            });
        }

        Ok(WalletHistoricalFindings {
            available: true,
            reason: self.reason.clone(),
            delivery_available: false,
            delivery_reason: DELIVERY_REASON,
            coverage: Some(HistoricalFindingCoverage {
                from_ledger: RULE_START_LEDGER,
                to_ledger: RULE_END_LEDGER,
            }),
            items,
        })
    }
}

fn contract_was_active_at(
    connection: &Connection,
    contract_id: &[u8; 32],
    ledger: u32,
) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM contract_usage
             WHERE contract_id=?1 AND first_ledger<=?2 AND last_ledger>=?2",
            params![&contract_id[..], ledger],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .context("checking incident contract activity")
}

fn wallet_contract_first_ledger(
    connection: &Connection,
    account_id: &[u8; 32],
    contract_id: &[u8; 32],
) -> Result<Option<u32>> {
    connection
        .query_row(
            "SELECT first_ledger FROM wallet_contracts
             WHERE account_id=?1 AND contract_id=?2",
            params![&account_id[..], &contract_id[..]],
            |row| row.get(0),
        )
        .optional()
        .context("checking incident wallet-contract relationship")
}

fn transaction_matches(
    connection: &Connection,
    tx_id: i64,
    ledger_sequence: u32,
    transaction_hash: &str,
) -> Result<bool> {
    let expected_hash = hex::decode(transaction_hash)
        .with_context(|| format!("decoding incident transaction hash {transaction_hash}"))?;
    connection
        .query_row(
            "SELECT ledger_sequence,tx_hash,successful
             FROM contract_transactions WHERE tx_id=?1",
            [tx_id],
            |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map(|value| {
            value.is_some_and(|(observed_ledger, observed_hash, successful)| {
                observed_ledger == ledger_sequence
                    && observed_hash == expected_hash
                    && successful == 1
            })
        })
        .context("checking exact incident transaction")
}

fn transaction_wallet_contract_matches(
    connection: &Connection,
    tx_id: i64,
    account_id: &[u8; 32],
    contract_id: &[u8; 32],
) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM transaction_wallet_contracts
             WHERE tx_id=?1 AND account_id=?2 AND contract_id=?3",
            params![tx_id, &account_id[..], &contract_id[..]],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .context("checking exact incident wallet-contract edge")
}

fn transaction_contract_matches(
    connection: &Connection,
    tx_id: i64,
    contract_id: &[u8; 32],
) -> Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM transaction_contracts WHERE tx_id=?1 AND contract_id=?2",
            params![tx_id, &contract_id[..]],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .context("checking exact incident contract edge")
}

fn stable_finding_id(account_id: &[u8; 32], match_type: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"stellar-public\0");
    digest.update(YIELDBLOX_RULE_ID.as_bytes());
    digest.update(b"\0");
    digest.update(account_id);
    digest.update(b"\0");
    digest.update(match_type.as_bytes());
    hex::encode(digest.finalize())
}

fn contract_bytes(value: &str) -> Result<[u8; 32]> {
    let contract: ContractId = value
        .parse()
        .with_context(|| format!("parsing incident contract {value}"))?;
    Ok(contract.0 .0)
}

fn account_bytes(value: &str) -> Result<[u8; 32]> {
    let account: MuxedAccount = value
        .parse()
        .with_context(|| format!("parsing incident account {value}"))?;
    match account {
        MuxedAccount::Ed25519(value) => Ok(value.0),
        MuxedAccount::MuxedEd25519(_) => anyhow::bail!("incident account must be a G-address"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_participant_and_exposure_findings_without_deliveries() {
        let connection = fixture();
        let rules =
            HistoricalIncidentRules::load(&connection, RULE_START_LEDGER, RULE_END_LEDGER).unwrap();
        assert!(rules.available());

        let participant = rules
            .findings_for_wallet(&connection, &rules.principal_borrower)
            .unwrap();
        assert_eq!(participant.items.len(), 1);
        assert_eq!(
            participant.items[0].match_type,
            "verified-incident-participant"
        );
        assert_eq!(participant.items[0].severity, "critical");
        assert!(participant.items[0].deliveries.is_empty());

        let exposed = [17_u8; 32];
        connection
            .execute(
                "INSERT INTO wallet_contracts VALUES(?1,?2,?3,8)",
                params![&exposed[..], &rules.pool_id[..], RULE_START_LEDGER],
            )
            .unwrap();
        let first = rules.findings_for_wallet(&connection, &exposed).unwrap();
        let second = rules.findings_for_wallet(&connection, &exposed).unwrap();
        assert_eq!(first.items[0].match_type, "contract-relationship");
        assert_eq!(first.items[0].severity, "info");
        assert_eq!(first.items[0].id, second.items[0].id);

        let unrelated = rules
            .findings_for_wallet(&connection, &[23_u8; 32])
            .unwrap();
        assert!(unrelated.available);
        assert!(unrelated.items.is_empty());
    }

    #[test]
    fn incomplete_replay_window_is_explicitly_unavailable() {
        let connection = fixture();
        let rules =
            HistoricalIncidentRules::load(&connection, FINDING_LEDGER, RULE_END_LEDGER).unwrap();
        assert!(!rules.available());
        let response = rules
            .findings_for_wallet(&connection, &rules.principal_borrower)
            .unwrap();
        assert!(!response.available);
        assert!(response.coverage.is_none());
        assert!(response.items.is_empty());
    }

    #[test]
    fn refuses_corrupt_or_incomplete_exact_incident_evidence() {
        for mutation in [
            "UPDATE contract_transactions SET tx_hash=zeroblob(32) WHERE tx_id=1131484622",
            "UPDATE contract_transactions SET successful=0 WHERE tx_id=1131484622",
            "DELETE FROM transaction_wallet_contracts WHERE tx_id=1131484622",
            "DELETE FROM transaction_contracts WHERE tx_id=1131484622",
        ] {
            let connection = fixture();
            connection.execute(mutation, []).unwrap();
            let rules =
                HistoricalIncidentRules::load(&connection, RULE_START_LEDGER, RULE_END_LEDGER)
                    .unwrap();
            assert!(
                !rules.available(),
                "mutation should disable rule: {mutation}"
            );
        }
    }

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE compact_ledgers(
                     sequence INTEGER PRIMARY KEY,
                     close_time INTEGER NOT NULL
                 );
                 CREATE TABLE contract_usage(
                     contract_id BLOB PRIMARY KEY,
                     first_ledger INTEGER NOT NULL,
                     last_ledger INTEGER NOT NULL
                 ) WITHOUT ROWID;
                 CREATE TABLE wallet_contracts(
                     account_id BLOB NOT NULL,
                     contract_id BLOB NOT NULL,
                     first_ledger INTEGER NOT NULL,
                     role_mask INTEGER NOT NULL,
                     PRIMARY KEY(account_id,contract_id)
                 ) WITHOUT ROWID;
                 CREATE TABLE contract_transactions(
                     tx_id INTEGER PRIMARY KEY,
                     tx_hash BLOB NOT NULL,
                     ledger_sequence INTEGER NOT NULL,
                     apply_order INTEGER NOT NULL,
                     successful INTEGER NOT NULL
                 );
                 CREATE TABLE transaction_wallet_contracts(
                     tx_id INTEGER NOT NULL,
                     account_id BLOB NOT NULL,
                     contract_id BLOB NOT NULL,
                     role_mask INTEGER NOT NULL,
                     PRIMARY KEY(tx_id,account_id,contract_id)
                 ) WITHOUT ROWID;
                 CREATE TABLE transaction_contracts(
                     tx_id INTEGER NOT NULL,
                     contract_id BLOB NOT NULL,
                     interaction_mask INTEGER NOT NULL,
                     PRIMARY KEY(tx_id,contract_id)
                 ) WITHOUT ROWID;",
            )
            .unwrap();
        for (sequence, close_time) in CHECKPOINTS {
            connection
                .execute(
                    "INSERT INTO compact_ledgers VALUES(?1,?2)",
                    params![sequence, close_time],
                )
                .unwrap();
        }
        let pool = contract_bytes(YIELDBLOX_POOL).unwrap();
        let oracle = contract_bytes(REFLECTOR_ORACLE).unwrap();
        let borrower = account_bytes(PRINCIPAL_BORROWER).unwrap();
        connection
            .execute(
                "INSERT INTO contract_usage VALUES(?1,?2,?3)",
                params![&pool[..], RULE_START_LEDGER, RULE_END_LEDGER],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO contract_usage VALUES(?1,?2,?3)",
                params![&oracle[..], RULE_START_LEDGER, RULE_END_LEDGER],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO wallet_contracts VALUES(?1,?2,?3,8)",
                params![&borrower[..], &pool[..], RULE_START_LEDGER],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO contract_transactions VALUES(?1,?2,?3,0,1)",
                params![
                    SETUP_TRANSACTION_ID,
                    hex::decode(SETUP_TRANSACTION).unwrap(),
                    RULE_START_LEDGER
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO contract_transactions VALUES(?1,?2,?3,0,1)",
                params![
                    FINDING_TRANSACTION_ID,
                    hex::decode(FINDING_TRANSACTION).unwrap(),
                    FINDING_LEDGER
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO transaction_wallet_contracts VALUES(?1,?2,?3,1)",
                params![FINDING_TRANSACTION_ID, &borrower[..], &pool[..]],
            )
            .unwrap();
        for contract in [&pool, &oracle] {
            connection
                .execute(
                    "INSERT INTO transaction_contracts VALUES(?1,?2,8)",
                    params![FINDING_TRANSACTION_ID, &contract[..]],
                )
                .unwrap();
        }
        connection
    }
}
