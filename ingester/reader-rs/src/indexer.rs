use crate::archive::ArchiveWalker;
use crate::decode::{decode_ledger, DecodedLedger};
use anyhow::{anyhow, bail, Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

const SCHEMA_VERSION: i64 = 5;

#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub archive_root: PathBuf,
    pub network_passphrase: String,
    pub max_xdr_bytes: usize,
    pub batch_ledgers: usize,
    pub limit: Option<usize>,
    pub decode_workers: usize,
    pub decode_ahead: usize,
    pub decode_retries: usize,
    pub retry_delay: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct IndexRunStats {
    pub scanned_files: u64,
    pub indexed_ledgers: u64,
    pub skipped_ledgers: u64,
    pub transactions: u64,
    pub operations: u64,
    pub direct_contract_calls: u64,
    pub contract_touches: u64,
    pub compressed_bytes: u64,
    pub xdr_bytes: u64,
    pub elapsed: Duration,
}

impl IndexRunStats {
    pub fn ledgers_per_second(&self) -> f64 {
        self.indexed_ledgers as f64 / self.elapsed.as_secs_f64().max(f64::EPSILON)
    }

    pub fn xdr_mib_per_second(&self) -> f64 {
        (self.xdr_bytes as f64 / 1_048_576.0) / self.elapsed.as_secs_f64().max(f64::EPSILON)
    }
}

pub struct IndexDatabase {
    connection: Connection,
}

#[derive(Debug, Default)]
struct IndexedRanges(Vec<(u32, u32)>);

impl IndexedRanges {
    fn load(connection: &Connection) -> Result<Self> {
        let mut statement = connection.prepare(
            "WITH ordered AS (
                 SELECT sequence,
                        sequence - ROW_NUMBER() OVER (ORDER BY sequence) AS island
                 FROM ledgers
             )
             SELECT MIN(sequence), MAX(sequence)
             FROM ordered
             GROUP BY island
             ORDER BY MIN(sequence)",
        )?;
        let ranges = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Self(ranges))
    }

    fn contains(&self, sequence: u32) -> bool {
        self.0
            .binary_search_by(|(start, end)| {
                if sequence < *start {
                    std::cmp::Ordering::Greater
                } else if sequence > *end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
    }

    fn insert(&mut self, sequence: u32) {
        let index = self.0.partition_point(|(start, _)| *start <= sequence);
        if index > 0 && self.0[index - 1].1 >= sequence {
            return;
        }

        let joins_left = index > 0
            && self.0[index - 1]
                .1
                .checked_add(1)
                .is_some_and(|next| next == sequence);
        let joins_right = index < self.0.len()
            && sequence
                .checked_add(1)
                .is_some_and(|next| next == self.0[index].0);

        match (joins_left, joins_right) {
            (true, true) => {
                let right_end = self.0.remove(index).1;
                self.0[index - 1].1 = right_end;
            }
            (true, false) => self.0[index - 1].1 = sequence,
            (false, true) => self.0[index].0 = sequence,
            (false, false) => self.0.insert(index, (sequence, sequence)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletProtocolSummary {
    pub contract_id: String,
    pub transactions: u64,
    pub first_ledger: u32,
    pub last_ledger: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct WalletTransactionSummary {
    pub hash: String,
    pub ledger_sequence: u32,
    pub apply_order: u32,
    pub successful: bool,
    pub operation_count: u32,
}

impl IndexDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut connection = Connection::open(path.as_ref())
            .with_context(|| format!("opening index database {}", path.as_ref().display()))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA temp_store=MEMORY;
             PRAGMA cache_size=-65536;
             PRAGMA busy_timeout=5000;",
        )?;
        connection.set_prepared_statement_cache_capacity(32);
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn run<F>(&mut self, options: &IndexOptions, mut on_progress: F) -> Result<IndexRunStats>
    where
        F: FnMut(&IndexRunStats),
    {
        if options.decode_workers == 0 {
            bail!("decode_workers must be greater than zero");
        }
        if options.decode_ahead < options.decode_workers {
            bail!(
                "decode_ahead ({}) must be at least decode_workers ({})",
                options.decode_ahead,
                options.decode_workers
            );
        }

        let started = Instant::now();
        bind_network(&self.connection, &options.network_passphrase)?;
        let mut indexed_ranges = IndexedRanges::load(&self.connection)?;
        let mut stats = IndexRunStats::default();
        let mut archive = ArchiveWalker::new(&options.archive_root)?;
        let batch_size = options.batch_ledgers.max(1);
        let mut exhausted = false;

        while !exhausted
            && options
                .limit
                .is_none_or(|limit| stats.indexed_ledgers < limit as u64)
        {
            let indexed_before = stats.indexed_ledgers;
            let scanned_before = stats.scanned_files;
            let remaining = options
                .limit
                .map(|limit| limit.saturating_sub(stats.indexed_ledgers as usize))
                .unwrap_or(usize::MAX);
            let target = batch_size.min(remaining);
            let mut files = Vec::with_capacity(target.min(batch_size));
            let mut batch_sequences = HashSet::with_capacity(target.min(batch_size));

            while files.len() < target {
                let Some(file) = archive.next() else {
                    exhausted = true;
                    break;
                };
                let file = file?;
                stats.scanned_files += 1;
                if indexed_ranges.contains(file.sequence) || !batch_sequences.insert(file.sequence)
                {
                    stats.skipped_ledgers += 1;
                    continue;
                }
                files.push(file);
            }

            if files.is_empty() {
                stats.elapsed = started.elapsed();
                if stats.scanned_files != scanned_before {
                    on_progress(&stats);
                }
                continue;
            }

            let transaction = self.connection.transaction()?;
            let mut batch_stats = IndexRunStats::default();
            let mut insert_cache = InsertCache::default();
            process_batch_ordered(
                &files,
                options.decode_workers,
                options.decode_ahead,
                &|file| decode_with_retry(file, options),
                |ledger| {
                    insert_ledger(&transaction, &ledger, &mut insert_cache)?;
                    record_ledger(&mut batch_stats, &ledger);
                    Ok(())
                },
            )?;
            transaction.commit()?;
            for file in &files {
                indexed_ranges.insert(file.sequence);
            }
            add_committed_stats(&mut stats, &batch_stats);
            stats.elapsed = started.elapsed();
            if stats.indexed_ledgers != indexed_before || stats.scanned_files != scanned_before {
                on_progress(&stats);
            }
        }
        stats.elapsed = started.elapsed();
        Ok(stats)
    }

    pub fn wallet_protocols(&self, account: &str) -> Result<Vec<WalletProtocolSummary>> {
        let account = normalized_account_strkey(account)?;
        let mut statement = self.connection.prepare_cached(
            "SELECT c.strkey,
                    COUNT(DISTINCT at.transaction_hash),
                    MIN(at.ledger_sequence),
                    MAX(at.ledger_sequence)
             FROM accounts a
             JOIN account_transactions at ON at.account_id=a.account_id
             JOIN transaction_contracts tc ON tc.transaction_hash=at.transaction_hash
             JOIN contracts c ON c.contract_id=tc.contract_id
             WHERE a.strkey=?1
             GROUP BY c.contract_id
             ORDER BY COUNT(DISTINCT at.transaction_hash) DESC, c.strkey",
        )?;
        let rows = statement
            .query_map([account], |row| {
                Ok(WalletProtocolSummary {
                    contract_id: row.get(0)?,
                    transactions: row.get(1)?,
                    first_ledger: row.get(2)?,
                    last_ledger: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn wallet_transactions(
        &self,
        account: &str,
        limit: usize,
    ) -> Result<Vec<WalletTransactionSummary>> {
        let account = normalized_account_strkey(account)?;
        let mut statement = self.connection.prepare_cached(
            "SELECT DISTINCT t.hash, t.ledger_sequence, t.apply_order,
                    t.successful, t.operation_count
             FROM accounts a
             JOIN account_transactions at ON at.account_id=a.account_id
             JOIN transactions t ON t.hash=at.transaction_hash
             WHERE a.strkey=?1
             ORDER BY t.ledger_sequence DESC, t.apply_order DESC
             LIMIT ?2",
        )?;
        let rows = statement
            .query_map(params![account, limit], |row| {
                let hash: Vec<u8> = row.get(0)?;
                Ok(WalletTransactionSummary {
                    hash: hex::encode(hash),
                    ledger_sequence: row.get(1)?,
                    apply_order: row.get(2)?,
                    successful: row.get(3)?,
                    operation_count: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[derive(Debug)]
struct DecodeJob {
    ordinal: usize,
    file: crate::archive::ArchiveFile,
}

#[derive(Debug)]
struct DecodeOutcome<T> {
    ordinal: usize,
    result: Result<T>,
}

/// Decodes concurrently while consuming results in archive order. At most
/// `decode_ahead` jobs can be decoding, queued, or waiting to be consumed.
pub(crate) fn process_batch_ordered<T, D, C>(
    files: &[crate::archive::ArchiveFile],
    decode_workers: usize,
    decode_ahead: usize,
    decode: &D,
    mut consume: C,
) -> Result<()>
where
    T: Send,
    D: Fn(&crate::archive::ArchiveFile) -> Result<T> + Sync,
    C: FnMut(T) -> Result<()>,
{
    if files.is_empty() {
        return Ok(());
    }
    let worker_count = decode_workers.min(files.len()).max(1);
    let window = decode_ahead.max(worker_count).min(files.len()).max(1);

    if worker_count == 1 {
        for file in files {
            let decoded = decode(file).with_context(|| {
                format!(
                    "decoding ledger {} from {}",
                    file.sequence,
                    file.path.display()
                )
            })?;
            consume(decoded)?;
        }
        return Ok(());
    }

    std::thread::scope(|scope| -> Result<()> {
        let (job_sender, job_receiver) = mpsc::sync_channel::<DecodeJob>(window);
        let job_receiver = Arc::new(Mutex::new(job_receiver));
        let (result_sender, result_receiver) = mpsc::sync_channel::<DecodeOutcome<T>>(window);

        for worker in 0..worker_count {
            let job_receiver = Arc::clone(&job_receiver);
            let result_sender = result_sender.clone();
            std::thread::Builder::new()
                .name(format!("stargazer-decode-{worker}"))
                .stack_size(8 * 1024 * 1024)
                .spawn_scoped(scope, move || loop {
                    let job = match job_receiver.lock() {
                        Ok(receiver) => receiver.recv(),
                        Err(_) => return,
                    };
                    let Ok(job) = job else {
                        return;
                    };
                    let sequence = job.file.sequence;
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        decode(&job.file)
                    }))
                    .unwrap_or_else(|_| {
                        Err(anyhow!(
                            "decoder worker panicked while decoding ledger {sequence}"
                        ))
                    });
                    if result_sender
                        .send(DecodeOutcome {
                            ordinal: job.ordinal,
                            result,
                        })
                        .is_err()
                    {
                        return;
                    }
                })
                .context("spawning decode worker")?;
        }
        drop(result_sender);

        let pipeline_result = (|| -> Result<()> {
            let mut next_to_schedule = 0;
            while next_to_schedule < window {
                job_sender
                    .send(DecodeJob {
                        ordinal: next_to_schedule,
                        file: files[next_to_schedule].clone(),
                    })
                    .context("decode workers stopped while scheduling a ledger")?;
                next_to_schedule += 1;
            }

            let mut next_to_consume = 0;
            let mut pending = BTreeMap::new();
            while next_to_consume < files.len() {
                let outcome = match pending.remove(&next_to_consume) {
                    Some(outcome) => outcome,
                    None => {
                        let outcome = result_receiver
                            .recv()
                            .context("all decode workers stopped before completing the batch")?;
                        if outcome.ordinal != next_to_consume {
                            if pending.insert(outcome.ordinal, outcome).is_some() {
                                bail!("decode worker returned a duplicate result");
                            }
                            continue;
                        }
                        outcome
                    }
                };
                let file = &files[next_to_consume];
                let decoded = outcome.result.with_context(|| {
                    format!(
                        "decoding ledger {} from {}",
                        file.sequence,
                        file.path.display()
                    )
                })?;
                consume(decoded)?;
                next_to_consume += 1;

                if next_to_schedule < files.len() {
                    job_sender
                        .send(DecodeJob {
                            ordinal: next_to_schedule,
                            file: files[next_to_schedule].clone(),
                        })
                        .context("decode workers stopped while scheduling a ledger")?;
                    next_to_schedule += 1;
                }
            }
            Ok(())
        })();

        drop(job_sender);
        drop(result_receiver);
        pipeline_result
    })
}

fn record_ledger(stats: &mut IndexRunStats, ledger: &DecodedLedger) {
    stats.indexed_ledgers += 1;
    stats.transactions += ledger.metrics.transaction_count as u64;
    stats.operations += ledger.metrics.operation_count as u64;
    stats.direct_contract_calls += ledger.metrics.direct_contract_call_count as u64;
    stats.contract_touches += ledger.metrics.contract_touch_count as u64;
    stats.compressed_bytes += ledger.metrics.compressed_bytes;
    stats.xdr_bytes += ledger.metrics.xdr_bytes as u64;
}

fn add_committed_stats(stats: &mut IndexRunStats, committed: &IndexRunStats) {
    stats.indexed_ledgers += committed.indexed_ledgers;
    stats.transactions += committed.transactions;
    stats.operations += committed.operations;
    stats.direct_contract_calls += committed.direct_contract_calls;
    stats.contract_touches += committed.contract_touches;
    stats.compressed_bytes += committed.compressed_bytes;
    stats.xdr_bytes += committed.xdr_bytes;
}

pub(crate) fn decode_with_retry(
    file: &crate::archive::ArchiveFile,
    options: &IndexOptions,
) -> Result<DecodedLedger> {
    let mut retries = 0;
    loop {
        match decode_ledger(file, &options.network_passphrase, options.max_xdr_bytes) {
            Ok(ledger) => return Ok(ledger),
            Err(error) if retries < options.decode_retries => {
                retries += 1;
                eprintln!(
                    "decode failed for ledger {} (attempt {}/{}): {error:#}; retrying",
                    file.sequence,
                    retries,
                    options.decode_retries + 1,
                );
                std::thread::sleep(options.retry_delay);
            }
            Err(error) => return Err(error),
        }
    }
}

fn migrate(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_meta (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         ) WITHOUT ROWID;",
    )?;
    let existing_version: Option<String> = transaction
        .query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(version) = existing_version {
        if version != SCHEMA_VERSION.to_string() {
            bail!("unsupported index schema version {version}; expected {SCHEMA_VERSION}");
        }
    }
    transaction.execute_batch(
        "
         CREATE TABLE IF NOT EXISTS ledgers (
             sequence INTEGER PRIMARY KEY,
             ledger_hash BLOB NOT NULL UNIQUE CHECK(length(ledger_hash)=32),
             previous_ledger_hash BLOB NOT NULL CHECK(length(previous_ledger_hash)=32),
             close_time INTEGER NOT NULL,
             meta_version INTEGER NOT NULL,
             transaction_count INTEGER NOT NULL,
             successful_transaction_count INTEGER NOT NULL,
             operation_count INTEGER NOT NULL,
             direct_contract_call_count INTEGER NOT NULL,
             contract_touch_count INTEGER NOT NULL,
             soroban_state_bytes INTEGER NOT NULL,
             evicted_keys INTEGER NOT NULL,
             compressed_bytes INTEGER NOT NULL,
             xdr_bytes INTEGER NOT NULL,
             archive_path TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS transactions (
             hash BLOB PRIMARY KEY CHECK(length(hash)=32),
             ledger_sequence INTEGER NOT NULL REFERENCES ledgers(sequence),
             apply_order INTEGER NOT NULL,
             source_account BLOB NOT NULL CHECK(length(source_account)=32),
             source_muxed_id BLOB,
             fee_source_account BLOB,
             fee_source_muxed_id BLOB,
             successful INTEGER NOT NULL,
             fee_charged INTEGER NOT NULL,
             operation_count INTEGER NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS accounts (
             account_id BLOB PRIMARY KEY CHECK(length(account_id)=32),
             strkey TEXT NOT NULL UNIQUE
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS account_transactions (
             account_id BLOB NOT NULL REFERENCES accounts(account_id),
             muxed_id BLOB NOT NULL,
             transaction_hash BLOB NOT NULL REFERENCES transactions(hash),
             ledger_sequence INTEGER NOT NULL,
             role_mask INTEGER NOT NULL CHECK(role_mask BETWEEN 1 AND 7),
             PRIMARY KEY(account_id, ledger_sequence, transaction_hash, muxed_id)
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS contracts (
             contract_id BLOB PRIMARY KEY CHECK(length(contract_id)=32),
             strkey TEXT NOT NULL UNIQUE
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS transaction_contracts (
             transaction_hash BLOB NOT NULL REFERENCES transactions(hash),
             ledger_sequence INTEGER NOT NULL,
             contract_id BLOB NOT NULL REFERENCES contracts(contract_id),
             interaction_mask INTEGER NOT NULL CHECK(interaction_mask BETWEEN 1 AND 15),
             PRIMARY KEY(transaction_hash, contract_id)
         ) WITHOUT ROWID;",
    )?;
    transaction.execute(
        "INSERT OR IGNORE INTO schema_meta(key, value) VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    transaction.commit()?;
    Ok(())
}

fn bind_network(connection: &Connection, passphrase: &str) -> Result<()> {
    let network_id = hex::encode(Sha256::digest(passphrase.as_bytes()));
    let existing: Option<String> = connection
        .query_row(
            "SELECT value FROM schema_meta WHERE key='network_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    match existing {
        Some(existing) if existing != network_id => {
            bail!("index network mismatch: database is bound to {existing}, requested {network_id}")
        }
        Some(_) => Ok(()),
        None => {
            connection.execute(
                "INSERT INTO schema_meta(key, value) VALUES ('network_id', ?1)",
                [network_id],
            )?;
            Ok(())
        }
    }
}

fn account_bytes(strkey: &str) -> Result<[u8; 32]> {
    let account: stellar_xdr::MuxedAccount = strkey
        .parse()
        .with_context(|| format!("parsing account StrKey {strkey}"))?;
    match account {
        stellar_xdr::MuxedAccount::Ed25519(value) => Ok(value.0),
        stellar_xdr::MuxedAccount::MuxedEd25519(_) => {
            bail!("account interaction was not normalized to a G-address: {strkey}")
        }
    }
}

fn normalized_account_strkey(strkey: &str) -> Result<String> {
    let account: stellar_xdr::MuxedAccount = strkey
        .parse()
        .with_context(|| format!("parsing account StrKey {strkey}"))?;
    Ok(match account {
        stellar_xdr::MuxedAccount::Ed25519(value) => {
            stellar_xdr::MuxedAccount::Ed25519(value).to_string()
        }
        stellar_xdr::MuxedAccount::MuxedEd25519(value) => {
            stellar_xdr::MuxedAccount::Ed25519(value.ed25519).to_string()
        }
    })
}

fn muxed_id_bytes(id: Option<u64>) -> Option<Vec<u8>> {
    id.map(|value| value.to_be_bytes().to_vec())
}

fn role_mask(role: &str) -> Result<i64> {
    match role {
        "transaction_source" => Ok(1),
        "operation_source" => Ok(2),
        "fee_source" => Ok(4),
        _ => bail!("unknown account interaction role {role}"),
    }
}

fn call_kind_mask(kind: &str) -> Result<i64> {
    match kind {
        "direct" => Ok(1),
        "authorized" => Ok(2),
        _ => bail!("unknown contract call kind {kind}"),
    }
}

fn touch_kind_mask(kind: &str) -> Result<i64> {
    match kind {
        "event" => Ok(4),
        "diagnostic" => Ok(8),
        _ => bail!("unknown contract touch kind {kind}"),
    }
}

fn insert_account(
    transaction: &Transaction<'_>,
    cache: &mut HashMap<String, [u8; 32]>,
    strkey: &str,
) -> Result<[u8; 32]> {
    if let Some(bytes) = cache.get(strkey) {
        return Ok(*bytes);
    }
    let bytes = account_bytes(strkey)?;
    transaction
        .prepare_cached("INSERT OR IGNORE INTO accounts(account_id, strkey) VALUES (?1, ?2)")?
        .execute(params![&bytes[..], strkey])?;
    cache.insert(strkey.to_owned(), bytes);
    Ok(bytes)
}

fn insert_contract(
    transaction: &Transaction<'_>,
    cache: &mut HashSet<[u8; 32]>,
    contract_id: &[u8; 32],
) -> Result<()> {
    if !cache.insert(*contract_id) {
        return Ok(());
    }
    let strkey = stellar_xdr::ContractId(stellar_xdr::Hash(*contract_id)).to_string();
    transaction
        .prepare_cached("INSERT OR IGNORE INTO contracts(contract_id, strkey) VALUES (?1, ?2)")?
        .execute(params![&contract_id[..], strkey])?;
    Ok(())
}

#[derive(Debug, Default)]
struct InsertCache {
    accounts: HashMap<String, [u8; 32]>,
    contracts: HashSet<[u8; 32]>,
}

fn insert_ledger(
    transaction: &Transaction<'_>,
    ledger: &DecodedLedger,
    cache: &mut InsertCache,
) -> Result<()> {
    let metrics = &ledger.metrics;
    validate_chain(transaction, metrics)?;
    transaction.prepare_cached(
        "INSERT INTO ledgers(
             sequence, ledger_hash, previous_ledger_hash, close_time, meta_version, transaction_count,
             successful_transaction_count, operation_count, direct_contract_call_count,
             contract_touch_count, soroban_state_bytes, evicted_keys, compressed_bytes, xdr_bytes,
             archive_path
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
    )?.execute(params![
            metrics.sequence,
            &metrics.ledger_hash[..],
            &metrics.previous_ledger_hash[..],
            metrics.close_time,
            metrics.meta_version,
            metrics.transaction_count,
            metrics.successful_transaction_count,
            metrics.operation_count,
            metrics.direct_contract_call_count,
            metrics.contract_touch_count,
            metrics.soroban_state_bytes,
            metrics.evicted_keys,
            metrics.compressed_bytes,
            metrics.xdr_bytes,
            metrics.path,
        ])?;

    for indexed_transaction in &ledger.transactions {
        let source_account = insert_account(
            transaction,
            &mut cache.accounts,
            &indexed_transaction.source_account,
        )?;
        let source_muxed_id = muxed_id_bytes(indexed_transaction.source_muxed_id);
        let fee_source_account = indexed_transaction
            .fee_source_account
            .as_deref()
            .map(|strkey| insert_account(transaction, &mut cache.accounts, strkey))
            .transpose()?
            .map(|bytes| bytes.to_vec());
        let fee_source_muxed_id = muxed_id_bytes(indexed_transaction.fee_source_muxed_id);
        transaction
            .prepare_cached(
                "INSERT INTO transactions(
                 hash, ledger_sequence, apply_order, source_account, source_muxed_id,
                 fee_source_account, fee_source_muxed_id, successful, fee_charged, operation_count
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?
            .execute(params![
                &indexed_transaction.hash[..],
                metrics.sequence,
                indexed_transaction.apply_order,
                &source_account[..],
                source_muxed_id,
                fee_source_account,
                fee_source_muxed_id,
                indexed_transaction.successful,
                indexed_transaction.fee_charged,
                indexed_transaction.operation_count,
            ])?;
        let mut account_roles = BTreeMap::<([u8; 32], Vec<u8>), i64>::new();
        for account in &indexed_transaction.accounts {
            let account_id = insert_account(transaction, &mut cache.accounts, &account.account_id)?;
            let muxed_id = muxed_id_bytes(account.muxed_id).unwrap_or_default();
            *account_roles.entry((account_id, muxed_id)).or_default() |= role_mask(account.role)?;
        }
        for ((account_id, muxed_id), role_mask) in account_roles {
            transaction
                .prepare_cached(
                    "INSERT INTO account_transactions(
                     account_id, muxed_id, transaction_hash, ledger_sequence, role_mask
                 ) VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(account_id, ledger_sequence, transaction_hash, muxed_id)
                 DO UPDATE SET role_mask=role_mask | excluded.role_mask",
                )?
                .execute(params![
                    &account_id[..],
                    muxed_id,
                    &indexed_transaction.hash[..],
                    metrics.sequence,
                    role_mask,
                ])?;
        }

        let mut contract_interactions = BTreeMap::<[u8; 32], i64>::new();
        for call in &indexed_transaction.contract_calls {
            *contract_interactions.entry(call.contract_id).or_default() |=
                call_kind_mask(call.call_kind)?;
        }
        for touch in &indexed_transaction.contract_touches {
            *contract_interactions.entry(touch.contract_id).or_default() |=
                touch_kind_mask(touch.touch_kind)?;
        }
        for (contract_id, interaction_mask) in contract_interactions {
            insert_contract(transaction, &mut cache.contracts, &contract_id)?;
            transaction
                .prepare_cached(
                    "INSERT INTO transaction_contracts(
                         transaction_hash, ledger_sequence, contract_id, interaction_mask
                     ) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(transaction_hash, contract_id)
                     DO UPDATE SET interaction_mask=interaction_mask | excluded.interaction_mask",
                )?
                .execute(params![
                    &indexed_transaction.hash[..],
                    metrics.sequence,
                    &contract_id[..],
                    interaction_mask,
                ])?;
        }
    }
    Ok(())
}

fn validate_chain(
    transaction: &Transaction<'_>,
    metrics: &crate::decode::LedgerMetrics,
) -> Result<()> {
    if metrics.sequence > 0 {
        let previous: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT ledger_hash FROM ledgers WHERE sequence=?1",
                [metrics.sequence - 1],
                |row| row.get(0),
            )
            .optional()?;
        if previous
            .as_deref()
            .is_some_and(|hash| hash != metrics.previous_ledger_hash)
        {
            bail!("hash-chain mismatch before ledger {}", metrics.sequence);
        }
    }
    let next: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT previous_ledger_hash FROM ledgers WHERE sequence=?1",
            [metrics.sequence.saturating_add(1)],
            |row| row.get(0),
        )
        .optional()?;
    if next
        .as_deref()
        .is_some_and(|hash| hash != metrics.ledger_hash)
    {
        bail!("hash-chain mismatch after ledger {}", metrics.sequence);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn fake_files(count: u32) -> Vec<crate::archive::ArchiveFile> {
        (0..count)
            .map(|sequence| crate::archive::ArchiveFile {
                path: PathBuf::from(format!("fixture--{sequence}.xdr.zst")),
                sequence,
            })
            .collect()
    }

    #[test]
    fn rate_helpers_are_finite_for_empty_run() {
        let stats = IndexRunStats::default();
        assert!(stats.ledgers_per_second().is_finite());
        assert!(stats.xdr_mib_per_second().is_finite());
    }

    #[test]
    fn indexed_ranges_find_sparse_islands() {
        let ranges = IndexedRanges(vec![(10, 20), (30, 30), (40, 50)]);
        assert!(ranges.contains(10));
        assert!(ranges.contains(15));
        assert!(ranges.contains(50));
        assert!(!ranges.contains(9));
        assert!(!ranges.contains(21));
        assert!(!ranges.contains(51));
    }

    #[test]
    fn indexed_ranges_merge_new_sequences() {
        let mut ranges = IndexedRanges(vec![(10, 20), (22, 30)]);
        ranges.insert(21);
        ranges.insert(40);
        ranges.insert(39);
        ranges.insert(10);
        assert_eq!(ranges.0, vec![(10, 30), (39, 40)]);
    }

    #[test]
    fn concurrent_decode_is_consumed_in_archive_order() {
        let files = fake_files(4);
        let completion_order = Arc::new(Mutex::new(Vec::new()));
        let decoder_completion_order = Arc::clone(&completion_order);
        let mut consumed = Vec::new();

        process_batch_ordered(
            &files,
            4,
            4,
            &|file| {
                if file.sequence == 0 {
                    std::thread::sleep(Duration::from_millis(40));
                }
                decoder_completion_order.lock().unwrap().push(file.sequence);
                Ok(file.sequence)
            },
            |sequence| {
                consumed.push(sequence);
                Ok(())
            },
        )
        .unwrap();

        assert_ne!(completion_order.lock().unwrap()[0], 0);
        assert_eq!(consumed, vec![0, 1, 2, 3]);
    }

    #[test]
    fn concurrent_decode_reports_the_earliest_archive_error() {
        let files = fake_files(3);
        let error = process_batch_ordered(
            &files,
            3,
            3,
            &|file| -> Result<u32> {
                match file.sequence {
                    0 => {
                        std::thread::sleep(Duration::from_millis(30));
                        bail!("first archive error")
                    }
                    1 => bail!("later archive error"),
                    sequence => Ok(sequence),
                }
            },
            |_| Ok(()),
        )
        .unwrap_err();

        assert!(format!("{error:#}").contains("first archive error"));
    }

    #[test]
    fn schema_is_idempotent() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        migrate(&mut connection).unwrap();
        let version: String = connection
            .query_row(
                "SELECT value FROM schema_meta WHERE key='schema_version'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION.to_string());
    }

    #[test]
    fn database_rejects_network_mismatch() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        bind_network(&connection, "network-a").unwrap();
        assert!(bind_network(&connection, "network-b").is_err());
    }

    #[test]
    fn muxed_address_normalizes_to_base_account() {
        let key = stellar_xdr::Uint256([7; 32]);
        let base = stellar_xdr::MuxedAccount::Ed25519(key.clone()).to_string();
        let muxed = stellar_xdr::MuxedAccount::MuxedEd25519(stellar_xdr::MuxedAccountMed25519 {
            id: 42,
            ed25519: key,
        })
        .to_string();
        assert_eq!(normalized_account_strkey(&muxed).unwrap(), base);
    }

    #[test]
    fn rejects_broken_hash_chain() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        connection
            .execute(
                "INSERT INTO ledgers VALUES (
                    10, ?1, ?2, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 1, 'fixture'
                )",
                params![&[1_u8; 32][..], &[0_u8; 32][..]],
            )
            .unwrap();
        let transaction = connection.transaction().unwrap();
        let metrics = crate::decode::LedgerMetrics {
            sequence: 11,
            path: "fixture".to_owned(),
            ledger_hash: [2; 32],
            previous_ledger_hash: [9; 32],
            close_time: 0,
            meta_version: 2,
            transaction_count: 0,
            successful_transaction_count: 0,
            operation_count: 0,
            direct_contract_call_count: 0,
            contract_touch_count: 0,
            soroban_state_bytes: 0,
            evicted_keys: 0,
            compressed_bytes: 1,
            xdr_bytes: 1,
        };
        assert!(validate_chain(&transaction, &metrics).is_err());
    }
}
