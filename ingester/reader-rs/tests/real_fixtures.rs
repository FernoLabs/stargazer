use stargazer_ledger_reader::{
    decode_ledger, parse_archive_filename, CompactIndexDatabase, ContractCall, IndexDatabase,
    IndexOptions, IndexedTransaction, DEFAULT_MAX_XDR_BYTES, PUBLIC_NETWORK_PASSPHRASE,
};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

fn fixture(name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../benchmarks/archive-samples")
        .join(name);
    path.exists().then_some(path)
}

#[test]
fn decodes_high_transaction_mainnet_fixture() {
    let Some(path) = fixture("FC352E49--63623606.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let file = parse_archive_filename(path).unwrap();
    let ledger = decode_ledger(&file, PUBLIC_NETWORK_PASSPHRASE, DEFAULT_MAX_XDR_BYTES).unwrap();
    assert_eq!(ledger.metrics.sequence, 63_623_606);
    assert_eq!(ledger.metrics.transaction_count, 998);
    assert_eq!(ledger.metrics.operation_count, 1_164);
    assert_eq!(ledger.metrics.direct_contract_call_count, 211);
    assert_eq!(ledger.transactions.len(), 998);
}

#[test]
fn consecutive_fixtures_form_a_hash_chain() {
    let Some(first_path) = fixture("FC352CF3--63623948.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let Some(second_path) = fixture("FC352CF2--63623949.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let first = decode_ledger(
        &parse_archive_filename(first_path).unwrap(),
        PUBLIC_NETWORK_PASSPHRASE,
        DEFAULT_MAX_XDR_BYTES,
    )
    .unwrap();
    let second = decode_ledger(
        &parse_archive_filename(second_path).unwrap(),
        PUBLIC_NETWORK_PASSPHRASE,
        DEFAULT_MAX_XDR_BYTES,
    )
    .unwrap();
    assert_eq!(first.metrics.sequence + 1, second.metrics.sequence);
    assert_eq!(
        first.metrics.ledger_hash,
        second.metrics.previous_ledger_hash
    );
}

fn index_options(root: PathBuf, workers: usize) -> IndexOptions {
    IndexOptions {
        archive_root: root,
        network_passphrase: PUBLIC_NETWORK_PASSPHRASE.to_owned(),
        max_xdr_bytes: DEFAULT_MAX_XDR_BYTES,
        batch_ledgers: 100,
        limit: Some(4),
        decode_workers: workers,
        decode_ahead: workers.saturating_mul(2).max(1),
        decode_retries: 0,
        retry_delay: Duration::ZERO,
    }
}

fn table_counts(path: &std::path::Path) -> Vec<i64> {
    let connection = rusqlite::Connection::open(path).unwrap();
    [
        "ledgers",
        "transactions",
        "accounts",
        "account_transactions",
        "contracts",
        "transaction_contracts",
    ]
    .into_iter()
    .map(|table| {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    })
    .collect()
}

#[test]
fn concurrent_and_serial_indexes_match_on_real_fixtures() {
    let Some(_) = fixture("FC352E49--63623606.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../benchmarks/archive-samples");
    let temp = tempfile::tempdir().unwrap();
    let serial_path = temp.path().join("serial.sqlite");
    let concurrent_path = temp.path().join("concurrent.sqlite");

    let serial_stats = IndexDatabase::open(&serial_path)
        .unwrap()
        .run(&index_options(root.clone(), 1), |_| {})
        .unwrap();
    let concurrent_stats = IndexDatabase::open(&concurrent_path)
        .unwrap()
        .run(&index_options(root, 4), |_| {})
        .unwrap();

    assert_eq!(serial_stats.indexed_ledgers, 4);
    assert_eq!(
        serial_stats.indexed_ledgers,
        concurrent_stats.indexed_ledgers
    );
    assert_eq!(serial_stats.transactions, concurrent_stats.transactions);
    assert_eq!(serial_stats.operations, concurrent_stats.operations);
    assert_eq!(table_counts(&serial_path), table_counts(&concurrent_path));
}

#[test]
fn decode_failure_rolls_back_the_concurrent_batch() {
    let Some(first) = fixture("FC352CF3--63623948.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let Some(second) = fixture("FC352CF2--63623949.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("archive");
    fs::create_dir(&archive).unwrap();
    fs::copy(&first, archive.join(first.file_name().unwrap())).unwrap();
    fs::copy(&second, archive.join(second.file_name().unwrap())).unwrap();
    fs::write(
        archive.join("FC352CF1--63623950.xdr.zst"),
        b"not a zstd ledger",
    )
    .unwrap();
    let database_path = temp.path().join("rollback.sqlite");
    let mut options = index_options(archive, 3);
    options.limit = None;

    let result = IndexDatabase::open(&database_path)
        .unwrap()
        .run(&options, |_| {});
    assert!(result.is_err());

    let counts = table_counts(&database_path);
    assert!(counts.into_iter().all(|count| count == 0));
}

fn irrelevant_transaction() -> IndexedTransaction {
    IndexedTransaction {
        hash: [7; 32],
        apply_order: 0,
        source_account: String::new(),
        source_muxed_id: None,
        fee_source_account: None,
        fee_source_muxed_id: None,
        successful: true,
        fee_charged: 0,
        operation_count: 1,
        accounts: Vec::new(),
        authorized_accounts: Vec::new(),
        wallet_contract_links: Vec::new(),
        contract_calls: Vec::new(),
        contract_touches: Vec::new(),
        contract_versions: Vec::new(),
        wasm_codes: Vec::new(),
    }
}

#[test]
fn compact_filter_rejects_transactions_without_contract_facts() {
    let mut transaction = irrelevant_transaction();
    assert!(!transaction.is_contract_interacting());

    transaction.contract_calls.push(ContractCall {
        operation_index: 0,
        contract_id: [9; 32],
        function_name: "test".to_owned(),
        call_kind: "direct",
        call_depth: 0,
    });
    assert!(transaction.is_contract_interacting());
}

fn compact_table_counts(connection: &rusqlite::Connection) -> Vec<i64> {
    [
        "compact_ledgers",
        "contract_transactions",
        "transaction_wallets",
        "transaction_contracts",
        "transaction_wallet_contracts",
        "wallet_contracts",
        "contract_usage",
        "transaction_protocols",
        "protocol_usage",
        "contract_executable_history",
        "contract_current_executable",
        "contract_code",
    ]
    .into_iter()
    .map(|table| {
        connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    })
    .collect()
}

fn scalar(connection: &rusqlite::Connection, sql: &str) -> i64 {
    connection.query_row(sql, [], |row| row.get(0)).unwrap()
}

#[test]
fn compact_index_preserves_real_contract_facts_and_resumes_without_duplicates() {
    let Some(first) = fixture("FC352CF3--63623948.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };
    let Some(second) = fixture("FC352CF2--63623949.xdr.zst") else {
        eprintln!("real benchmark fixtures are not present; skipping");
        return;
    };

    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("archive");
    fs::create_dir(&archive).unwrap();
    let first_copy = archive.join(first.file_name().unwrap());
    let second_copy = archive.join(second.file_name().unwrap());
    fs::copy(&first, &first_copy).unwrap();
    fs::copy(&second, &second_copy).unwrap();

    let mut expected_transactions = Vec::new();
    for path in [&first_copy, &second_copy] {
        let ledger = decode_ledger(
            &parse_archive_filename(path).unwrap(),
            PUBLIC_NETWORK_PASSPHRASE,
            DEFAULT_MAX_XDR_BYTES,
        )
        .unwrap();
        expected_transactions.extend(
            ledger
                .transactions
                .into_iter()
                .filter(IndexedTransaction::is_contract_interacting)
                .map(|transaction| {
                    (
                        ledger.metrics.sequence,
                        transaction.apply_order as u32,
                        transaction.hash,
                    )
                }),
        );
    }
    expected_transactions.sort_unstable_by_key(|(ledger, order, _)| (*ledger, *order));

    let protocol_roots =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/stellar-mainnet-defi-roots.json");
    let database_path = temp.path().join("compact.sqlite");
    let mut database = CompactIndexDatabase::open(&database_path, Some(&protocol_roots)).unwrap();
    let mut options = index_options(archive, 2);
    options.batch_ledgers = 2;
    options.limit = Some(2);

    let first_run = database.run(&options, |_| {}).unwrap();
    assert_eq!(first_run.indexed_ledgers, 2);
    assert_eq!(
        first_run.contract_transactions,
        expected_transactions.len() as u64
    );

    let connection = rusqlite::Connection::open(&database_path).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT ledger_sequence,apply_order,tx_hash
             FROM contract_transactions
             ORDER BY ledger_sequence,apply_order",
        )
        .unwrap();
    let stored_transactions = statement
        .query_map([], |row| {
            let hash: Vec<u8> = row.get(2)?;
            Ok((
                row.get::<_, u32>(0)?,
                row.get::<_, u32>(1)?,
                <[u8; 32]>::try_from(hash).unwrap(),
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    drop(statement);
    assert_eq!(stored_transactions, expected_transactions);

    assert_eq!(
        scalar(
            &connection,
            "SELECT COUNT(*)
             FROM contract_transactions t
             WHERE NOT EXISTS(
                 SELECT 1 FROM transaction_contracts c WHERE c.tx_id=t.tx_id
             )",
        ),
        0,
        "the real fixture has no retained transaction without a contract edge",
    );
    assert_eq!(
        scalar(
            &connection,
            "WITH expected AS (
                 SELECT link.account_id,link.contract_id,
                        MIN(t.ledger_sequence) AS first_ledger
                 FROM transaction_wallet_contracts link
                 JOIN contract_transactions t USING(tx_id)
                 GROUP BY link.account_id,link.contract_id
             )
             SELECT COUNT(*)
             FROM expected e
             LEFT JOIN wallet_contracts actual
               ON actual.account_id=e.account_id AND actual.contract_id=e.contract_id
             WHERE actual.account_id IS NULL OR actual.first_ledger<>e.first_ledger",
        ),
        0,
    );
    assert_eq!(
        scalar(
            &connection,
            "WITH expected AS (
                 SELECT account_id,contract_id
                 FROM transaction_wallet_contracts
                 GROUP BY account_id,contract_id
             )
             SELECT (SELECT COUNT(*) FROM expected) -
                    (SELECT COUNT(*) FROM wallet_contracts)",
        ),
        0,
    );
    assert_eq!(
        scalar(
            &connection,
            "SELECT COUNT(*)
             FROM transaction_wallet_contracts link
             WHERE NOT EXISTS(
                       SELECT 1 FROM transaction_wallets wallet
                       WHERE wallet.tx_id=link.tx_id
                         AND wallet.account_id=link.account_id
                   )
                OR NOT EXISTS(
                       SELECT 1 FROM transaction_contracts contract
                       WHERE contract.tx_id=link.tx_id
                         AND contract.contract_id=link.contract_id
                   )
                OR (link.role_mask & 4) != 0",
        ),
        0,
    );
    assert_eq!(
        scalar(
            &connection,
            "SELECT COUNT(*)
             FROM transaction_wallet_contracts link
             JOIN wallet_contracts summary
               ON summary.account_id=link.account_id
              AND summary.contract_id=link.contract_id
             WHERE (summary.role_mask & link.role_mask) != link.role_mask",
        ),
        0,
    );

    assert!(scalar(&connection, "SELECT COUNT(*) FROM protocol_usage") > 0);
    assert_eq!(
        scalar(
            &connection,
            "WITH expected AS (
                 SELECT tp.protocol_id,
                        MIN(t.ledger_sequence) AS first_ledger,
                        MAX(t.ledger_sequence) AS last_ledger,
                        COUNT(*) AS transaction_count,
                        SUM(t.successful) AS successful_transaction_count
                 FROM transaction_protocols tp
                 JOIN contract_transactions t USING(tx_id)
                 GROUP BY tp.protocol_id
             )
             SELECT COUNT(*)
             FROM expected e
             LEFT JOIN protocol_usage actual USING(protocol_id)
             WHERE actual.protocol_id IS NULL
                OR actual.first_ledger<>e.first_ledger
                OR actual.last_ledger<>e.last_ledger
                OR actual.transaction_count<>e.transaction_count
                OR actual.successful_transaction_count<>e.successful_transaction_count",
        ),
        0,
    );

    assert!(
        scalar(
            &connection,
            "SELECT COUNT(*) FROM contract_executable_history"
        ) > 0
    );
    assert_eq!(
        scalar(
            &connection,
            "SELECT COUNT(*)
             FROM contract_current_executable current
             WHERE NOT EXISTS (
                 SELECT 1
                 FROM contract_executable_history history
                 WHERE history.contract_id=current.contract_id
                   AND history.ledger_sequence=current.ledger_sequence
                   AND history.executable_kind=current.executable_kind
                   AND history.wasm_hash=current.wasm_hash
                   AND history.history_id=(
                       SELECT MAX(latest.history_id)
                       FROM contract_executable_history latest
                       WHERE latest.contract_id=current.contract_id
                   )
             )",
        ),
        0,
    );

    let counts_before_resume = compact_table_counts(&connection);
    drop(connection);
    let resumed = database.run(&options, |_| {}).unwrap();
    assert_eq!(resumed.indexed_ledgers, 0);
    assert_eq!(resumed.skipped_ledgers, 2);
    let connection = rusqlite::Connection::open(&database_path).unwrap();
    assert_eq!(compact_table_counts(&connection), counts_before_resume);
}

#[test]
fn compact_index_records_the_known_executable_upgrade_transition() {
    let Some(fixture) = fixture("FC3857A7--63416408.xdr.zst") else {
        eprintln!("real executable-upgrade fixture is not present; skipping");
        return;
    };
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("archive");
    fs::create_dir(&archive).unwrap();
    fs::copy(&fixture, archive.join(fixture.file_name().unwrap())).unwrap();

    let database_path = temp.path().join("upgrade.sqlite");
    let mut database = CompactIndexDatabase::open(&database_path, None).unwrap();
    let mut options = index_options(archive, 1);
    options.batch_ledgers = 1;
    options.limit = Some(1);
    let stats = database.run(&options, |_| {}).unwrap();
    assert_eq!(stats.indexed_ledgers, 1);
    assert!(stats.executable_changes >= 2);

    const CONTRACT: &str = "594BB8CC37F5CA566FEF1268F39F40CC0E7DB288241BC116910A4C079DC0B814";
    const TX_HASH: &str = "0351D70BCFB816137FA2ED970DAF3E00B1B932D759FA0C20F8002CFACF5E2200";
    const OLD_WASM: &str = "0CBC0AA6A92452375EFF21A6183B46488F6EE11405999430E68A5A23D9B6C8C2";
    const NEW_WASM: &str = "0BB7516A359D2A086D77DE4562D17211C28AD96B7AB13755B2E8987FFB9663A1";

    let connection = rusqlite::Connection::open(&database_path).unwrap();
    let contract = hex::decode(CONTRACT).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT hex(wasm_hash),source_kind,active_before_window,hex(tx_hash)
             FROM contract_executable_history
             WHERE contract_id=?1
             ORDER BY history_id",
        )
        .unwrap();
    let transitions = statement
        .query_map([&contract], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(
        transitions,
        vec![
            (
                OLD_WASM.to_owned(),
                "observed_state".to_owned(),
                1,
                TX_HASH.to_owned(),
            ),
            (
                NEW_WASM.to_owned(),
                "updated".to_owned(),
                0,
                TX_HASH.to_owned(),
            ),
        ]
    );

    let current: (String, i64) = connection
        .query_row(
            "SELECT hex(wasm_hash),exact
             FROM contract_current_executable
             WHERE contract_id=?1",
            [&contract],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(current, (NEW_WASM.to_owned(), 1));
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*)
                 FROM contract_transactions t
                 JOIN transaction_contracts c USING(tx_id)
                 WHERE c.contract_id=?1 AND hex(t.tx_hash)=?2
                   AND (c.interaction_mask & 16) != 0",
                rusqlite::params![&contract, TX_HASH],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        1,
    );
}
