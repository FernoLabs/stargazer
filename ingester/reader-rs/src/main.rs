use anyhow::{Context, Result};
use clap::{Args as ClapArgs, Parser, Subcommand};
use stargazer_ledger_reader::{
    decode_ledger, read_metrics, read_summary, ArchiveWalker, CompactIndexDatabase, IndexDatabase,
    IndexOptions, DEFAULT_MAX_XDR_BYTES, PUBLIC_NETWORK_PASSPHRASE,
};
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
#[command(about = "Stream and index Stellar Galexie ledger archives")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate files and emit checksum-bearing JSONL summaries.
    Validate(ReadArgs),
    /// Decode files and emit compact ledger metrics as JSONL.
    Inspect(InspectArgs),
    /// Build or resume a durable SQLite reverse index.
    Index(IndexArgs),
    /// Build the compact demo index with contract transactions and executable history only.
    CompactIndex(CompactIndexArgs),
    /// Benchmark streaming decode and fact extraction without database I/O.
    Benchmark(BenchmarkArgs),
    /// Query the reverse index for a wallet's direct contract interactions.
    Wallet(WalletArgs),
}

#[derive(Debug, ClapArgs)]
struct CompactIndexArgs {
    #[command(flatten)]
    index: IndexArgs,

    /// Protocol contract seed used for protocol-level usage aggregates.
    #[arg(long)]
    protocol_roots: Option<PathBuf>,
}

#[derive(Debug, Clone, ClapArgs)]
struct ReadArgs {
    /// Root containing Galexie partition directories or individual fixtures.
    #[arg(long, env = "STARGAZER_ARCHIVE_ROOT")]
    root: PathBuf,

    /// Stop after this many ledger files.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, ClapArgs)]
struct InspectArgs {
    #[command(flatten)]
    read: ReadArgs,

    /// Output JSONL path. Defaults to stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Debug, ClapArgs)]
struct IndexArgs {
    /// Root containing Galexie partition directories or individual fixtures.
    #[arg(long, env = "STARGAZER_ARCHIVE_ROOT")]
    root: PathBuf,

    /// SQLite index path. Existing ledgers are skipped automatically.
    #[arg(long, env = "STARGAZER_INDEX_DB")]
    db: PathBuf,

    /// Maximum number of new ledgers to index in this run.
    #[arg(long)]
    limit: Option<usize>,

    /// Ledgers committed atomically per SQLite transaction.
    #[arg(long, default_value_t = 200)]
    batch_ledgers: usize,

    /// Ledger decoder threads. SQLite writes remain single-threaded and ordered.
    #[arg(long, default_value_t = default_decode_workers())]
    decode_workers: usize,

    /// Maximum queued, decoding, or decoded ledgers held by the pipeline.
    /// Defaults to twice --decode-workers.
    #[arg(long)]
    decode_ahead: Option<usize>,

    /// Retries for files that may still be arriving from the downloader.
    #[arg(long, default_value_t = 3)]
    decode_retries: usize,

    #[arg(long, default_value_t = 500)]
    retry_delay_ms: u64,

    /// Reject a ledger whose decoded XDR allocations exceed this many MiB.
    #[arg(long, default_value_t = DEFAULT_MAX_XDR_BYTES / 1024 / 1024)]
    max_xdr_mib: usize,

    #[arg(long, env = "STELLAR_NETWORK_PASSPHRASE", default_value = PUBLIC_NETWORK_PASSPHRASE)]
    network_passphrase: String,
}

#[derive(Debug, ClapArgs)]
struct BenchmarkArgs {
    #[arg(long)]
    root: PathBuf,

    /// Ledger files per iteration.
    #[arg(long, default_value_t = 3)]
    limit: usize,

    #[arg(long, default_value_t = 10)]
    iterations: usize,

    #[arg(long, default_value_t = DEFAULT_MAX_XDR_BYTES / 1024 / 1024)]
    max_xdr_mib: usize,

    #[arg(long, env = "STELLAR_NETWORK_PASSPHRASE", default_value = PUBLIC_NETWORK_PASSPHRASE)]
    network_passphrase: String,
}

#[derive(Debug, ClapArgs)]
struct WalletArgs {
    #[arg(long, env = "STARGAZER_INDEX_DB")]
    db: PathBuf,

    /// Stellar G... or M... account address.
    #[arg(long)]
    account: String,

    /// Include recent transaction summaries in addition to contract totals.
    #[arg(long)]
    include_transactions: bool,

    #[arg(long, default_value_t = 100)]
    transaction_limit: usize,
}

fn validate(args: ReadArgs) -> Result<()> {
    let limit = args.limit.unwrap_or(usize::MAX);
    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    for file in ArchiveWalker::new(&args.root)?.take(limit) {
        let file = file?;
        let summary =
            read_summary(&file).with_context(|| format!("validating ledger {}", file.sequence))?;
        serde_json::to_writer(&mut output, &summary)?;
        output.write_all(b"\n")?;
    }
    output.flush()?;
    Ok(())
}

fn inspect(args: InspectArgs) -> Result<()> {
    let limit = args.read.limit.unwrap_or(usize::MAX);
    let mut output: Box<dyn Write> = match args.output {
        Some(path) => Box::new(BufWriter::new(
            File::create(&path).with_context(|| format!("creating {}", path.display()))?,
        )),
        None => Box::new(BufWriter::new(io::stdout().lock())),
    };
    for file in ArchiveWalker::new(&args.read.root)?.take(limit) {
        let file = file?;
        let metrics =
            read_metrics(&file).with_context(|| format!("inspecting ledger {}", file.sequence))?;
        serde_json::to_writer(&mut output, &metrics)?;
        output.write_all(b"\n")?;
    }
    output.flush()?;
    Ok(())
}

fn index(args: IndexArgs) -> Result<()> {
    anyhow::ensure!(
        args.decode_workers > 0,
        "--decode-workers must be greater than zero"
    );
    let decode_ahead = args
        .decode_ahead
        .unwrap_or_else(|| args.decode_workers.saturating_mul(2));
    anyhow::ensure!(
        decode_ahead >= args.decode_workers,
        "--decode-ahead must be at least --decode-workers"
    );
    let mut database = IndexDatabase::open(&args.db)?;
    let options = IndexOptions {
        archive_root: args.root,
        network_passphrase: args.network_passphrase,
        max_xdr_bytes: args.max_xdr_mib.saturating_mul(1024 * 1024),
        batch_ledgers: args.batch_ledgers,
        limit: args.limit,
        decode_workers: args.decode_workers,
        decode_ahead,
        decode_retries: args.decode_retries,
        retry_delay: Duration::from_millis(args.retry_delay_ms),
    };
    let stats = database.run(&options, |stats| {
        eprintln!(
            "indexed={} skipped={} tx={} ops={} {:.1} ledgers/s {:.1} MiB/s",
            stats.indexed_ledgers,
            stats.skipped_ledgers,
            stats.transactions,
            stats.operations,
            stats.ledgers_per_second(),
            stats.xdr_mib_per_second(),
        );
    })?;
    eprintln!(
        "complete: indexed={} skipped={} tx={} ops={} contract_calls={} contract_touches={} elapsed={:.2}s rate={:.1} ledgers/s xdr={:.1} MiB/s",
        stats.indexed_ledgers,
        stats.skipped_ledgers,
        stats.transactions,
        stats.operations,
        stats.direct_contract_calls,
        stats.contract_touches,
        stats.elapsed.as_secs_f64(),
        stats.ledgers_per_second(),
        stats.xdr_mib_per_second(),
    );
    Ok(())
}

fn compact_index(args: CompactIndexArgs) -> Result<()> {
    let protocol_roots = args.protocol_roots;
    let args = args.index;
    anyhow::ensure!(
        args.decode_workers > 0,
        "--decode-workers must be greater than zero"
    );
    let decode_ahead = args
        .decode_ahead
        .unwrap_or_else(|| args.decode_workers.saturating_mul(2));
    anyhow::ensure!(
        decode_ahead >= args.decode_workers,
        "--decode-ahead must be at least --decode-workers"
    );
    let mut database = CompactIndexDatabase::open(&args.db, protocol_roots.as_deref())?;
    let options = IndexOptions {
        archive_root: args.root,
        network_passphrase: args.network_passphrase,
        max_xdr_bytes: args.max_xdr_mib.saturating_mul(1024 * 1024),
        batch_ledgers: args.batch_ledgers,
        limit: args.limit,
        decode_workers: args.decode_workers,
        decode_ahead,
        decode_retries: args.decode_retries,
        retry_delay: Duration::from_millis(args.retry_delay_ms),
    };
    let stats = database.run(&options, |stats| {
        eprintln!(
            "indexed={} skipped={} contract_tx={} wallet_edges={} contract_edges={} versions={} wasm={} {:.1} ledgers/s",
            stats.indexed_ledgers,
            stats.skipped_ledgers,
            stats.contract_transactions,
            stats.wallet_edges,
            stats.contract_edges,
            stats.executable_changes,
            stats.wasm_modules,
            stats.ledgers_per_second(),
        );
    })?;
    eprintln!(
        "complete: indexed={} skipped={} contract_tx={} wallet_edges={} contract_edges={} versions={} wasm={} elapsed={:.2}s rate={:.1} ledgers/s",
        stats.indexed_ledgers,
        stats.skipped_ledgers,
        stats.contract_transactions,
        stats.wallet_edges,
        stats.contract_edges,
        stats.executable_changes,
        stats.wasm_modules,
        stats.elapsed.as_secs_f64(),
        stats.ledgers_per_second(),
    );
    Ok(())
}

fn default_decode_workers() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(2)
}

fn percentile(sorted: &[Duration], percentile: usize) -> Duration {
    let index = (sorted.len().saturating_sub(1) * percentile) / 100;
    sorted[index]
}

fn benchmark(args: BenchmarkArgs) -> Result<()> {
    anyhow::ensure!(args.limit > 0, "--limit must be greater than zero");
    anyhow::ensure!(
        args.iterations > 0,
        "--iterations must be greater than zero"
    );
    let started = Instant::now();
    let mut latencies = Vec::with_capacity(args.limit.saturating_mul(args.iterations));
    let mut xdr_bytes = 0_u64;
    let mut transactions = 0_u64;
    let mut operations = 0_u64;

    for _ in 0..args.iterations {
        let mut seen = 0;
        for file in ArchiveWalker::new(&args.root)?.take(args.limit) {
            let file = file?;
            let ledger_started = Instant::now();
            let ledger = decode_ledger(
                &file,
                &args.network_passphrase,
                args.max_xdr_mib.saturating_mul(1024 * 1024),
            )?;
            latencies.push(ledger_started.elapsed());
            xdr_bytes += ledger.metrics.xdr_bytes as u64;
            transactions += ledger.metrics.transaction_count as u64;
            operations += ledger.metrics.operation_count as u64;
            std::hint::black_box(ledger);
            seen += 1;
        }
        anyhow::ensure!(
            seen == args.limit,
            "archive contains only {seen} files, but --limit requested {}",
            args.limit
        );
    }
    latencies.sort_unstable();
    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f64().max(f64::EPSILON);
    let ledgers = latencies.len() as f64;
    eprintln!(
        "benchmark: ledgers={} iterations={} elapsed={:.3}s rate={:.1} ledgers/s xdr={:.1} MiB/s tx={:.0}/s ops={:.0}/s latency_ms[p50={:.2} p95={:.2} p99={:.2}]",
        latencies.len(),
        args.iterations,
        seconds,
        ledgers / seconds,
        (xdr_bytes as f64 / 1_048_576.0) / seconds,
        transactions as f64 / seconds,
        operations as f64 / seconds,
        percentile(&latencies, 50).as_secs_f64() * 1000.0,
        percentile(&latencies, 95).as_secs_f64() * 1000.0,
        percentile(&latencies, 99).as_secs_f64() * 1000.0,
    );
    Ok(())
}

fn wallet(args: WalletArgs) -> Result<()> {
    let database = IndexDatabase::open(&args.db)?;
    let protocols = database.wallet_protocols(&args.account)?;
    let transactions = if args.include_transactions {
        Some(database.wallet_transactions(&args.account, args.transaction_limit)?)
    } else {
        None
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "account": args.account,
            "contracts": protocols,
            "transactions": transactions,
        }))?
    );
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Validate(args) => validate(args),
        Command::Inspect(args) => inspect(args),
        Command::Index(args) => index(args),
        Command::CompactIndex(args) => compact_index(args),
        Command::Benchmark(args) => benchmark(args),
        Command::Wallet(args) => wallet(args),
    }
}
