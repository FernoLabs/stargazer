mod anchor_idl;
mod blob_store;
mod block_backfill;
mod config;
mod context;
mod database;
mod delivery;
mod http;
mod models;
mod name_service;
mod program_bytes;
mod program_disassembly;
mod program_metadata;
mod protocol_metrics;
mod protocols;
mod solana;
mod telegram;

use anyhow::{Context, Result, bail};
use blob_store::BlobStore;
use block_backfill::{
    BlockBackfillRangeReport, GrpcBackfillReport, backfill_ferno_block, backfill_ferno_blocks,
    backfill_grpc_transactions, backfill_rpc_blocks,
};
use config::Config;
use context::AppState;
use database::Database;
use std::env;
use tokio::net::TcpListener;
use tracing::debug;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let command = parse_command()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("firewatch_backend=info,backend=info,tower_http=info")
        }))
        .init();

    load_dotenv();

    let config = Config::from_env()?;
    config.prepare_dirs()?;

    let blob_store = BlobStore::new(config.blob_store_dir.clone())?;
    let db = Database::connect(&config.database_url, blob_store).await?;
    db.migrate().await?;

    let state = AppState::shared(config.clone(), db, reqwest::Client::new());

    match command {
        Command::Serve => {}
        Command::BackfillBlock {
            slot,
            enqueue_deliveries,
        } => {
            let report = backfill_ferno_block(state, slot, enqueue_deliveries).await?;
            println!("Backfilled Ferno block {}", report.slot);
            println!("Source: {}", report.url);
            println!("Transactions: {}", report.transaction_count);
            println!(
                "Skipped unwatched transactions: {}",
                report.skipped_unwatched_transaction_count
            );
            println!(
                "Skipped failed transactions: {}",
                report.skipped_failed_transaction_count
            );
            println!(
                "Skipped missing meta: {}",
                report.skipped_missing_meta_count
            );
            println!("Matched transactions: {}", report.matched_transaction_count);
            println!("Parsed events: {}", report.parsed_event_count);
            println!("Inserted events: {}", report.inserted_event_count);
            println!(
                "Parsed metric observations: {}",
                report.parsed_metric_observation_count
            );
            println!(
                "Inserted metric observations: {}",
                report.inserted_metric_observation_count
            );
            println!("Program data writes: {}", report.program_data_write_count);
            println!(
                "Program IDL data writes: {}",
                report.program_idl_data_write_count
            );
            println!(
                "Program version updates: {}",
                report.program_version_update_count
            );
            println!(
                "Program IDL version updates: {}",
                report.program_idl_version_update_count
            );
            println!(
                "Telegram delivery: {}",
                if enqueue_deliveries { "on" } else { "off" }
            );
            return Ok(());
        }
        Command::BackfillBlocks {
            start_slot,
            end_slot,
            concurrency,
            retries,
            enqueue_deliveries,
        } => {
            let report = backfill_ferno_blocks(
                state,
                start_slot,
                end_slot,
                concurrency,
                retries,
                enqueue_deliveries,
            )
            .await;
            print_range_report(&report, enqueue_deliveries);
            return Ok(());
        }
        Command::BackfillGrpc {
            start_slot,
            end_slot,
            enqueue_deliveries,
        } => {
            let report =
                backfill_grpc_transactions(state, start_slot, end_slot, enqueue_deliveries).await?;
            print_grpc_report(&report, enqueue_deliveries);
            return Ok(());
        }
        Command::BackfillRpc {
            start_slot,
            end_slot,
            concurrency,
            retries,
            enqueue_deliveries,
        } => {
            let report = backfill_rpc_blocks(
                state,
                start_slot,
                end_slot,
                concurrency,
                retries,
                enqueue_deliveries,
            )
            .await;
            print_range_report(&report, enqueue_deliveries);
            return Ok(());
        }
        Command::MigrateBlobs { vacuum } => {
            let report = state.db.migrate_inline_blobs().await?;
            println!("Migrated inline blobs to {}", blob_store_label(&config));
            println!("Program versions: {}", report.program_versions);
            println!(
                "Program IDL version bytes: {}",
                report.program_idl_version_bytes
            );
            println!(
                "Program IDL version texts: {}",
                report.program_idl_version_texts
            );
            println!("Program disassemblies: {}", report.disassemblies);
            println!("Inline bytes moved: {}", report.bytes_moved);
            if vacuum {
                state.db.vacuum().await?;
                println!("Vacuumed SQLite database");
            }
            return Ok(());
        }
    }

    let router = http::router(state.clone())?;
    let listener = TcpListener::bind(&config.http_bind).await?;

    tokio::try_join!(
        solana::run_indexer(state.clone()),
        delivery::run_retry_loop(state.clone()),
        name_service::run_indexer_loop(state.clone()),
        telegram::run_command_loop(state.clone()),
        async move {
            axum::serve(listener, router)
                .await
                .map_err(anyhow::Error::from)
        },
    )?;

    Ok(())
}

enum Command {
    Serve,
    BackfillBlock {
        slot: u64,
        enqueue_deliveries: bool,
    },
    BackfillBlocks {
        start_slot: u64,
        end_slot: u64,
        concurrency: usize,
        retries: usize,
        enqueue_deliveries: bool,
    },
    BackfillGrpc {
        start_slot: u64,
        end_slot: u64,
        enqueue_deliveries: bool,
    },
    BackfillRpc {
        start_slot: u64,
        end_slot: u64,
        concurrency: usize,
        retries: usize,
        enqueue_deliveries: bool,
    },
    MigrateBlobs {
        vacuum: bool,
    },
}

fn parse_command() -> Result<Command> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(Command::Serve);
    }

    match args[0].as_str() {
        "backfill-block" | "--backfill-block" => {
            let slot = args
                .get(1)
                .context("usage: firewatch-backend backfill-block <slot> [--deliver]")?
                .parse::<u64>()
                .context("backfill block slot must be an unsigned integer")?;
            let enqueue_deliveries = args.iter().skip(2).any(|arg| arg == "--deliver");
            Ok(Command::BackfillBlock {
                slot,
                enqueue_deliveries,
            })
        }
        "backfill-blocks" | "--backfill-blocks" => {
            let start_slot = args
                .get(1)
                .context(
                    "usage: firewatch-backend backfill-blocks <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]",
                )?
                .parse::<u64>()
                .context("backfill start slot must be an unsigned integer")?;
            let end_slot = args
                .get(2)
                .context(
                    "usage: firewatch-backend backfill-blocks <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]",
                )?
                .parse::<u64>()
                .context("backfill end slot must be an unsigned integer")?;
            if start_slot > end_slot {
                bail!("backfill start slot must be less than or equal to end slot");
            }

            let mut concurrency = 4;
            let mut retries = 2;
            let mut enqueue_deliveries = false;
            let mut index = 3;
            while index < args.len() {
                match args[index].as_str() {
                    "--deliver" => {
                        enqueue_deliveries = true;
                        index += 1;
                    }
                    "--concurrency" => {
                        let value = args
                            .get(index + 1)
                            .context("--concurrency requires a value")?;
                        concurrency = value
                            .parse::<usize>()
                            .context("--concurrency must be a positive integer")?;
                        if concurrency == 0 {
                            bail!("--concurrency must be greater than zero");
                        }
                        index += 2;
                    }
                    "--retries" => {
                        let value = args.get(index + 1).context("--retries requires a value")?;
                        retries = value
                            .parse::<usize>()
                            .context("--retries must be a non-negative integer")?;
                        index += 2;
                    }
                    unknown => bail!("unknown backfill-blocks option {unknown:?}"),
                }
            }

            Ok(Command::BackfillBlocks {
                start_slot,
                end_slot,
                concurrency,
                retries,
                enqueue_deliveries,
            })
        }
        "backfill-grpc" | "--backfill-grpc" => {
            let start_slot = args
                .get(1)
                .context(
                    "usage: firewatch-backend backfill-grpc <start-slot> <end-slot> [--deliver]",
                )?
                .parse::<u64>()
                .context("gRPC backfill start slot must be an unsigned integer")?;
            let end_slot = args
                .get(2)
                .context(
                    "usage: firewatch-backend backfill-grpc <start-slot> <end-slot> [--deliver]",
                )?
                .parse::<u64>()
                .context("gRPC backfill end slot must be an unsigned integer")?;
            if start_slot > end_slot {
                bail!("gRPC backfill start slot must be less than or equal to end slot");
            }

            let enqueue_deliveries = args.iter().skip(3).any(|arg| arg == "--deliver");
            if args.iter().skip(3).any(|arg| arg != "--deliver") {
                bail!("unknown backfill-grpc option; supported options: [--deliver]");
            }

            Ok(Command::BackfillGrpc {
                start_slot,
                end_slot,
                enqueue_deliveries,
            })
        }
        "backfill-rpc" | "--backfill-rpc" => {
            let start_slot = args
                .get(1)
                .context(
                    "usage: firewatch-backend backfill-rpc <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]",
                )?
                .parse::<u64>()
                .context("RPC backfill start slot must be an unsigned integer")?;
            let end_slot = args
                .get(2)
                .context(
                    "usage: firewatch-backend backfill-rpc <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]",
                )?
                .parse::<u64>()
                .context("RPC backfill end slot must be an unsigned integer")?;
            if start_slot > end_slot {
                bail!("RPC backfill start slot must be less than or equal to end slot");
            }

            let mut concurrency = 4;
            let mut retries = 2;
            let mut enqueue_deliveries = false;
            let mut index = 3;
            while index < args.len() {
                match args[index].as_str() {
                    "--deliver" => {
                        enqueue_deliveries = true;
                        index += 1;
                    }
                    "--concurrency" => {
                        let value = args
                            .get(index + 1)
                            .context("--concurrency requires a value")?;
                        concurrency = value
                            .parse::<usize>()
                            .context("--concurrency must be a positive integer")?;
                        if concurrency == 0 {
                            bail!("--concurrency must be greater than zero");
                        }
                        index += 2;
                    }
                    "--retries" => {
                        let value = args.get(index + 1).context("--retries requires a value")?;
                        retries = value
                            .parse::<usize>()
                            .context("--retries must be a non-negative integer")?;
                        index += 2;
                    }
                    unknown => bail!("unknown backfill-rpc option {unknown:?}"),
                }
            }

            Ok(Command::BackfillRpc {
                start_slot,
                end_slot,
                concurrency,
                retries,
                enqueue_deliveries,
            })
        }
        "migrate-blobs" | "--migrate-blobs" => {
            let mut vacuum = false;
            for arg in args.iter().skip(1) {
                match arg.as_str() {
                    "--vacuum" => vacuum = true,
                    unknown => bail!("unknown migrate-blobs option {unknown:?}"),
                }
            }
            Ok(Command::MigrateBlobs { vacuum })
        }
        "--help" | "-h" | "help" => {
            println!("Usage:");
            println!("  firewatch-backend");
            println!("  firewatch-backend backfill-block <slot> [--deliver]");
            println!(
                "  firewatch-backend backfill-blocks <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]"
            );
            println!(
                "  firewatch-backend backfill-rpc <start-slot> <end-slot> [--concurrency N] [--retries N] [--deliver]"
            );
            println!("  firewatch-backend backfill-grpc <start-slot> <end-slot> [--deliver]");
            println!("  firewatch-backend migrate-blobs [--vacuum]");
            std::process::exit(0);
        }
        unknown => bail!(
            "unknown command {unknown:?}; use `backfill-block <slot>`, `backfill-blocks <start> <end>`, `backfill-rpc <start> <end>`, `backfill-grpc <start> <end>`, `migrate-blobs`, or no command"
        ),
    }
}

fn blob_store_label(config: &Config) -> String {
    config
        .blob_store_dir
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "disabled blob store".to_owned())
}

fn print_range_report(report: &BlockBackfillRangeReport, enqueue_deliveries: bool) {
    for outcome in &report.outcomes {
        let seconds = outcome.elapsed_ms as f64 / 1000.0;
        if let Some(block_report) = outcome.report.as_ref() {
            println!(
                "{}\t{seconds:.2}s\t{} attempts\t{} tx\t{} matched\t{} parsed events\t{} inserted events",
                outcome.slot,
                outcome.attempts,
                block_report.transaction_count,
                block_report.matched_transaction_count,
                block_report.parsed_event_count,
                block_report.inserted_event_count
            );
        } else {
            println!(
                "{}\t{seconds:.2}s\t{} attempts\tERROR\t{}",
                outcome.slot,
                outcome.attempts,
                outcome.error.as_deref().unwrap_or("unknown error")
            );
        }
    }

    let elapsed_seconds = report.elapsed_ms as f64 / 1000.0;
    let ok_blocks = report.ok_count();
    let block_rate = if elapsed_seconds > 0.0 {
        ok_blocks as f64 / elapsed_seconds
    } else {
        0.0
    };
    let transaction_rate = if elapsed_seconds > 0.0 {
        report.transaction_count() as f64 / elapsed_seconds
    } else {
        0.0
    };
    println!(
        "Backfilled blocks {}..{}",
        report.start_slot, report.end_slot
    );
    println!("Concurrency: {}", report.concurrency);
    println!("Retries: {}", report.retries);
    println!(
        "Telegram delivery: {}",
        if enqueue_deliveries { "on" } else { "off" }
    );
    println!("Elapsed: {elapsed_seconds:.2}s");
    println!("OK blocks: {}", report.ok_count());
    println!("Errored blocks: {}", report.error_count());
    println!("Transactions: {}", report.transaction_count());
    println!(
        "Skipped unwatched transactions: {}",
        report
            .outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|block_report| block_report.skipped_unwatched_transaction_count)
            .sum::<usize>()
    );
    println!(
        "Matched transactions: {}",
        report.matched_transaction_count()
    );
    println!("Parsed events: {}", report.parsed_event_count());
    println!("Inserted events: {}", report.inserted_event_count());
    println!(
        "Parsed metric observations: {}",
        report.parsed_metric_observation_count()
    );
    println!(
        "Inserted metric observations: {}",
        report.inserted_metric_observation_count()
    );
    println!("Program data writes: {}", report.program_data_write_count());
    println!(
        "Program IDL data writes: {}",
        report.program_idl_data_write_count()
    );
    println!(
        "Program version updates: {}",
        report.program_version_update_count()
    );
    println!(
        "Program IDL version updates: {}",
        report.program_idl_version_update_count()
    );
    println!("Throughput: {block_rate:.2} ok blocks/s, {transaction_rate:.0} tx/s");
}

fn print_grpc_report(report: &GrpcBackfillReport, enqueue_deliveries: bool) {
    let elapsed_seconds = report.elapsed_ms as f64 / 1000.0;
    let block_rate = if elapsed_seconds > 0.0 {
        report.observed_block_count as f64 / elapsed_seconds
    } else {
        0.0
    };
    let transaction_rate = if elapsed_seconds > 0.0 {
        report.transaction_count as f64 / elapsed_seconds
    } else {
        0.0
    };

    println!(
        "Backfilled Yellowstone gRPC slots {}..{}",
        report.start_slot, report.end_slot
    );
    println!(
        "Telegram delivery: {}",
        if enqueue_deliveries { "on" } else { "off" }
    );
    println!("Elapsed: {elapsed_seconds:.2}s");
    println!("Observed blocks: {}", report.observed_block_count);
    println!("Filtered transactions: {}", report.transaction_count);
    println!("Matched transactions: {}", report.matched_transaction_count);
    println!("Parsed events: {}", report.parsed_event_count);
    println!("Inserted events: {}", report.inserted_event_count);
    println!(
        "Parsed metric observations: {}",
        report.parsed_metric_observation_count
    );
    println!(
        "Inserted metric observations: {}",
        report.inserted_metric_observation_count
    );
    println!("Program data writes: {}", report.program_data_write_count);
    println!(
        "Program IDL data writes: {}",
        report.program_idl_data_write_count
    );
    println!(
        "Program version updates: {}",
        report.program_version_update_count
    );
    println!(
        "Program IDL version updates: {}",
        report.program_idl_version_update_count
    );
    println!(
        "Last observed slot: {}",
        report
            .last_observed_slot
            .map(|slot| slot.to_string())
            .unwrap_or_else(|| "none".to_owned())
    );
    println!("Throughput: {block_rate:.2} slots/s, {transaction_rate:.0} filtered tx/s");
}

fn load_dotenv() {
    if dotenvy::dotenv().is_err() {
        let fallback_path = concat!(env!("CARGO_MANIFEST_DIR"), "/.env");
        if dotenvy::from_path(fallback_path).is_ok() {
            debug!(
                path = fallback_path,
                "loaded FireWatch backend .env from manifest directory"
            );
        }
    }
}
