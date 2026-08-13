use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::Parser;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

const PROGRESS_SCHEMA_VERSION: u32 = 1;
const SERVING_SCHEMA_VERSION: &str = "1";
const RECORD_BYTES: usize = 37;

#[derive(Debug, Parser)]
#[command(
    name = "stargazer-wallet-finalizer",
    about = "Build a bounded-memory wallet activity serving index from a compact Stargazer index"
)]
struct Cli {
    /// Completed compact index opened strictly read-only.
    #[arg(long, env = "STARGAZER_COMPACT_DB")]
    compact_db: PathBuf,

    /// Atomically published serving database.
    #[arg(long, env = "STARGAZER_SERVING_DB")]
    output: PathBuf,

    /// Resumable shard/checkpoint directory. Defaults beside --output.
    #[arg(long)]
    work_dir: Option<PathBuf>,

    /// Number of account partitions. Must be between 1 and 256.
    #[arg(long, default_value_t = 256)]
    shards: usize,

    /// Flush durable partition checkpoints after approximately this many exact edges.
    #[arg(long, default_value_t = 10_000_000)]
    checkpoint_edges: u64,

    /// Commit protocol/contract rollups after this many wallets.
    #[arg(long, default_value_t = 100_000)]
    commit_wallets: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Partition,
    Aggregate,
    Protocols,
    Indexes,
    Complete,
}

#[derive(Debug, Deserialize, Serialize)]
struct Progress {
    schema_version: u32,
    phase: Phase,
    last_completed_tx_id: i64,
    edges_scanned: u64,
    wallet_tx_records: u64,
    shard_lengths: Vec<u64>,
    next_shard: usize,
    last_protocol_account_hex: Option<String>,
    protocol_wallets: u64,
}

impl Progress {
    fn fresh(shards: usize) -> Self {
        Self {
            schema_version: PROGRESS_SCHEMA_VERSION,
            phase: Phase::Partition,
            last_completed_tx_id: 0,
            edges_scanned: 0,
            wallet_tx_records: 0,
            shard_lengths: vec![0; shards],
            next_shard: 0,
            last_protocol_account_hex: None,
            protocol_wallets: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct TxMetadata {
    tx_id: i64,
    successful: bool,
    ledger: u32,
}

#[derive(Clone, Copy, Debug)]
struct WalletTxRecord {
    account: [u8; 32],
    ledger: u32,
    successful: bool,
}

#[derive(Clone, Copy, Debug)]
struct WalletAggregate {
    transactions: u64,
    successful_transactions: u64,
    first_ledger: u32,
    last_ledger: u32,
}

impl WalletAggregate {
    fn from_record(record: WalletTxRecord) -> Self {
        Self {
            transactions: 1,
            successful_transactions: u64::from(record.successful),
            first_ledger: record.ledger,
            last_ledger: record.ledger,
        }
    }

    fn add(&mut self, record: WalletTxRecord) {
        self.transactions += 1;
        self.successful_transactions += u64::from(record.successful);
        self.first_ledger = self.first_ledger.min(record.ledger);
        self.last_ledger = self.last_ledger.max(record.ledger);
    }
}

struct RunPaths {
    output: PathBuf,
    partial: PathBuf,
    progress: PathBuf,
    shards: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let work = cli.work_dir.clone().unwrap_or_else(|| {
        let file_name = cli
            .output
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("stargazer-serving.sqlite");
        cli.output.with_file_name(format!("{file_name}.work"))
    });
    run(FinalizerConfig {
        compact_db: cli.compact_db,
        output: cli.output,
        work,
        shards: cli.shards,
        checkpoint_edges: cli.checkpoint_edges,
        commit_wallets: cli.commit_wallets,
    })
}

struct FinalizerConfig {
    compact_db: PathBuf,
    output: PathBuf,
    work: PathBuf,
    shards: usize,
    checkpoint_edges: u64,
    commit_wallets: usize,
}

fn run(config: FinalizerConfig) -> Result<()> {
    if !(1..=256).contains(&config.shards) {
        bail!("--shards must be between 1 and 256");
    }
    if config.checkpoint_edges == 0 {
        bail!("--checkpoint-edges must be positive");
    }
    if config.commit_wallets == 0 {
        bail!("--commit-wallets must be positive");
    }
    if !config.compact_db.is_file() {
        bail!(
            "compact database does not exist: {}",
            config.compact_db.display()
        );
    }

    let paths = build_paths(&config);
    if paths.output.exists() {
        validate_complete_serving_db(&paths.output)?;
        eprintln!(
            "serving database is already complete: {}",
            paths.output.display()
        );
        return Ok(());
    }

    fs::create_dir_all(&paths.shards)
        .with_context(|| format!("creating work directory {}", paths.shards.display()))?;
    if let Some(parent) = paths.partial.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating output directory {}", parent.display()))?;
    }

    let mut progress = load_or_initialize_progress(&paths, config.shards)?;
    let source = open_source(&config.compact_db)?;
    validate_source(&source)?;

    if progress.phase == Phase::Partition {
        partition_wallet_transactions(
            &source,
            &paths,
            &mut progress,
            config.shards,
            config.checkpoint_edges,
        )?;
    }

    let mut serving = open_serving_partial(&paths.partial)?;
    create_serving_schema(&serving)?;

    if progress.phase == Phase::Aggregate {
        aggregate_shards(&mut serving, &paths, &mut progress, config.shards)?;
    }
    if progress.phase == Phase::Protocols {
        roll_up_protocols_and_contracts(
            &source,
            &mut serving,
            &paths,
            &mut progress,
            config.commit_wallets,
        )?;
    }
    if progress.phase == Phase::Indexes {
        finalize_indexes_and_metadata(&source, &serving)?;
        progress.phase = Phase::Complete;
        save_progress(&paths.progress, &progress)?;
    }

    drop(serving);
    validate_complete_serving_db(&paths.partial)?;
    fs::rename(&paths.partial, &paths.output).with_context(|| {
        format!(
            "atomically publishing {} as {}",
            paths.partial.display(),
            paths.output.display()
        )
    })?;
    eprintln!(
        "wallet activity serving database complete: {} ({} exact wallet-transactions)",
        paths.output.display(),
        format_count(progress.wallet_tx_records)
    );
    Ok(())
}

fn build_paths(config: &FinalizerConfig) -> RunPaths {
    let partial = config.output.with_file_name(format!(
        "{}.partial",
        config
            .output
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("stargazer-serving.sqlite")
    ));
    RunPaths {
        output: config.output.clone(),
        partial,
        progress: config.work.join("progress.json"),
        shards: config.work.join("shards"),
    }
}

fn open_source(path: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening compact database {} read-only", path.display()))?;
    connection.execute_batch(
        "PRAGMA query_only=ON;
         PRAGMA cache_size=-262144;
         PRAGMA mmap_size=1073741824;
         PRAGMA temp_store=FILE;",
    )?;
    Ok(connection)
}

fn open_serving_partial(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)
        .with_context(|| format!("opening serving database {}", path.display()))?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA temp_store=FILE;
         PRAGMA cache_size=-131072;",
    )?;
    Ok(connection)
}

fn validate_source(connection: &Connection) -> Result<()> {
    let schema_version: String = connection
        .query_row(
            "SELECT value FROM compact_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .context("compact index is missing schema_version")?;
    if schema_version != "2" {
        bail!("compact schema version 2 is required, found {schema_version}");
    }
    for table in [
        "compact_ledgers",
        "contract_transactions",
        "transaction_wallet_contracts",
        "wallet_contracts",
        "protocol_contracts",
    ] {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("compact index is missing required table {table}");
        }
    }
    Ok(())
}

fn load_or_initialize_progress(paths: &RunPaths, shards: usize) -> Result<Progress> {
    if paths.progress.exists() {
        let bytes = fs::read(&paths.progress)
            .with_context(|| format!("reading {}", paths.progress.display()))?;
        let progress: Progress = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing {}", paths.progress.display()))?;
        if progress.schema_version != PROGRESS_SCHEMA_VERSION {
            bail!(
                "unsupported finalizer progress schema {}",
                progress.schema_version
            );
        }
        if progress.shard_lengths.len() != shards {
            bail!(
                "progress uses {} shards but --shards is {shards}",
                progress.shard_lengths.len()
            );
        }
        return Ok(progress);
    }

    let partial_exists = paths.partial.exists();
    let has_shards = fs::read_dir(&paths.shards)?.next().transpose()?.is_some();
    if partial_exists || has_shards {
        bail!(
            "work artifacts exist without {}; move them aside or restore the checkpoint",
            paths.progress.display()
        );
    }
    let progress = Progress::fresh(shards);
    save_progress(&paths.progress, &progress)?;
    Ok(progress)
}

fn save_progress(path: &Path, progress: &Progress) -> Result<()> {
    let parent = path.parent().context("progress path has no parent")?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(progress)?;
    {
        let mut file = File::create(&temporary)
            .with_context(|| format!("creating {}", temporary.display()))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    fs::rename(&temporary, path)
        .with_context(|| format!("publishing checkpoint {}", path.display()))?;
    Ok(())
}

fn shard_path(paths: &RunPaths, shard: usize) -> PathBuf {
    paths.shards.join(format!("wallet-tx-{shard:03}.bin"))
}

fn open_shard_writers(paths: &RunPaths, progress: &Progress) -> Result<Vec<BufWriter<File>>> {
    let mut writers = Vec::with_capacity(progress.shard_lengths.len());
    for (shard, expected_length) in progress.shard_lengths.iter().enumerate() {
        let path = shard_path(paths, shard);
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening shard {}", path.display()))?;
        file.set_len(*expected_length).with_context(|| {
            format!(
                "truncating shard {} to durable checkpoint length {expected_length}",
                path.display()
            )
        })?;
        writers.push(BufWriter::with_capacity(64 * 1024, file));
    }
    Ok(writers)
}

fn partition_wallet_transactions(
    source: &Connection,
    paths: &RunPaths,
    progress: &mut Progress,
    shard_count: usize,
    checkpoint_edges: u64,
) -> Result<()> {
    eprintln!(
        "partition phase: resuming after tx_id {} ({} edges already durable)",
        progress.last_completed_tx_id,
        format_count(progress.edges_scanned)
    );
    let max_tx_id: i64 = source.query_row(
        "SELECT COALESCE(MAX(tx_id),0) FROM contract_transactions",
        [],
        |row| row.get(0),
    )?;
    let mut writers = open_shard_writers(paths, progress)?;

    let mut edge_statement = source.prepare(
        "SELECT tx_id,account_id
         FROM transaction_wallet_contracts
         WHERE tx_id > ?1
         ORDER BY tx_id,account_id,contract_id",
    )?;
    let mut edge_rows = edge_statement.query([progress.last_completed_tx_id])?;
    let mut tx_statement = source.prepare(
        "SELECT tx_id,successful,ledger_sequence
         FROM contract_transactions
         WHERE tx_id > ?1
         ORDER BY tx_id",
    )?;
    let mut tx_rows = tx_statement.query([progress.last_completed_tx_id])?;
    let mut tx_metadata = next_tx_metadata(&mut tx_rows)?;

    let started = Instant::now();
    let started_edges = progress.edges_scanned;
    let started_tx_id = progress.last_completed_tx_id;
    let mut last_report = Instant::now();
    let mut edges_since_checkpoint = 0_u64;
    let mut current_tx_id: Option<i64> = None;
    let mut current_tx_metadata: Option<TxMetadata> = None;
    let mut current_account = [0_u8; 32];

    while let Some(row) = edge_rows.next()? {
        let tx_id: i64 = row.get(0)?;
        let account_blob = row.get_ref(1)?.as_blob()?;
        let account: [u8; 32] = account_blob
            .try_into()
            .context("transaction_wallet_contracts account_id is not 32 bytes")?;

        if current_tx_id != Some(tx_id) {
            if let (Some(previous_tx), Some(metadata)) = (current_tx_id, current_tx_metadata) {
                write_wallet_tx_record(
                    &mut writers,
                    shard_count,
                    WalletTxRecord {
                        account: current_account,
                        ledger: metadata.ledger,
                        successful: metadata.successful,
                    },
                )?;
                progress.wallet_tx_records += 1;
                if edges_since_checkpoint >= checkpoint_edges {
                    checkpoint_partition(paths, progress, &mut writers, previous_tx)?;
                    edges_since_checkpoint = 0;
                }
            }

            while tx_metadata.is_some_and(|metadata| metadata.tx_id < tx_id) {
                tx_metadata = next_tx_metadata(&mut tx_rows)?;
            }
            let metadata = tx_metadata
                .filter(|metadata| metadata.tx_id == tx_id)
                .with_context(|| format!("transaction metadata missing for tx_id {tx_id}"))?;
            current_tx_id = Some(tx_id);
            current_tx_metadata = Some(metadata);
            current_account = account;
        } else if account != current_account {
            let metadata = current_tx_metadata.context("current transaction metadata missing")?;
            write_wallet_tx_record(
                &mut writers,
                shard_count,
                WalletTxRecord {
                    account: current_account,
                    ledger: metadata.ledger,
                    successful: metadata.successful,
                },
            )?;
            progress.wallet_tx_records += 1;
            current_account = account;
        }

        progress.edges_scanned += 1;
        edges_since_checkpoint += 1;
        if last_report.elapsed() >= Duration::from_secs(30) {
            report_partition(
                progress,
                tx_id,
                max_tx_id,
                started_tx_id,
                started_edges,
                started.elapsed(),
            );
            last_report = Instant::now();
        }
    }

    if let (Some(tx_id), Some(metadata)) = (current_tx_id, current_tx_metadata) {
        write_wallet_tx_record(
            &mut writers,
            shard_count,
            WalletTxRecord {
                account: current_account,
                ledger: metadata.ledger,
                successful: metadata.successful,
            },
        )?;
        progress.wallet_tx_records += 1;
        checkpoint_partition(paths, progress, &mut writers, tx_id)?;
    } else {
        checkpoint_partition(paths, progress, &mut writers, progress.last_completed_tx_id)?;
    }
    progress.phase = Phase::Aggregate;
    progress.next_shard = 0;
    save_progress(&paths.progress, progress)?;
    report_partition(
        progress,
        progress.last_completed_tx_id,
        max_tx_id,
        started_tx_id,
        started_edges,
        started.elapsed(),
    );
    Ok(())
}

fn next_tx_metadata(rows: &mut rusqlite::Rows<'_>) -> Result<Option<TxMetadata>> {
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let ledger_i64: i64 = row.get(2)?;
    let ledger = u32::try_from(ledger_i64)
        .with_context(|| format!("ledger sequence outside u32 range: {ledger_i64}"))?;
    Ok(Some(TxMetadata {
        tx_id: row.get(0)?,
        successful: row.get::<_, i64>(1)? != 0,
        ledger,
    }))
}

fn write_wallet_tx_record(
    writers: &mut [BufWriter<File>],
    shard_count: usize,
    record: WalletTxRecord,
) -> Result<()> {
    let shard = usize::from(record.account[0]) % shard_count;
    let writer = &mut writers[shard];
    writer.write_all(&record.account)?;
    writer.write_all(&record.ledger.to_le_bytes())?;
    writer.write_all(&[u8::from(record.successful)])?;
    Ok(())
}

fn checkpoint_partition(
    paths: &RunPaths,
    progress: &mut Progress,
    writers: &mut [BufWriter<File>],
    last_completed_tx_id: i64,
) -> Result<()> {
    for (index, writer) in writers.iter_mut().enumerate() {
        writer.flush()?;
        writer.get_ref().sync_data()?;
        progress.shard_lengths[index] = writer.get_ref().metadata()?.len();
    }
    progress.last_completed_tx_id = last_completed_tx_id;
    save_progress(&paths.progress, progress)
}

fn report_partition(
    progress: &Progress,
    current_tx_id: i64,
    max_tx_id: i64,
    started_tx_id: i64,
    started_edges: u64,
    elapsed: Duration,
) {
    let seconds = elapsed.as_secs_f64().max(0.001);
    let edge_rate = progress.edges_scanned.saturating_sub(started_edges) as f64 / seconds;
    let tx_fraction = if max_tx_id > 0 {
        current_tx_id as f64 / max_tx_id as f64
    } else {
        1.0
    };
    let tx_advanced = current_tx_id.saturating_sub(started_tx_id) as f64;
    let tx_remaining = max_tx_id.saturating_sub(current_tx_id) as f64;
    let eta = if tx_advanced > 0.0 && tx_remaining > 0.0 {
        Some(Duration::from_secs_f64(
            seconds * tx_remaining / tx_advanced,
        ))
    } else {
        None
    };
    eprintln!(
        "partition progress tx_id={}/{} ({:.2}%) edges={} wallet_tx={} rate={:.0} edges/s eta={}",
        current_tx_id,
        max_tx_id,
        tx_fraction.clamp(0.0, 1.0) * 100.0,
        format_count(progress.edges_scanned),
        format_count(progress.wallet_tx_records),
        edge_rate,
        eta.map(format_duration).unwrap_or_else(|| "—".into())
    );
}

fn create_serving_schema(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS serving_meta(
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         ) WITHOUT ROWID;
         CREATE TABLE IF NOT EXISTS wallet_activity(
             account_id BLOB PRIMARY KEY CHECK(length(account_id)=32),
             transaction_count INTEGER NOT NULL CHECK(transaction_count>=0),
             successful_transaction_count INTEGER NOT NULL
                 CHECK(successful_transaction_count>=0 AND successful_transaction_count<=transaction_count),
             protocol_count INTEGER NOT NULL CHECK(protocol_count>=0),
             contract_count INTEGER NOT NULL CHECK(contract_count>=0),
             first_ledger INTEGER NOT NULL,
             last_ledger INTEGER NOT NULL
         ) WITHOUT ROWID;",
    )?;
    Ok(())
}

fn aggregate_shards(
    serving: &mut Connection,
    paths: &RunPaths,
    progress: &mut Progress,
    shard_count: usize,
) -> Result<()> {
    for shard in progress.next_shard..shard_count {
        let started = Instant::now();
        let path = shard_path(paths, shard);
        let mut reader = BufReader::with_capacity(1024 * 1024, File::open(&path)?);
        let mut aggregates = HashMap::<[u8; 32], WalletAggregate>::new();
        let mut records = 0_u64;
        while let Some(record) = read_wallet_tx_record(&mut reader)? {
            records += 1;
            aggregates
                .entry(record.account)
                .and_modify(|aggregate| aggregate.add(record))
                .or_insert_with(|| WalletAggregate::from_record(record));
        }

        let transaction = serving.transaction()?;
        {
            let mut insert = transaction.prepare(
                "INSERT OR REPLACE INTO wallet_activity(
                     account_id,transaction_count,successful_transaction_count,
                     protocol_count,contract_count,first_ledger,last_ledger
                 ) VALUES(?1,?2,?3,0,0,?4,?5)",
            )?;
            for (account, aggregate) in &aggregates {
                insert.execute(params![
                    &account[..],
                    to_i64(aggregate.transactions, "transaction_count")?,
                    to_i64(
                        aggregate.successful_transactions,
                        "successful_transaction_count"
                    )?,
                    i64::from(aggregate.first_ledger),
                    i64::from(aggregate.last_ledger),
                ])?;
            }
        }
        transaction.commit()?;
        progress.next_shard = shard + 1;
        save_progress(&paths.progress, progress)?;
        eprintln!(
            "aggregate shard {}/{}: {} records -> {} wallets in {}",
            shard + 1,
            shard_count,
            format_count(records),
            format_count(aggregates.len() as u64),
            format_duration(started.elapsed())
        );
    }
    progress.phase = Phase::Protocols;
    progress.last_protocol_account_hex = None;
    progress.protocol_wallets = 0;
    save_progress(&paths.progress, progress)?;
    Ok(())
}

fn read_wallet_tx_record(reader: &mut impl Read) -> Result<Option<WalletTxRecord>> {
    let mut buffer = [0_u8; RECORD_BYTES];
    let mut read = 0;
    while read < RECORD_BYTES {
        match reader.read(&mut buffer[read..]) {
            Ok(0) if read == 0 => return Ok(None),
            Ok(0) => bail!("truncated shard record: read {read} of {RECORD_BYTES} bytes"),
            Ok(count) => read += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let mut account = [0_u8; 32];
    account.copy_from_slice(&buffer[..32]);
    let ledger = u32::from_le_bytes(buffer[32..36].try_into().expect("fixed ledger bytes"));
    let successful = match buffer[36] {
        0 => false,
        1 => true,
        value => bail!("invalid successful byte {value} in shard record"),
    };
    Ok(Some(WalletTxRecord {
        account,
        ledger,
        successful,
    }))
}

fn roll_up_protocols_and_contracts(
    source: &Connection,
    serving: &mut Connection,
    paths: &RunPaths,
    progress: &mut Progress,
    commit_wallets: usize,
) -> Result<()> {
    let protocol_roots = load_protocol_roots(source)?;
    let resume_account = progress
        .last_protocol_account_hex
        .as_deref()
        .map(hex::decode)
        .transpose()
        .context("decoding protocol checkpoint account")?;

    let sql = if resume_account.is_some() {
        "SELECT account_id,contract_id FROM wallet_contracts
         WHERE account_id > ?1 ORDER BY account_id,contract_id"
    } else {
        "SELECT account_id,contract_id FROM wallet_contracts
         ORDER BY account_id,contract_id"
    };
    let mut statement = source.prepare(sql)?;
    let mut rows = if let Some(account) = resume_account.as_ref() {
        statement.query([account])?
    } else {
        statement.query([])?
    };

    let started = Instant::now();
    let mut current_account: Option<[u8; 32]> = None;
    let mut contract_count = 0_u64;
    let mut protocols = BTreeSet::<String>::new();
    let mut batch = Vec::<([u8; 32], u64, u64)>::with_capacity(commit_wallets);

    while let Some(row) = rows.next()? {
        let account: [u8; 32] = row
            .get_ref(0)?
            .as_blob()?
            .try_into()
            .context("wallet_contracts account_id is not 32 bytes")?;
        let contract: [u8; 32] = row
            .get_ref(1)?
            .as_blob()?
            .try_into()
            .context("wallet_contracts contract_id is not 32 bytes")?;

        if current_account.is_some_and(|current| current != account) {
            batch.push((
                current_account.expect("checked current account"),
                contract_count,
                protocols.len() as u64,
            ));
            if batch.len() >= commit_wallets {
                commit_protocol_batch(serving, &batch)?;
                progress.protocol_wallets += batch.len() as u64;
                progress.last_protocol_account_hex = batch.last().map(|entry| hex::encode(entry.0));
                save_progress(&paths.progress, progress)?;
                report_protocols(progress, started.elapsed());
                batch.clear();
            }
            contract_count = 0;
            protocols.clear();
        }

        current_account = Some(account);
        contract_count += 1;
        if let Some(ids) = protocol_roots.get(&contract) {
            protocols.extend(ids.iter().cloned());
        }
    }

    if let Some(account) = current_account {
        batch.push((account, contract_count, protocols.len() as u64));
    }
    if !batch.is_empty() {
        commit_protocol_batch(serving, &batch)?;
        progress.protocol_wallets += batch.len() as u64;
        progress.last_protocol_account_hex = batch.last().map(|entry| hex::encode(entry.0));
        save_progress(&paths.progress, progress)?;
    }
    report_protocols(progress, started.elapsed());
    progress.phase = Phase::Indexes;
    save_progress(&paths.progress, progress)?;
    Ok(())
}

fn load_protocol_roots(source: &Connection) -> Result<HashMap<[u8; 32], BTreeSet<String>>> {
    let mut statement = source.prepare(
        "SELECT contract_id,protocol_id FROM protocol_contracts ORDER BY contract_id,protocol_id",
    )?;
    let mut rows = statement.query([])?;
    let mut roots = HashMap::<[u8; 32], BTreeSet<String>>::new();
    while let Some(row) = rows.next()? {
        let contract: [u8; 32] = row
            .get_ref(0)?
            .as_blob()?
            .try_into()
            .context("protocol_contracts contract_id is not 32 bytes")?;
        roots.entry(contract).or_default().insert(row.get(1)?);
    }
    Ok(roots)
}

fn commit_protocol_batch(serving: &mut Connection, batch: &[([u8; 32], u64, u64)]) -> Result<()> {
    let transaction = serving.transaction()?;
    {
        let mut update = transaction.prepare(
            "UPDATE wallet_activity
             SET contract_count=?1,protocol_count=?2
             WHERE account_id=?3",
        )?;
        for (account, contracts, protocols) in batch {
            let changed = update.execute(params![
                to_i64(*contracts, "contract_count")?,
                to_i64(*protocols, "protocol_count")?,
                &account[..]
            ])?;
            if changed != 1 {
                bail!(
                    "wallet {} exists in wallet_contracts but not wallet_activity",
                    hex::encode(account)
                );
            }
        }
    }
    transaction.commit()?;
    Ok(())
}

fn report_protocols(progress: &Progress, elapsed: Duration) {
    let rate = progress.protocol_wallets as f64 / elapsed.as_secs_f64().max(0.001);
    eprintln!(
        "protocol rollup wallets={} rate={:.0} wallets/s checkpoint={}",
        format_count(progress.protocol_wallets),
        rate,
        progress
            .last_protocol_account_hex
            .as_deref()
            .map(|value| &value[..value.len().min(12)])
            .unwrap_or("—")
    );
}

fn finalize_indexes_and_metadata(source: &Connection, serving: &Connection) -> Result<()> {
    let (ledger_start, ledger_end): (i64, i64) = source.query_row(
        "SELECT MIN(sequence),MAX(sequence) FROM compact_ledgers",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    serving.execute_batch(
        "CREATE INDEX IF NOT EXISTS wallet_activity_by_transactions
             ON wallet_activity(transaction_count DESC,account_id ASC);
         CREATE INDEX IF NOT EXISTS wallet_activity_by_protocols
             ON wallet_activity(protocol_count DESC,transaction_count DESC,account_id ASC);",
    )?;
    let generated_at: String =
        serving.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| {
            row.get(0)
        })?;
    let transaction = serving.unchecked_transaction()?;
    for (key, value) in [
        ("schema_version", SERVING_SCHEMA_VERSION.to_owned()),
        ("source_indexed_ledger_start", ledger_start.to_string()),
        ("source_indexed_ledger_end", ledger_end.to_string()),
        ("generated_at", generated_at),
    ] {
        transaction.execute(
            "INSERT OR REPLACE INTO serving_meta(key,value) VALUES(?1,?2)",
            params![key, value],
        )?;
    }
    transaction.commit()?;
    serving.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")?;
    Ok(())
}

fn validate_complete_serving_db(path: &Path) -> Result<()> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {} for final validation", path.display()))?;
    let schema_version: Option<String> = connection
        .query_row(
            "SELECT value FROM serving_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if schema_version.as_deref() != Some(SERVING_SCHEMA_VERSION) {
        bail!("serving database {} is incomplete", path.display());
    }
    for index in [
        "wallet_activity_by_transactions",
        "wallet_activity_by_protocols",
    ] {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1)",
            [index],
            |row| row.get(0),
        )?;
        if !exists {
            bail!("serving database is missing required index {index}");
        }
    }
    Ok(())
}

fn to_i64(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).with_context(|| format!("{field} exceeds SQLite INTEGER range"))
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let remainder = seconds % 60;
    if days > 0 {
        format!("{days}d {hours:02}h")
    } else if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else if minutes > 0 {
        format!("{minutes}m {remainder:02}s")
    } else {
        format!("{remainder}s")
    }
}

fn format_count(value: u64) -> String {
    let digits = value.to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(character);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn builds_exact_wallet_activity_and_protocol_counts() -> Result<()> {
        let temp = tempdir()?;
        let source_path = temp.path().join("compact.sqlite");
        create_fixture(&source_path)?;
        let output = temp.path().join("serving.sqlite");
        let work = temp.path().join("work");

        run(FinalizerConfig {
            compact_db: source_path,
            output: output.clone(),
            work,
            shards: 4,
            checkpoint_edges: 2,
            commit_wallets: 1,
        })?;

        let serving = Connection::open(output)?;
        let a: (i64, i64, i64, i64, i64, i64) = serving.query_row(
            "SELECT transaction_count,successful_transaction_count,
                    protocol_count,contract_count,first_ledger,last_ledger
             FROM wallet_activity WHERE account_id=?1",
            [&[1_u8; 32][..]],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        assert_eq!(a, (2, 1, 2, 2, 10, 11));

        let b: (i64, i64, i64, i64, i64, i64) = serving.query_row(
            "SELECT transaction_count,successful_transaction_count,
                    protocol_count,contract_count,first_ledger,last_ledger
             FROM wallet_activity WHERE account_id=?1",
            [&[2_u8; 32][..]],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )?;
        assert_eq!(b, (1, 1, 1, 1, 12, 12));
        validate_complete_serving_db(&temp.path().join("serving.sqlite"))?;
        Ok(())
    }

    #[test]
    fn record_round_trip_rejects_truncation() -> Result<()> {
        let record = WalletTxRecord {
            account: [9; 32],
            ledger: 42,
            successful: true,
        };
        let temp = tempdir()?;
        let path = temp.path().join("record.bin");
        let mut writers = vec![BufWriter::new(File::create(&path)?)];
        write_wallet_tx_record(&mut writers, 1, record)?;
        writers[0].flush()?;
        let mut reader = File::open(&path)?;
        let decoded = read_wallet_tx_record(&mut reader)?.context("missing record")?;
        assert_eq!(decoded.account, record.account);
        assert_eq!(decoded.ledger, record.ledger);
        assert_eq!(decoded.successful, record.successful);
        assert!(read_wallet_tx_record(&mut reader)?.is_none());

        fs::write(&path, [0_u8; RECORD_BYTES - 1])?;
        assert!(read_wallet_tx_record(&mut File::open(path)?).is_err());
        Ok(())
    }

    fn create_fixture(path: &Path) -> Result<()> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "CREATE TABLE compact_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
             INSERT INTO compact_meta VALUES('schema_version','2');
             CREATE TABLE compact_ledgers(sequence INTEGER PRIMARY KEY);
             INSERT INTO compact_ledgers VALUES(10),(11),(12);
             CREATE TABLE contract_transactions(
                 tx_id INTEGER PRIMARY KEY,
                 successful INTEGER NOT NULL,
                 ledger_sequence INTEGER NOT NULL
             );
             INSERT INTO contract_transactions VALUES(1,1,10),(2,0,11),(3,1,12);
             CREATE TABLE transaction_wallet_contracts(
                 tx_id INTEGER NOT NULL,
                 account_id BLOB NOT NULL,
                 contract_id BLOB NOT NULL,
                 PRIMARY KEY(tx_id,account_id,contract_id)
             ) WITHOUT ROWID;
             CREATE TABLE wallet_contracts(
                 account_id BLOB NOT NULL,
                 contract_id BLOB NOT NULL,
                 PRIMARY KEY(account_id,contract_id)
             ) WITHOUT ROWID;
             CREATE TABLE protocol_contracts(
                 protocol_id TEXT NOT NULL,
                 contract_id BLOB NOT NULL,
                 role TEXT NOT NULL,
                 PRIMARY KEY(protocol_id,contract_id,role)
             ) WITHOUT ROWID;",
        )?;
        let a = [1_u8; 32];
        let b = [2_u8; 32];
        let x = [11_u8; 32];
        let y = [12_u8; 32];
        let z = [13_u8; 32];
        for (tx, account, contract) in [(1_i64, &a, &x), (1, &a, &y), (2, &a, &x), (3, &b, &z)] {
            connection.execute(
                "INSERT INTO transaction_wallet_contracts VALUES(?1,?2,?3)",
                params![tx, &account[..], &contract[..]],
            )?;
        }
        for (account, contract) in [(&a, &x), (&a, &y), (&b, &z)] {
            connection.execute(
                "INSERT INTO wallet_contracts VALUES(?1,?2)",
                params![&account[..], &contract[..]],
            )?;
        }
        for (protocol, contract) in [("p1", &x), ("p1", &y), ("p2", &y), ("p2", &z)] {
            connection.execute(
                "INSERT INTO protocol_contracts VALUES(?1,?2,'root')",
                params![protocol, &contract[..]],
            )?;
        }
        Ok(())
    }
}
