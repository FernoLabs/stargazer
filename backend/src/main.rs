mod anchor_idl;
mod blob_store;
mod block_backfill;
mod config;
mod context;
mod database;
mod delivery;
mod http;
mod link_preview;
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
    BlockBackfillRangeReport, GrpcBackfillReport, ProgramUpgradeHistoryBackfillReport,
    backfill_ferno_block, backfill_ferno_blocks, backfill_grpc_transactions,
    backfill_program_upgrade_history, backfill_rpc_blocks,
};
use config::Config;
use context::AppState;
use database::Database;
use std::env;
use tokio::net::TcpListener;
use tracing::{debug, info, warn};
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

    let blob_store = BlobStore::new(config.blob_store_dir.clone(), config.blob_store_s3.clone())?;
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
        Command::BackfillProgramHistory {
            program_address,
            before_slot,
            limit,
            lookback_slots,
            concurrency,
            retries,
            enqueue_deliveries,
        } => {
            let report = backfill_program_upgrade_history(
                state,
                &program_address,
                before_slot,
                limit,
                lookback_slots,
                concurrency,
                retries,
                enqueue_deliveries,
            )
            .await?;
            print_program_upgrade_history_report(&report, enqueue_deliveries);
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
        Command::RepairBlockTime { slot } => {
            match solana::repair_block_time_slot(state, slot).await? {
                Some(report) => {
                    println!("Repaired block time for slot {slot}");
                    print_block_time_update_report(&report);
                }
                None => {
                    println!("No block time available for slot {slot}");
                }
            }
            return Ok(());
        }
        Command::RepairRecentBlockTimes { limit } => {
            let report = solana::repair_recent_missing_block_times(state, limit).await?;
            print_block_time_repair_report(&report);
            return Ok(());
        }
    }

    spawn_block_time_repair(state.clone());
    spawn_sns_label_bootstrap(state.clone());

    let router = http::router(state.clone())?;
    let listener = TcpListener::bind(&config.http_bind).await?;

    tokio::try_join!(
        solana::run_indexer(state.clone()),
        delivery::run_retry_loop(state.clone()),
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
    BackfillProgramHistory {
        program_address: String,
        before_slot: Option<u64>,
        limit: usize,
        lookback_slots: u64,
        concurrency: usize,
        retries: usize,
        enqueue_deliveries: bool,
    },
    MigrateBlobs {
        vacuum: bool,
    },
    RepairBlockTime {
        slot: u64,
    },
    RepairRecentBlockTimes {
        limit: i64,
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
        "backfill-program-history" | "--backfill-program-history" => {
            let program_address = args
                .get(1)
                .context(
                    "usage: firewatch-backend backfill-program-history <program-id> [--before-slot N] [--limit N] [--lookback-slots N] [--concurrency N] [--retries N] [--deliver]",
                )?
                .to_owned();

            let mut before_slot = None;
            let mut limit = 2;
            let mut lookback_slots = 2_048;
            let mut concurrency = 4;
            let mut retries = 2;
            let mut enqueue_deliveries = false;
            let mut index = 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--deliver" => {
                        enqueue_deliveries = true;
                        index += 1;
                    }
                    "--before-slot" => {
                        let value = args
                            .get(index + 1)
                            .context("--before-slot requires a value")?;
                        before_slot = Some(
                            value
                                .parse::<u64>()
                                .context("--before-slot must be an unsigned integer")?,
                        );
                        index += 2;
                    }
                    "--limit" => {
                        let value = args.get(index + 1).context("--limit requires a value")?;
                        limit = value
                            .parse::<usize>()
                            .context("--limit must be a positive integer")?;
                        if limit == 0 {
                            bail!("--limit must be greater than zero");
                        }
                        index += 2;
                    }
                    "--lookback-slots" => {
                        let value = args
                            .get(index + 1)
                            .context("--lookback-slots requires a value")?;
                        lookback_slots = value
                            .parse::<u64>()
                            .context("--lookback-slots must be an unsigned integer")?;
                        index += 2;
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
                    unknown => bail!("unknown backfill-program-history option {unknown:?}"),
                }
            }

            Ok(Command::BackfillProgramHistory {
                program_address,
                before_slot,
                limit,
                lookback_slots,
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
        "repair-block-time" | "--repair-block-time" => {
            let slot = args
                .get(1)
                .context("usage: firewatch-backend repair-block-time <slot>")?
                .parse::<u64>()
                .context("repair block time slot must be an unsigned integer")?;
            if args.len() > 2 {
                bail!("unknown repair-block-time option; usage: repair-block-time <slot>");
            }
            Ok(Command::RepairBlockTime { slot })
        }
        "repair-recent-block-times" | "--repair-recent-block-times" => {
            let limit = match args.get(1) {
                Some(value) => value
                    .parse::<i64>()
                    .context("repair recent block time limit must be an integer")?,
                None => default_block_time_repair_limit(),
            };
            if args.len() > 2 {
                bail!(
                    "unknown repair-recent-block-times option; usage: repair-recent-block-times [limit]"
                );
            }
            Ok(Command::RepairRecentBlockTimes { limit })
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
            println!(
                "  firewatch-backend backfill-program-history <program-id> [--before-slot N] [--limit N] [--lookback-slots N] [--concurrency N] [--retries N] [--deliver]"
            );
            println!("  firewatch-backend migrate-blobs [--vacuum]");
            println!("  firewatch-backend repair-block-time <slot>");
            println!("  firewatch-backend repair-recent-block-times [limit]");
            std::process::exit(0);
        }
        unknown => bail!(
            "unknown command {unknown:?}; use `backfill-block <slot>`, `backfill-blocks <start> <end>`, `backfill-rpc <start> <end>`, `backfill-grpc <start> <end>`, `backfill-program-history <program-id>`, `migrate-blobs`, `repair-block-time <slot>`, `repair-recent-block-times [limit]`, or no command"
        ),
    }
}

fn default_block_time_repair_limit() -> i64 {
    env::var("BLOCK_TIME_REPAIR_SLOT_LIMIT")
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(5_000)
        .max(0)
}

fn spawn_block_time_repair(state: context::SharedState) {
    let limit = default_block_time_repair_limit();
    if limit == 0 {
        return;
    }

    tokio::spawn(async move {
        match solana::repair_recent_missing_block_times(state, limit).await {
            Ok(report) if report.updated_rows.total() > 0 => {
                info!(
                    checked_slots = report.checked_slots,
                    repaired_slots = report.repaired_slots,
                    missing_slots = report.missing_slots,
                    failed_slots = report.failed_slots,
                    updated_rows = report.updated_rows.total(),
                    "repaired missing block times on startup",
                );
            }
            Ok(report) => {
                debug!(
                    checked_slots = report.checked_slots,
                    missing_slots = report.missing_slots,
                    failed_slots = report.failed_slots,
                    "no missing block times repaired on startup",
                );
            }
            Err(error) => {
                warn!(error = ?error, "failed to repair missing block times on startup");
            }
        }
    });
}

fn spawn_sns_label_bootstrap(state: context::SharedState) {
    let limit = state.config.sns_bootstrap_label_limit;
    if limit == 0 {
        return;
    }

    tokio::spawn(async move {
        match name_service::bootstrap_recent_activity_labels(state, limit).await {
            Ok(resolved_count) if resolved_count > 0 => {
                info!(
                    resolved_count,
                    limit, "bootstrapped SNS labels from indexed activity",
                );
            }
            Ok(_) => {
                debug!(limit, "no SNS labels bootstrapped from indexed activity");
            }
            Err(error) => {
                warn!(error = ?error, "failed to bootstrap SNS labels on startup");
            }
        }
    });
}

fn print_block_time_repair_report(report: &solana::BlockTimeRepairReport) {
    println!("Checked slots: {}", report.checked_slots);
    println!("Repaired slots: {}", report.repaired_slots);
    println!("Slots without block time: {}", report.missing_slots);
    println!("Failed slots: {}", report.failed_slots);
    print_block_time_update_report(&report.updated_rows);
}

fn print_block_time_update_report(report: &database::BlockTimeUpdateReport) {
    println!("Updated rows: {}", report.total());
    println!("Events: {}", report.events);
    println!("Program versions: {}", report.program_versions);
    println!("Program IDL versions: {}", report.program_idl_versions);
    println!("Program data writes: {}", report.program_data_writes);
    println!("Program IDL writes: {}", report.program_idl_writes);
    println!(
        "Protocol metric observations: {}",
        report.protocol_metric_observations
    );
}

fn blob_store_label(config: &Config) -> String {
    match (&config.blob_store_dir, &config.blob_store_s3) {
        (Some(path), Some(s3)) => format!("{} and s3://{}", path.display(), s3.bucket),
        (Some(path), None) => path.display().to_string(),
        (None, Some(s3)) => format!("s3://{}", s3.bucket),
        (None, None) => "disabled blob store".to_owned(),
    }
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

fn print_program_upgrade_history_report(
    report: &ProgramUpgradeHistoryBackfillReport,
    enqueue_deliveries: bool,
) {
    let elapsed_seconds = report.elapsed_ms as f64 / 1000.0;
    let ok_blocks = report
        .windows
        .iter()
        .map(|window| window.report.ok_count())
        .sum::<usize>();
    let errored_blocks = report
        .windows
        .iter()
        .map(|window| window.report.error_count())
        .sum::<usize>();
    let transaction_count = report
        .windows
        .iter()
        .map(|window| window.report.transaction_count())
        .sum::<usize>();
    let matched_transaction_count = report
        .windows
        .iter()
        .map(|window| window.report.matched_transaction_count())
        .sum::<usize>();
    let parsed_event_count = report
        .windows
        .iter()
        .map(|window| window.report.parsed_event_count())
        .sum::<usize>();
    let inserted_event_count = report
        .windows
        .iter()
        .map(|window| window.report.inserted_event_count())
        .sum::<usize>();
    let program_data_write_count = report
        .windows
        .iter()
        .map(|window| window.report.program_data_write_count())
        .sum::<usize>();
    let program_version_update_count = report
        .windows
        .iter()
        .map(|window| window.report.program_version_update_count())
        .sum::<usize>();

    println!(
        "Backfilled program upgrade history for {}",
        report.program_address
    );
    println!("Program data account: {}", report.program_data_account);
    println!("Upgrade signatures found: {}", report.signatures.len());
    for signature in &report.signatures {
        println!("{}\tslot {}", signature.signature, signature.slot);
    }
    println!("Backfill windows: {}", report.windows.len());
    for window in &report.windows {
        println!(
            "{}..{}\t{} ok\t{} errors\t{} matched tx\t{} program versions",
            window.start_slot,
            window.end_slot,
            window.report.ok_count(),
            window.report.error_count(),
            window.report.matched_transaction_count(),
            window.report.program_version_update_count(),
        );
    }
    println!(
        "Telegram delivery: {}",
        if enqueue_deliveries { "on" } else { "off" }
    );
    println!("Elapsed: {elapsed_seconds:.2}s");
    println!("OK blocks: {ok_blocks}");
    println!("Errored blocks: {errored_blocks}");
    println!("Transactions: {transaction_count}");
    println!("Matched transactions: {matched_transaction_count}");
    println!("Parsed events: {parsed_event_count}");
    println!("Inserted events: {inserted_event_count}");
    println!("Program data writes: {program_data_write_count}");
    println!("Program version updates: {program_version_update_count}");
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
