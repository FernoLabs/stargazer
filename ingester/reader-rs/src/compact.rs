use crate::archive::ArchiveWalker;
use crate::decode::{DecodedLedger, IndexedTransaction};
use crate::indexer::{decode_with_retry, process_batch_ordered, IndexOptions};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

const COMPACT_SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, Default, Serialize)]
pub struct CompactIndexStats {
    pub scanned_files: u64,
    pub indexed_ledgers: u64,
    pub skipped_ledgers: u64,
    pub contract_transactions: u64,
    pub wallet_edges: u64,
    pub contract_edges: u64,
    pub executable_changes: u64,
    pub wasm_modules: u64,
    #[serde(skip)]
    pub elapsed: Duration,
}

impl CompactIndexStats {
    pub fn ledgers_per_second(&self) -> f64 {
        self.indexed_ledgers as f64 / self.elapsed.as_secs_f64().max(f64::EPSILON)
    }
}

#[derive(Debug, Deserialize)]
struct ProtocolRoots {
    roots: Vec<ProtocolRoot>,
}

#[derive(Debug, Deserialize)]
struct ProtocolRoot {
    protocol: String,
    role: String,
    contract_id: String,
    source_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutableState {
    kind: String,
    wasm_hash: Vec<u8>,
}

type WalletContractRoleMap = BTreeMap<([u8; 32], [u8; 32]), i64>;

#[derive(Debug, Default)]
struct CompactInsertCache {
    executable: HashMap<[u8; 32], Option<ExecutableState>>,
    wallet_contracts: HashMap<([u8; 32], [u8; 32]), i64>,
}

#[derive(Debug, Clone)]
struct UsageRollup {
    first_ledger: u32,
    last_ledger: u32,
    transactions: u64,
    successful: u64,
}

#[derive(Debug, Default)]
struct BatchRollups {
    contracts: HashMap<[u8; 32], UsageRollup>,
    protocols: HashMap<String, UsageRollup>,
}

fn add_usage<K: std::hash::Hash + Eq>(
    rollups: &mut HashMap<K, UsageRollup>,
    key: K,
    ledger: u32,
    successful: bool,
) {
    let rollup = rollups.entry(key).or_insert(UsageRollup {
        first_ledger: ledger,
        last_ledger: ledger,
        transactions: 0,
        successful: 0,
    });
    rollup.last_ledger = ledger;
    rollup.transactions += 1;
    rollup.successful += u64::from(successful);
}

#[derive(Debug, Clone, Default)]
struct CompactRanges(Option<(u32, u32)>);

impl CompactRanges {
    fn load(connection: &Connection) -> Result<Self> {
        let values: (Option<u32>, Option<u32>, u64) = connection.query_row(
            "SELECT MIN(sequence), MAX(sequence), COUNT(*) FROM compact_ledgers",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        match values {
            (None, None, 0) => Ok(Self(None)),
            (Some(start), Some(end), count) if count == u64::from(end) - u64::from(start) + 1 => {
                Ok(Self(Some((start, end))))
            }
            _ => bail!("compact ledger checkpoint is not one contiguous range"),
        }
    }

    fn contains(&self, sequence: u32) -> bool {
        self.0
            .is_some_and(|(start, end)| sequence >= start && sequence <= end)
    }

    fn insert(&mut self, sequence: u32) -> Result<()> {
        match self.0 {
            None => self.0 = Some((sequence, sequence)),
            Some((start, end)) if sequence >= start && sequence <= end => {}
            Some((start, end)) if end.checked_add(1) == Some(sequence) => {
                self.0 = Some((start, sequence));
            }
            Some((start, end)) => bail!(
                "compact backfill must append without gaps: indexed {start}-{end}, next {sequence}"
            ),
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ChainState {
    last: Option<(u32, [u8; 32])>,
}

pub struct CompactIndexDatabase {
    connection: Connection,
    protocols_by_contract: HashMap<[u8; 32], Vec<String>>,
}

impl CompactIndexDatabase {
    pub fn open(path: impl AsRef<Path>, protocol_roots: Option<&Path>) -> Result<Self> {
        let mut connection = Connection::open(path.as_ref())
            .with_context(|| format!("opening compact index {}", path.as_ref().display()))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=OFF;
             PRAGMA temp_store=MEMORY;
             PRAGMA cache_size=-524288;
             PRAGMA mmap_size=1073741824;
             PRAGMA wal_autocheckpoint=65536;
             PRAGMA busy_timeout=5000;",
        )?;
        connection.set_prepared_statement_cache_capacity(48);
        migrate(&mut connection)?;
        if let Some(path) = protocol_roots {
            seed_protocols(&mut connection, path)?;
        }
        let protocols_by_contract = load_protocol_map(&connection)?;
        Ok(Self {
            connection,
            protocols_by_contract,
        })
    }

    pub fn run<F>(
        &mut self,
        options: &IndexOptions,
        mut on_progress: F,
    ) -> Result<CompactIndexStats>
    where
        F: FnMut(&CompactIndexStats),
    {
        if options.decode_workers == 0 {
            bail!("decode_workers must be greater than zero");
        }
        if options.decode_ahead < options.decode_workers {
            bail!("decode_ahead must be at least decode_workers");
        }
        bind_network(&self.connection, &options.network_passphrase)?;

        let started = Instant::now();
        let mut ranges = CompactRanges::load(&self.connection)?;
        let mut archive = ArchiveWalker::new(&options.archive_root)?;
        let mut stats = CompactIndexStats::default();
        let mut exhausted = false;
        let mut chain = ChainState::default();
        let mut cache = CompactInsertCache::default();
        let batch_size = options.batch_ledgers.max(1);

        while !exhausted
            && options
                .limit
                .is_none_or(|limit| stats.indexed_ledgers < limit as u64)
        {
            let remaining = options
                .limit
                .map(|limit| limit.saturating_sub(stats.indexed_ledgers as usize))
                .unwrap_or(usize::MAX);
            let target = batch_size.min(remaining);
            let mut files = Vec::with_capacity(target);
            let mut batch_sequences = HashSet::with_capacity(target);
            while files.len() < target {
                let Some(file) = archive.next() else {
                    exhausted = true;
                    break;
                };
                let file = file?;
                stats.scanned_files += 1;
                if ranges.contains(file.sequence) || !batch_sequences.insert(file.sequence) {
                    stats.skipped_ledgers += 1;
                    continue;
                }
                files.push(file);
            }
            if files.is_empty() {
                continue;
            }

            // Reject a missing archive ledger before opening or committing the
            // batch. Mutating the checkpoint only after commit could otherwise
            // leave a sparse database that cannot be resumed.
            let mut committed_ranges = ranges.clone();
            for file in &files {
                committed_ranges.insert(file.sequence)?;
            }

            let transaction = self.connection.transaction()?;
            let mut batch_stats = CompactIndexStats::default();
            let mut rollups = BatchRollups::default();
            let protocols = &self.protocols_by_contract;
            process_batch_ordered(
                &files,
                options.decode_workers,
                options.decode_ahead,
                &|file| {
                    let mut ledger = decode_with_retry(file, options)?;
                    ledger
                        .transactions
                        .retain(IndexedTransaction::is_contract_interacting);
                    Ok(ledger)
                },
                |ledger| {
                    insert_ledger(
                        &transaction,
                        &ledger,
                        protocols,
                        &mut cache,
                        &mut chain,
                        &mut rollups,
                        &mut batch_stats,
                    )
                },
            )?;
            flush_rollups(&transaction, &rollups)?;
            transaction.commit()?;
            ranges = committed_ranges;
            add_stats(&mut stats, &batch_stats);
            stats.elapsed = started.elapsed();
            on_progress(&stats);
        }
        stats.elapsed = started.elapsed();
        Ok(stats)
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    if connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_meta'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some()
    {
        bail!("refusing to open a full-fact index as a compact index");
    }
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS compact_meta(
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS compact_ledgers(
             sequence INTEGER PRIMARY KEY,
             ledger_hash BLOB NOT NULL CHECK(length(ledger_hash)=32),
             previous_ledger_hash BLOB NOT NULL CHECK(length(previous_ledger_hash)=32),
             close_time INTEGER NOT NULL,
             contract_transaction_count INTEGER NOT NULL,
             archive_path TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS contract_transactions(
             tx_id INTEGER PRIMARY KEY,
             tx_hash BLOB NOT NULL CHECK(length(tx_hash)=32),
             ledger_sequence INTEGER NOT NULL,
             apply_order INTEGER NOT NULL,
             successful INTEGER NOT NULL
         );
         CREATE TABLE IF NOT EXISTS transaction_wallets(
             tx_id INTEGER NOT NULL,
             account_id BLOB NOT NULL CHECK(length(account_id)=32),
             role_mask INTEGER NOT NULL,
             PRIMARY KEY(tx_id, account_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS transaction_contracts(
             tx_id INTEGER NOT NULL,
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             interaction_mask INTEGER NOT NULL,
             PRIMARY KEY(tx_id, contract_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS transaction_wallet_contracts(
             tx_id INTEGER NOT NULL,
             account_id BLOB NOT NULL CHECK(length(account_id)=32),
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             role_mask INTEGER NOT NULL,
             PRIMARY KEY(tx_id, account_id, contract_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS wallet_contracts(
             account_id BLOB NOT NULL CHECK(length(account_id)=32),
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             first_ledger INTEGER NOT NULL,
             role_mask INTEGER NOT NULL,
             PRIMARY KEY(account_id, contract_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS contract_usage(
             contract_id BLOB PRIMARY KEY CHECK(length(contract_id)=32),
             first_ledger INTEGER NOT NULL,
             last_ledger INTEGER NOT NULL,
             transaction_count INTEGER NOT NULL,
             successful_transaction_count INTEGER NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS protocols(
             protocol_id TEXT PRIMARY KEY
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS protocol_contracts(
             protocol_id TEXT NOT NULL,
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             role TEXT NOT NULL,
             source_url TEXT,
             PRIMARY KEY(protocol_id, contract_id, role)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS transaction_protocols(
             tx_id INTEGER NOT NULL,
             protocol_id TEXT NOT NULL,
             PRIMARY KEY(tx_id, protocol_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS protocol_usage(
             protocol_id TEXT PRIMARY KEY,
             first_ledger INTEGER NOT NULL,
             last_ledger INTEGER NOT NULL,
             transaction_count INTEGER NOT NULL,
             successful_transaction_count INTEGER NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS contract_executable_history(
             history_id INTEGER PRIMARY KEY,
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             ledger_sequence INTEGER NOT NULL,
             apply_order INTEGER NOT NULL,
             tx_hash BLOB NOT NULL CHECK(length(tx_hash)=32),
             executable_kind TEXT NOT NULL,
             wasm_hash BLOB NOT NULL,
             source_kind TEXT NOT NULL,
             active_before_window INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS contract_executable_history_by_contract
             ON contract_executable_history(
                 contract_id,ledger_sequence,apply_order,history_id
             );
         CREATE TABLE IF NOT EXISTS contract_current_executable(
             contract_id BLOB PRIMARY KEY CHECK(length(contract_id)=32),
             ledger_sequence INTEGER NOT NULL,
             executable_kind TEXT NOT NULL,
             wasm_hash BLOB NOT NULL,
             exact INTEGER NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS contract_availability_events(
             event_id INTEGER PRIMARY KEY,
             contract_id BLOB NOT NULL CHECK(length(contract_id)=32),
             ledger_sequence INTEGER NOT NULL,
             tx_hash BLOB NOT NULL CHECK(length(tx_hash)=32),
             event_kind TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS contract_availability_by_contract
             ON contract_availability_events(contract_id,ledger_sequence,event_id);
         CREATE TABLE IF NOT EXISTS contract_code(
             wasm_hash BLOB PRIMARY KEY CHECK(length(wasm_hash)=32),
             first_seen_ledger INTEGER NOT NULL,
             first_seen_tx_hash BLOB NOT NULL CHECK(length(first_seen_tx_hash)=32),
             byte_length INTEGER NOT NULL,
             first_seen_source_kind TEXT NOT NULL
         ) WITHOUT ROWID;",
    )?;
    let existing: Option<String> = transaction
        .query_row(
            "SELECT value FROM compact_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(version) = existing {
        if version != COMPACT_SCHEMA_VERSION.to_string() {
            bail!(
                "unsupported compact schema version {version}; expected {COMPACT_SCHEMA_VERSION}"
            );
        }
    } else {
        transaction.execute(
            "INSERT INTO compact_meta(key,value) VALUES('schema_version',?1)",
            [COMPACT_SCHEMA_VERSION.to_string()],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn bind_network(connection: &Connection, passphrase: &str) -> Result<()> {
    let network_id = hex::encode(Sha256::digest(passphrase.as_bytes()));
    let existing: Option<String> = connection
        .query_row(
            "SELECT value FROM compact_meta WHERE key='network_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match existing {
        Some(existing) if existing != network_id => bail!("compact index network mismatch"),
        Some(_) => Ok(()),
        None => {
            connection.execute(
                "INSERT INTO compact_meta(key,value) VALUES('network_id',?1)",
                [network_id],
            )?;
            Ok(())
        }
    }
}

fn seed_protocols(connection: &mut Connection, path: &Path) -> Result<()> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading protocol roots {}", path.display()))?;
    let roots: ProtocolRoots = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing protocol roots {}", path.display()))?;
    let transaction = connection.transaction()?;
    for root in roots.roots {
        let contract: stellar_xdr::ContractId = root
            .contract_id
            .parse()
            .with_context(|| format!("parsing contract {}", root.contract_id))?;
        transaction.execute(
            "INSERT OR IGNORE INTO protocols(protocol_id) VALUES(?1)",
            [&root.protocol],
        )?;
        transaction.execute(
            "INSERT OR REPLACE INTO protocol_contracts(
                 protocol_id,contract_id,role,source_url
             ) VALUES(?1,?2,?3,?4)",
            params![
                root.protocol,
                &contract.0 .0[..],
                root.role,
                root.source_url
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn load_protocol_map(connection: &Connection) -> Result<HashMap<[u8; 32], Vec<String>>> {
    let mut statement = connection
        .prepare("SELECT contract_id,protocol_id FROM protocol_contracts ORDER BY protocol_id")?;
    let mut rows = statement.query([])?;
    let mut result = HashMap::<[u8; 32], Vec<String>>::new();
    while let Some(row) = rows.next()? {
        let bytes: Vec<u8> = row.get(0)?;
        let id: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid protocol contract id length"))?;
        result.entry(id).or_default().push(row.get(1)?);
    }
    Ok(result)
}

fn account_bytes(strkey: &str) -> Result<[u8; 32]> {
    let account: stellar_xdr::MuxedAccount = strkey
        .parse()
        .with_context(|| format!("parsing account {strkey}"))?;
    Ok(match account {
        stellar_xdr::MuxedAccount::Ed25519(value) => value.0,
        stellar_xdr::MuxedAccount::MuxedEd25519(value) => value.ed25519.0,
    })
}

fn account_role_mask(role: &str) -> Result<i64> {
    match role {
        "transaction_source" => Ok(1),
        "operation_source" => Ok(2),
        "fee_source" => Ok(4),
        "authorized_account" => Ok(8),
        _ => bail!("unknown account role {role}"),
    }
}

fn transaction_contracts(tx: &IndexedTransaction) -> BTreeMap<[u8; 32], i64> {
    let mut contracts = BTreeMap::new();
    for call in &tx.contract_calls {
        let mask = if call.call_kind == "direct" { 1 } else { 2 };
        *contracts.entry(call.contract_id).or_default() |= mask;
    }
    for touch in &tx.contract_touches {
        let mask = if touch.touch_kind == "event" { 4 } else { 8 };
        *contracts.entry(touch.contract_id).or_default() |= mask;
    }
    for version in &tx.contract_versions {
        *contracts.entry(version.contract_id).or_default() |= 16;
    }
    contracts
}

fn transaction_wallets(tx: &IndexedTransaction) -> Result<BTreeMap<[u8; 32], i64>> {
    let mut wallets = BTreeMap::new();
    for account in &tx.accounts {
        *wallets
            .entry(account_bytes(&account.account_id)?)
            .or_default() |= account_role_mask(account.role)?;
    }
    for account in &tx.authorized_accounts {
        *wallets.entry(account_bytes(account)?).or_default() |= 8;
    }
    Ok(wallets)
}

fn wallet_contract_links(tx: &IndexedTransaction) -> Result<WalletContractRoleMap> {
    let mut links = BTreeMap::new();
    for link in &tx.wallet_contract_links {
        *links
            .entry((account_bytes(&link.account_id)?, link.contract_id))
            .or_default() |= account_role_mask(link.role)?;
    }
    Ok(links)
}

fn validate_chain(
    transaction: &Transaction<'_>,
    ledger: &DecodedLedger,
    state: &mut ChainState,
) -> Result<()> {
    let metrics = &ledger.metrics;
    if let Some((sequence, hash)) = state.last {
        if sequence.checked_add(1) == Some(metrics.sequence) {
            if hash != metrics.previous_ledger_hash {
                bail!(
                    "compact hash-chain mismatch before ledger {}",
                    metrics.sequence
                );
            }
            state.last = Some((metrics.sequence, metrics.ledger_hash));
            return Ok(());
        }
    }
    let previous: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT ledger_hash FROM compact_ledgers WHERE sequence=?1",
            [metrics.sequence.saturating_sub(1)],
            |row| row.get(0),
        )
        .optional()?;
    if previous.is_some_and(|hash| hash != metrics.previous_ledger_hash) {
        bail!(
            "compact hash-chain mismatch before ledger {}",
            metrics.sequence
        );
    }
    let next: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT previous_ledger_hash FROM compact_ledgers WHERE sequence=?1",
            [metrics.sequence.saturating_add(1)],
            |row| row.get(0),
        )
        .optional()?;
    if next.is_some_and(|hash| hash != metrics.ledger_hash) {
        bail!(
            "compact hash-chain mismatch after ledger {}",
            metrics.sequence
        );
    }
    state.last = Some((metrics.sequence, metrics.ledger_hash));
    Ok(())
}

fn current_executable(
    transaction: &Transaction<'_>,
    cache: &mut CompactInsertCache,
    contract_id: [u8; 32],
) -> Result<Option<ExecutableState>> {
    if let Some(value) = cache.executable.get(&contract_id) {
        return Ok(value.clone());
    }
    let value = transaction
        .query_row(
            "SELECT executable_kind,wasm_hash
             FROM contract_current_executable WHERE contract_id=?1",
            [&contract_id[..]],
            |row| {
                Ok(ExecutableState {
                    kind: row.get(0)?,
                    wasm_hash: row.get(1)?,
                })
            },
        )
        .optional()?;
    cache.executable.insert(contract_id, value.clone());
    Ok(value)
}

fn record_versions(
    transaction: &Transaction<'_>,
    tx: &IndexedTransaction,
    ledger_sequence: u32,
    cache: &mut CompactInsertCache,
    stats: &mut CompactIndexStats,
) -> Result<()> {
    if !tx.successful {
        return Ok(());
    }
    for observation in &tx.contract_versions {
        if observation.change_kind == "removed" {
            transaction.execute(
                "INSERT INTO contract_availability_events(
                     contract_id,ledger_sequence,tx_hash,event_kind
                 ) VALUES(?1,?2,?3,'removed')",
                params![&observation.contract_id[..], ledger_sequence, &tx.hash[..]],
            )?;
            continue;
        }
        if observation.change_kind == "restored" {
            transaction.execute(
                "INSERT INTO contract_availability_events(
                     contract_id,ledger_sequence,tx_hash,event_kind
                 ) VALUES(?1,?2,?3,'restored')",
                params![&observation.contract_id[..], ledger_sequence, &tx.hash[..]],
            )?;
        }
        let candidate = ExecutableState {
            kind: observation.executable_kind.to_owned(),
            wasm_hash: observation
                .wasm_hash
                .map_or_else(Vec::new, |hash| hash.to_vec()),
        };
        let current = current_executable(transaction, cache, observation.contract_id)?;
        let baseline = observation.change_kind == "observed_state";
        if baseline && current.is_some() {
            continue;
        }
        if observation.change_kind == "restored" && current.as_ref() == Some(&candidate) {
            continue;
        }
        if current.as_ref() == Some(&candidate) {
            continue;
        }
        let active_before_window = i64::from(baseline || observation.change_kind == "restored");
        transaction.execute(
            "INSERT INTO contract_executable_history(
                 contract_id,ledger_sequence,apply_order,tx_hash,executable_kind,wasm_hash,
                 source_kind,active_before_window
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                &observation.contract_id[..],
                ledger_sequence,
                tx.apply_order,
                &tx.hash[..],
                &candidate.kind,
                &candidate.wasm_hash,
                observation.change_kind,
                active_before_window,
            ],
        )?;
        transaction.execute(
            "INSERT INTO contract_current_executable(
                 contract_id,ledger_sequence,executable_kind,wasm_hash,exact
             ) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(contract_id) DO UPDATE SET
                 ledger_sequence=excluded.ledger_sequence,
                 executable_kind=excluded.executable_kind,
                 wasm_hash=excluded.wasm_hash,
                 exact=excluded.exact",
            params![
                &observation.contract_id[..],
                ledger_sequence,
                &candidate.kind,
                &candidate.wasm_hash,
                i64::from(observation.exact && active_before_window == 0),
            ],
        )?;
        cache
            .executable
            .insert(observation.contract_id, Some(candidate));
        stats.executable_changes += 1;
    }
    for code in &tx.wasm_codes {
        if code.change_kind == "removed" || code.code_size == 0 {
            continue;
        }
        stats.wasm_modules += transaction.execute(
            "INSERT OR IGNORE INTO contract_code(
                 wasm_hash,first_seen_ledger,first_seen_tx_hash,byte_length,
                 first_seen_source_kind
             ) VALUES(?1,?2,?3,?4,?5)",
            params![
                &code.wasm_hash[..],
                ledger_sequence,
                &tx.hash[..],
                code.code_size,
                code.change_kind,
            ],
        )? as u64;
    }
    Ok(())
}

fn flush_rollups(transaction: &Transaction<'_>, rollups: &BatchRollups) -> Result<()> {
    let mut contract_statement = transaction.prepare_cached(
        "INSERT INTO contract_usage(
             contract_id,first_ledger,last_ledger,transaction_count,
             successful_transaction_count
         ) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(contract_id) DO UPDATE SET
             first_ledger=MIN(first_ledger,excluded.first_ledger),
             last_ledger=MAX(last_ledger,excluded.last_ledger),
             transaction_count=transaction_count+excluded.transaction_count,
             successful_transaction_count=successful_transaction_count+excluded.successful_transaction_count",
    )?;
    for (contract_id, usage) in &rollups.contracts {
        contract_statement.execute(params![
            &contract_id[..],
            usage.first_ledger,
            usage.last_ledger,
            usage.transactions,
            usage.successful,
        ])?;
    }
    let mut protocol_statement = transaction.prepare_cached(
        "INSERT INTO protocol_usage(
             protocol_id,first_ledger,last_ledger,transaction_count,
             successful_transaction_count
         ) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(protocol_id) DO UPDATE SET
             first_ledger=MIN(first_ledger,excluded.first_ledger),
             last_ledger=MAX(last_ledger,excluded.last_ledger),
             transaction_count=transaction_count+excluded.transaction_count,
             successful_transaction_count=successful_transaction_count+excluded.successful_transaction_count",
    )?;
    for (protocol, usage) in &rollups.protocols {
        protocol_statement.execute(params![
            protocol,
            usage.first_ledger,
            usage.last_ledger,
            usage.transactions,
            usage.successful,
        ])?;
    }
    Ok(())
}

fn insert_ledger(
    transaction: &Transaction<'_>,
    ledger: &DecodedLedger,
    protocols_by_contract: &HashMap<[u8; 32], Vec<String>>,
    cache: &mut CompactInsertCache,
    chain: &mut ChainState,
    rollups: &mut BatchRollups,
    stats: &mut CompactIndexStats,
) -> Result<()> {
    validate_chain(transaction, ledger, chain)?;
    transaction.execute(
        "INSERT INTO compact_ledgers(
             sequence,ledger_hash,previous_ledger_hash,close_time,
             contract_transaction_count,archive_path
         ) VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            ledger.metrics.sequence,
            &ledger.metrics.ledger_hash[..],
            &ledger.metrics.previous_ledger_hash[..],
            ledger.metrics.close_time,
            ledger.transactions.len(),
            ledger.metrics.path,
        ],
    )?;

    for indexed_tx in &ledger.transactions {
        let wallets = transaction_wallets(indexed_tx)?;
        let contracts = transaction_contracts(indexed_tx);
        let wallet_contracts = wallet_contract_links(indexed_tx)?;
        transaction.execute(
            "INSERT INTO contract_transactions(
                 tx_hash,ledger_sequence,apply_order,successful
             ) VALUES(?1,?2,?3,?4)",
            params![
                &indexed_tx.hash[..],
                ledger.metrics.sequence,
                indexed_tx.apply_order,
                indexed_tx.successful,
            ],
        )?;
        let tx_id = transaction.last_insert_rowid();
        for (account_id, role_mask) in &wallets {
            transaction.execute(
                "INSERT INTO transaction_wallets(tx_id,account_id,role_mask)
                 VALUES(?1,?2,?3)",
                params![tx_id, &account_id[..], role_mask],
            )?;
        }
        for ((account_id, contract_id), role_mask) in &wallet_contracts {
            transaction.execute(
                "INSERT INTO transaction_wallet_contracts(
                     tx_id,account_id,contract_id,role_mask
                 ) VALUES(?1,?2,?3,?4)",
                params![tx_id, &account_id[..], &contract_id[..], role_mask],
            )?;
            let seen_roles = cache
                .wallet_contracts
                .entry((*account_id, *contract_id))
                .or_default();
            if role_mask & !*seen_roles != 0 {
                transaction.execute(
                    "INSERT INTO wallet_contracts(
                         account_id,contract_id,first_ledger,role_mask
                     ) VALUES(?1,?2,?3,?4)
                     ON CONFLICT(account_id,contract_id) DO UPDATE SET
                         first_ledger=MIN(wallet_contracts.first_ledger,excluded.first_ledger),
                         role_mask=wallet_contracts.role_mask | excluded.role_mask",
                    params![
                        &account_id[..],
                        &contract_id[..],
                        ledger.metrics.sequence,
                        role_mask
                    ],
                )?;
                *seen_roles |= role_mask;
            }
        }
        let mut protocols = BTreeSet::new();
        for (contract_id, interaction_mask) in &contracts {
            transaction.execute(
                "INSERT INTO transaction_contracts(tx_id,contract_id,interaction_mask)
                 VALUES(?1,?2,?3)",
                params![tx_id, &contract_id[..], interaction_mask],
            )?;
            add_usage(
                &mut rollups.contracts,
                *contract_id,
                ledger.metrics.sequence,
                indexed_tx.successful,
            );
            if let Some(ids) = protocols_by_contract.get(contract_id) {
                protocols.extend(ids.iter().cloned());
            }
        }
        for protocol in protocols {
            transaction.execute(
                "INSERT INTO transaction_protocols(tx_id,protocol_id) VALUES(?1,?2)",
                params![tx_id, &protocol],
            )?;
            add_usage(
                &mut rollups.protocols,
                protocol,
                ledger.metrics.sequence,
                indexed_tx.successful,
            );
        }
        record_versions(
            transaction,
            indexed_tx,
            ledger.metrics.sequence,
            cache,
            stats,
        )?;
        stats.contract_transactions += 1;
        stats.wallet_edges += wallets.len() as u64;
        stats.contract_edges += contracts.len() as u64;
    }
    stats.indexed_ledgers += 1;
    Ok(())
}

fn add_stats(total: &mut CompactIndexStats, batch: &CompactIndexStats) {
    total.indexed_ledgers += batch.indexed_ledgers;
    total.contract_transactions += batch.contract_transactions;
    total.wallet_edges += batch.wallet_edges;
    total.contract_edges += batch.contract_edges;
    total.executable_changes += batch.executable_changes;
    total.wasm_modules += batch.wasm_modules;
}

#[cfg(test)]
mod tests {
    use super::CompactRanges;

    #[test]
    fn compact_ranges_reject_checkpoint_gaps() {
        let mut ranges = CompactRanges::default();
        ranges.insert(10).unwrap();
        ranges.insert(11).unwrap();
        assert!(ranges.insert(9).is_err());
        assert!(ranges.insert(13).is_err());
        assert!(!ranges.contains(9));
        assert!(ranges.contains(10));
        assert!(ranges.contains(11));
        assert!(!ranges.contains(13));
    }
}
