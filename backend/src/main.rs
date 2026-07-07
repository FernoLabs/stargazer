mod anchor_idl;
mod blob_store;
mod block_backfill;
mod config;
mod context;
mod database;
mod delivery;
mod http;
mod link_preview;
mod liquidity_pools;
mod models;
mod name_service;
mod program_authority;
mod program_bytes;
mod program_disassembly;
mod program_metadata;
mod protocol_governance;
mod protocol_metrics;
mod protocols;
mod solana;
mod telegram;
mod token_liquidity;

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
use liquidity_pools::{
    LiquidityPoolDimensionReport, LiquidityPoolSeedOptions, LiquidityPoolSeedReport,
    default_seed_mint_label, dimension_liquidity_pools, seed_liquidity_pools,
};
use program_authority::{ProtocolAuthoritySyncReport, sync_protocol_upgrade_authority_multisigs};
use protocol_governance::{ProtocolGovernanceSyncReport, sync_protocol_governance_realms};
use serde_json::json;
use std::env;
use token_liquidity::{
    LOMBARD_LBTC_MINT, LOMBARD_LBTC_SYMBOL, PoolBuyPressureReport, PoolLiquidityTransition,
    TokenLiquidityScanOptions, TokenLiquidityScanReport, format_amount, format_usd,
    scan_token_liquidity,
};
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

    let api_only = matches!(&command, Command::ServeApi);

    match command {
        Command::Serve | Command::ServeApi => {}
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
            println!(
                "Parsed liquidity pool observations: {}",
                report.parsed_liquidity_pool_observation_count
            );
            println!(
                "Inserted liquidity pools: {}",
                report.inserted_liquidity_pool_count
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
                "Program data writes compacted: {}",
                report.program_data_writes
            );
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
        Command::DimensionLiquidityPools => {
            let report = dimension_liquidity_pools(state).await?;
            print_liquidity_pool_dimension_report(&report);
            return Ok(());
        }
        Command::PruneLiquiditySnapshots => {
            let deleted_count = state
                .db
                .prune_liquidity_pool_snapshots(
                    state.config.liquidity_pool_snapshot_retention_days,
                    state.config.watched_liquidity_pool_snapshot_retention_days,
                )
                .await?;
            println!("Pruned liquidity pool snapshots: {deleted_count}");
            println!(
                "Default retention: {} days",
                state.config.liquidity_pool_snapshot_retention_days
            );
            println!(
                "Watched retention: {} days",
                state.config.watched_liquidity_pool_snapshot_retention_days
            );
            return Ok(());
        }
        Command::SeedLiquidityPools { mint, include_all } => {
            let report =
                seed_liquidity_pools(state, LiquidityPoolSeedOptions { mint, include_all }).await?;
            print_liquidity_pool_seed_report(&report);
            return Ok(());
        }
        Command::SyncProgramAuthorities {
            vault_index_count,
            discover_all_squads,
        } => {
            let report = sync_protocol_upgrade_authority_multisigs(
                state,
                vault_index_count,
                discover_all_squads,
            )
            .await?;
            print_protocol_authority_sync_report(&report);
            return Ok(());
        }
        Command::SyncGovernanceRealms => {
            let report = sync_protocol_governance_realms(state).await?;
            print_protocol_governance_sync_report(&report);
            return Ok(());
        }
        Command::ScanTokenLiquidity {
            mint,
            symbol,
            lookback_days,
            min_token_reserve,
            min_token_reserve_usd,
            max_signature_pages,
            wallet_addresses,
            include_wallet_subscriptions,
            emit_events,
            enqueue_deliveries,
        } => {
            let report = scan_token_liquidity(
                state,
                TokenLiquidityScanOptions {
                    mint,
                    symbol,
                    lookback_days,
                    min_token_reserve,
                    min_token_reserve_usd,
                    max_signature_pages,
                    wallet_addresses,
                    include_wallet_subscriptions,
                    emit_events,
                    enqueue_deliveries,
                },
            )
            .await?;
            print_token_liquidity_report(&report);
            return Ok(());
        }
    }

    if !api_only {
        let suppressed_false_positive_deliveries = state
            .db
            .mark_raydium_amm_v4_swap_false_positive_deliveries(models::now_ts())
            .await?;
        if suppressed_false_positive_deliveries > 0 {
            warn!(
                count = suppressed_false_positive_deliveries,
                "marked Raydium AMM v4 swap false-positive telegram deliveries as delivered"
            );
        }
        let suppressed_false_positive_deliveries = state
            .db
            .mark_raydium_clmm_known_false_positive_deliveries(models::now_ts())
            .await?;
        if suppressed_false_positive_deliveries > 0 {
            warn!(
                count = suppressed_false_positive_deliveries,
                "marked Raydium CLMM known false-positive deliveries as delivered"
            );
        }
        let suppressed_false_positive_deliveries = state
            .db
            .mark_kamino_reserve_config_parser_false_positive_deliveries(models::now_ts())
            .await?;
        if suppressed_false_positive_deliveries > 0 {
            warn!(
                count = suppressed_false_positive_deliveries,
                "marked Kamino reserve config parser false-positive deliveries as delivered"
            );
        }

        spawn_block_time_repair(state.clone());
        spawn_known_incident_recovery(state.clone());
        spawn_sns_label_bootstrap(state.clone());
        spawn_liquidity_pool_snapshot_prune(state.clone());
    }

    let router = http::router(state.clone())?;
    let listener = TcpListener::bind(&config.http_bind).await?;

    if api_only {
        info!(bind = %config.http_bind, "serving HTTP API only");
        axum::serve(listener, router)
            .await
            .map_err(anyhow::Error::from)?;
        return Ok(());
    }

    tokio::try_join!(
        solana::run_indexer(state.clone()),
        delivery::run_retry_loop(state.clone()),
        delivery::run_daily_summary_loop(state.clone()),
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
    ServeApi,
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
    DimensionLiquidityPools,
    PruneLiquiditySnapshots,
    SeedLiquidityPools {
        mint: Option<String>,
        include_all: bool,
    },
    SyncProgramAuthorities {
        vault_index_count: u8,
        discover_all_squads: bool,
    },
    SyncGovernanceRealms,
    ScanTokenLiquidity {
        mint: String,
        symbol: String,
        lookback_days: i64,
        min_token_reserve: Option<f64>,
        min_token_reserve_usd: Option<f64>,
        max_signature_pages: usize,
        wallet_addresses: Vec<String>,
        include_wallet_subscriptions: bool,
        emit_events: bool,
        enqueue_deliveries: bool,
    },
}

fn parse_command() -> Result<Command> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(Command::Serve);
    }

    match args[0].as_str() {
        "serve" | "--serve" => Ok(Command::Serve),
        "serve-api" | "--serve-api" | "api" | "--api" => Ok(Command::ServeApi),
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
        "dimension-liquidity-pools" | "--dimension-liquidity-pools" => {
            if args.len() > 1 {
                bail!("unknown dimension-liquidity-pools option; usage: dimension-liquidity-pools");
            }
            Ok(Command::DimensionLiquidityPools)
        }
        "prune-liquidity-snapshots" | "--prune-liquidity-snapshots" => {
            if args.len() > 1 {
                bail!("unknown prune-liquidity-snapshots option; usage: prune-liquidity-snapshots");
            }
            Ok(Command::PruneLiquiditySnapshots)
        }
        "seed-liquidity-pools" | "--seed-liquidity-pools" => {
            let mut mint = None;
            let mut include_all = false;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--mint" => {
                        mint = Some(
                            args.get(index + 1)
                                .context("--mint requires a value")?
                                .to_owned(),
                        );
                        index += 2;
                    }
                    "--all" => {
                        include_all = true;
                        index += 1;
                    }
                    unknown => bail!("unknown seed-liquidity-pools option {unknown:?}"),
                }
            }
            if include_all && mint.is_some() {
                bail!("use either seed-liquidity-pools --all or --mint MINT");
            }

            Ok(Command::SeedLiquidityPools { mint, include_all })
        }
        "sync-program-authorities" | "--sync-program-authorities" => {
            let mut vault_index_count = 8_u8;
            let mut discover_all_squads = false;
            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--discover-all-squads" => {
                        discover_all_squads = true;
                        index += 1;
                    }
                    "--vault-index-count" => {
                        let value = args
                            .get(index + 1)
                            .context("--vault-index-count requires a value")?;
                        vault_index_count = value
                            .parse::<u8>()
                            .context("--vault-index-count must be between 1 and 255")?;
                        if vault_index_count == 0 {
                            bail!("--vault-index-count must be greater than zero");
                        }
                        index += 2;
                    }
                    unknown => bail!("unknown sync-program-authorities option {unknown:?}"),
                }
            }

            Ok(Command::SyncProgramAuthorities {
                vault_index_count,
                discover_all_squads,
            })
        }
        "sync-governance-realms" | "--sync-governance-realms" => Ok(Command::SyncGovernanceRealms),
        "scan-token-liquidity" | "--scan-token-liquidity" => {
            let mut mint = LOMBARD_LBTC_MINT.to_owned();
            let mut symbol = LOMBARD_LBTC_SYMBOL.to_owned();
            let mut lookback_days = 30_i64;
            let mut min_token_reserve = Some(0.1_f64);
            let mut min_token_reserve_usd = Some(10_000.0_f64);
            let mut max_signature_pages = 100_usize;
            let mut wallet_addresses = Vec::new();
            let mut include_wallet_subscriptions = false;
            let mut emit_events = false;
            let mut enqueue_deliveries = false;

            let mut index = 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--mint" => {
                        mint = args
                            .get(index + 1)
                            .context("--mint requires a value")?
                            .to_owned();
                        index += 2;
                    }
                    "--symbol" => {
                        symbol = args
                            .get(index + 1)
                            .context("--symbol requires a value")?
                            .to_owned();
                        index += 2;
                    }
                    "--lookback-days" => {
                        lookback_days = args
                            .get(index + 1)
                            .context("--lookback-days requires a value")?
                            .parse::<i64>()
                            .context("--lookback-days must be a positive integer")?;
                        if lookback_days <= 0 {
                            bail!("--lookback-days must be greater than zero");
                        }
                        index += 2;
                    }
                    "--min-token-reserve" => {
                        min_token_reserve = Some(
                            args.get(index + 1)
                                .context("--min-token-reserve requires a value")?
                                .parse::<f64>()
                                .context("--min-token-reserve must be a number")?,
                        );
                        index += 2;
                    }
                    "--min-token-reserve-usd" => {
                        min_token_reserve_usd = Some(
                            args.get(index + 1)
                                .context("--min-token-reserve-usd requires a value")?
                                .parse::<f64>()
                                .context("--min-token-reserve-usd must be a number")?,
                        );
                        index += 2;
                    }
                    "--max-signature-pages" => {
                        max_signature_pages = args
                            .get(index + 1)
                            .context("--max-signature-pages requires a value")?
                            .parse::<usize>()
                            .context("--max-signature-pages must be a positive integer")?;
                        if max_signature_pages == 0 {
                            bail!("--max-signature-pages must be greater than zero");
                        }
                        index += 2;
                    }
                    "--wallet" => {
                        wallet_addresses.push(
                            args.get(index + 1)
                                .context("--wallet requires a value")?
                                .to_owned(),
                        );
                        index += 2;
                    }
                    "--wallet-subscriptions" => {
                        include_wallet_subscriptions = true;
                        index += 1;
                    }
                    "--emit" => {
                        emit_events = true;
                        index += 1;
                    }
                    "--deliver" => {
                        emit_events = true;
                        enqueue_deliveries = true;
                        index += 1;
                    }
                    unknown => bail!("unknown scan-token-liquidity option {unknown:?}"),
                }
            }

            Ok(Command::ScanTokenLiquidity {
                mint,
                symbol,
                lookback_days,
                min_token_reserve,
                min_token_reserve_usd,
                max_signature_pages,
                wallet_addresses,
                include_wallet_subscriptions,
                emit_events,
                enqueue_deliveries,
            })
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
            println!("  firewatch-backend dimension-liquidity-pools");
            println!("  firewatch-backend prune-liquidity-snapshots");
            println!("  firewatch-backend seed-liquidity-pools [--mint MINT | --all]");
            println!(
                "  firewatch-backend sync-program-authorities [--discover-all-squads] [--vault-index-count N]"
            );
            println!("  firewatch-backend sync-governance-realms");
            println!(
                "  firewatch-backend scan-token-liquidity [--mint MINT] [--symbol SYMBOL] [--lookback-days N] [--min-token-reserve N] [--min-token-reserve-usd N] [--max-signature-pages N] [--wallet ADDRESS] [--wallet-subscriptions] [--emit] [--deliver]"
            );
            std::process::exit(0);
        }
        unknown => bail!(
            "unknown command {unknown:?}; use `backfill-block <slot>`, `backfill-blocks <start> <end>`, `backfill-rpc <start> <end>`, `backfill-grpc <start> <end>`, `backfill-program-history <program-id>`, `migrate-blobs`, `repair-block-time <slot>`, `repair-recent-block-times [limit]`, `dimension-liquidity-pools`, `prune-liquidity-snapshots`, `seed-liquidity-pools`, `sync-program-authorities`, `sync-governance-realms`, `scan-token-liquidity`, or no command"
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

const BONK_DAO_INCIDENT_SLOT: u64 = 431_127_240;
const BONK_DAO_INCIDENT_BLOCK_TIME: i64 = 1_783_326_388;
const BONK_DAO_INCIDENT_SIGNATURE: &str =
    "5tPU1srcRcnmibB7KJi2WQ7cK4zuq5iKTMrSCLjq7hvGjuK4KUTmaHfwicixkJa5jZJmp3y98T7r2qecKV5mWw8P";
const BONK_DAO_INCIDENT_INSTRUCTION_PATH: &str = "2.0";
const BONK_DAO_INCIDENT_SOURCE_TOKEN_ACCOUNT: &str = "F8FqZuUKfoy58aHLW6bfeEhfW9sTtJyqFTqnxVmGZ6dU";
const BONK_DAO_INCIDENT_DESTINATION_TOKEN_ACCOUNT: &str =
    "28AymsqjJ6p312raqaNUNn8DADT4kyRAwT2nJ87scmPy";
const BONK_DAO_INCIDENT_RAW_AMOUNT: &str = "442610445030596600";
const BONK_DAO_INCIDENT_DISPLAY_AMOUNT: &str = "4426104450305.966";
const BONK_DAO_INCIDENT_EVENT_ID: &str = "5tPU1srcRcnmibB7KJi2WQ7cK4zuq5iKTMrSCLjq7hvGjuK4KUTmaHfwicixkJa5jZJmp3y98T7r2qecKV5mWw8P:2.0:treasury_token_outflow:AGkGWK1R669KDT4FCqgDgK7PgahGJPjD4J9xmVjuL9kn";
const BONK_DAO_PROPOSAL_SLOT: u64 = 429_842_335;
const BONK_DAO_PROPOSAL_BLOCK_TIME: i64 = 1_782_807_924;
const BONK_DAO_PROPOSAL_SIGNATURE: &str =
    "3bvjf8Xma3QCbp4PJpKS8w5hsAk8exa2vZ5eoKgHhRcCaj5Cs9UxCZszg1pxtT1mngw3NP5j2JQJjRnBxmyQD2r2";
const BONK_DAO_PROPOSAL_INSTRUCTION_PATH: &str = "3";
const BONK_DAO_PROPOSAL_ADDRESS: &str = "6wR1jdhhJ31bbdRNXva8MxqsgsNLKTxargcdAyZ7FcRj";
const BONK_DAO_PROPOSAL_TOKEN_OWNER_RECORD: &str = "7wxcqy3es8nk276atyWPwrw6BsgLspRcvzmKGp2XYULh";
const BONK_DAO_PROPOSAL_ACTOR: &str = "8xxRdtzsw1CJWfEahViqNjZwh5dYJAdSbBctWwcbycVo";
const BONK_DAO_PROPOSAL_EVENT_ID: &str = "3bvjf8Xma3QCbp4PJpKS8w5hsAk8exa2vZ5eoKgHhRcCaj5Cs9UxCZszg1pxtT1mngw3NP5j2JQJjRnBxmyQD2r2:3:program_admin_instruction:GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw";

fn spawn_known_incident_recovery(state: context::SharedState) {
    tokio::spawn(async move {
        recover_bonk_dao_proposal_creation_alert(state.clone()).await;
        recover_bonk_dao_treasury_outflow_alert(state).await;
    });
}

async fn recover_known_bonk_dao_event(
    state: &context::SharedState,
    slot: u64,
    signature: &str,
    event_id: &str,
    label: &str,
) -> bool {
    if known_incident_alert_exists(state, event_id)
        .await
        .unwrap_or(false)
    {
        debug!(label, "Bonk DAO known incident alert already indexed");
        return true;
    }

    info!(
        slot,
        signature, label, "recovering missing Bonk DAO known incident alert",
    );

    match backfill_ferno_block(state.clone(), slot, false).await {
        Ok(report) => {
            info!(
                slot,
                label,
                parsed_events = report.parsed_event_count,
                inserted_events = report.inserted_event_count,
                "ran Ferno block recovery backfill for Bonk DAO known incident slot",
            );
        }
        Err(error) => {
            warn!(
                slot,
                label,
                error = ?error,
                "Ferno block recovery backfill could not load Bonk DAO known incident slot",
            );
        }
    }

    if known_incident_alert_exists(state, event_id)
        .await
        .unwrap_or(false)
    {
        return true;
    }

    let report = backfill_rpc_blocks(state.clone(), slot, slot, 1, 1, false).await;
    if report.ok_count() > 0 {
        info!(
            slot,
            label,
            parsed_events = report.parsed_event_count(),
            inserted_events = report.inserted_event_count(),
            "ran RPC recovery backfill for Bonk DAO known incident slot",
        );
    } else {
        warn!(
            slot,
            label,
            errors = report.error_count(),
            "RPC recovery backfill could not load Bonk DAO known incident slot",
        );
    }

    known_incident_alert_exists(state, event_id)
        .await
        .unwrap_or(false)
}

async fn recover_bonk_dao_proposal_creation_alert(state: context::SharedState) {
    if recover_known_bonk_dao_event(
        &state,
        BONK_DAO_PROPOSAL_SLOT,
        BONK_DAO_PROPOSAL_SIGNATURE,
        BONK_DAO_PROPOSAL_EVENT_ID,
        "proposal_creation",
    )
    .await
    {
        return;
    }

    match insert_known_bonk_dao_proposal_creation_event(&state).await {
        Ok(true) => {
            warn!(
                slot = BONK_DAO_PROPOSAL_SLOT,
                signature = BONK_DAO_PROPOSAL_SIGNATURE,
                "inserted verified Bonk DAO proposal creation fallback",
            );
        }
        Ok(false) => {
            debug!("Bonk DAO proposal creation fallback was already present");
        }
        Err(error) => {
            warn!(
                error = ?error,
                "failed to insert verified Bonk DAO proposal creation fallback",
            );
        }
    }
}

async fn recover_bonk_dao_treasury_outflow_alert(state: context::SharedState) {
    if recover_known_bonk_dao_event(
        &state,
        BONK_DAO_INCIDENT_SLOT,
        BONK_DAO_INCIDENT_SIGNATURE,
        BONK_DAO_INCIDENT_EVENT_ID,
        "treasury_outflow",
    )
    .await
    {
        return;
    }

    match insert_known_bonk_dao_treasury_outflow_event(&state).await {
        Ok(true) => {
            warn!(
                slot = BONK_DAO_INCIDENT_SLOT,
                signature = BONK_DAO_INCIDENT_SIGNATURE,
                "inserted verified Bonk DAO treasury outflow incident fallback",
            );
        }
        Ok(false) => {
            debug!("Bonk DAO incident fallback was already present");
        }
        Err(error) => {
            warn!(
                error = ?error,
                "failed to insert verified Bonk DAO treasury outflow incident fallback",
            );
        }
    }
}

async fn known_incident_alert_exists(state: &context::SharedState, event_id: &str) -> Result<bool> {
    Ok(state.db.event_by_id(event_id).await?.is_some())
}

async fn insert_known_bonk_dao_proposal_creation_event(
    state: &context::SharedState,
) -> Result<bool> {
    let indexed_at = models::now_ts();
    let mut tags = vec![
        "impact:high".to_owned(),
        "instruction:create_proposal".to_owned(),
        format!("protocol:{}", protocols::BONK_DAO_PROTOCOL_SLUG),
        "sector:defi".to_owned(),
        "severity:high".to_owned(),
        "signal:admin_instruction".to_owned(),
        "source:realms".to_owned(),
        "source:spl_governance".to_owned(),
        "target:program".to_owned(),
        "watch:defi".to_owned(),
    ];
    tags.sort();
    tags.dedup();

    state
        .db
        .insert_event(&models::ChainEvent {
            id: BONK_DAO_PROPOSAL_EVENT_ID.to_owned(),
            signature: BONK_DAO_PROPOSAL_SIGNATURE.to_owned(),
            instruction_path: BONK_DAO_PROPOSAL_INSTRUCTION_PATH.to_owned(),
            kind: models::event_kind::PROGRAM_ADMIN_INSTRUCTION.to_owned(),
            target_type: models::target_type::PROGRAM.to_owned(),
            target_address: protocols::BONK_DAO_GOVERNANCE_PROGRAM_ID.to_owned(),
            actor: Some(BONK_DAO_PROPOSAL_ACTOR.to_owned()),
            slot: BONK_DAO_PROPOSAL_SLOT as i64,
            block_time: Some(BONK_DAO_PROPOSAL_BLOCK_TIME),
            indexed_at,
            summary: "High-impact Bonk DAO governance proposal created".to_owned(),
            tags,
            payload: json!({
                "classification": "admin",
                "severity": "high",
                "high_impact": true,
                "protocol": protocols::BONK_DAO_PROTOCOL_SLUG,
                "protocol_name": "Bonk DAO",
                "program": protocols::BONK_DAO_GOVERNANCE_PROGRAM_ID,
                "program_label": "SPL Governance",
                "instruction_name": "create_proposal",
                "discriminator": "06",
                "realm": protocols::BONK_DAO_REALM_ADDRESS,
                "proposal": BONK_DAO_PROPOSAL_ADDRESS,
                "governance": protocols::BONK_DAO_GOVERNANCE_ADDRESS,
                "token_owner_record": BONK_DAO_PROPOSAL_TOKEN_OWNER_RECORD,
                "governing_token_mint": protocols::BONK_DAO_COMMUNITY_MINT_ADDRESS,
                "source_protocol": protocols::REALMS_PROTOCOL_SLUG,
                "source_protocol_name": "Realms",
                "call_origin": "top_level",
                "stack_height": 1,
                "via_multisig": false,
                "parent_program": null,
                "immediate_parent_program": null,
                "recovery_source": "verified_bonkdao_incident",
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: indexed_at,
        })
        .await
}

async fn insert_known_bonk_dao_treasury_outflow_event(
    state: &context::SharedState,
) -> Result<bool> {
    let indexed_at = models::now_ts();
    let mut tags = vec![
        "alert:review_now".to_owned(),
        "impact:high".to_owned(),
        format!("mint:{}", protocols::BONK_DAO_COMMUNITY_MINT_ADDRESS),
        format!("protocol:{}", protocols::BONK_DAO_PROTOCOL_SLUG),
        "sector:defi".to_owned(),
        "severity:critical".to_owned(),
        "signal:treasury_outflow".to_owned(),
        "signal:wallet".to_owned(),
        "target:wallet".to_owned(),
        "token:bonk".to_owned(),
        "wallet:activity".to_owned(),
        "wallet:movement".to_owned(),
        "watch:defi".to_owned(),
    ];
    tags.sort();
    tags.dedup();

    state
        .db
        .insert_event(&models::ChainEvent {
            id: BONK_DAO_INCIDENT_EVENT_ID.to_owned(),
            signature: BONK_DAO_INCIDENT_SIGNATURE.to_owned(),
            instruction_path: BONK_DAO_INCIDENT_INSTRUCTION_PATH.to_owned(),
            kind: models::event_kind::TREASURY_TOKEN_OUTFLOW.to_owned(),
            target_type: models::target_type::WALLET.to_owned(),
            target_address: protocols::BONK_DAO_TREASURY_WALLET.to_owned(),
            actor: Some(protocols::BONK_DAO_TREASURY_WALLET.to_owned()),
            slot: BONK_DAO_INCIDENT_SLOT as i64,
            block_time: Some(BONK_DAO_INCIDENT_BLOCK_TIME),
            indexed_at,
            summary: format!(
                "Critical Bonk DAO treasury outflow: {} {} sent",
                BONK_DAO_INCIDENT_DISPLAY_AMOUNT,
                protocols::BONK_TOKEN_SYMBOL
            ),
            tags,
            payload: json!({
                "classification": "treasury_token_outflow",
                "severity": "critical",
                "protocol": protocols::BONK_DAO_PROTOCOL_SLUG,
                "protocol_name": "Bonk DAO",
                "wallet": protocols::BONK_DAO_TREASURY_WALLET,
                "source_wallet": protocols::BONK_DAO_TREASURY_WALLET,
                "authority": protocols::BONK_DAO_TREASURY_WALLET,
                "source_token_account": BONK_DAO_INCIDENT_SOURCE_TOKEN_ACCOUNT,
                "destination_token_account": BONK_DAO_INCIDENT_DESTINATION_TOKEN_ACCOUNT,
                "token_symbol": protocols::BONK_TOKEN_SYMBOL,
                "token_mint": protocols::BONK_DAO_COMMUNITY_MINT_ADDRESS,
                "token_decimals": protocols::BONK_TOKEN_DECIMALS,
                "raw_amount": BONK_DAO_INCIDENT_RAW_AMOUNT,
                "display_amount": BONK_DAO_INCIDENT_DISPLAY_AMOUNT,
                "critical_threshold_raw_amount": protocols::BONK_DAO_CRITICAL_OUTFLOW_RAW_AMOUNT.to_string(),
                "critical_threshold_display_amount": "1000000000000",
                "recovery_source": "verified_bonkdao_incident",
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: indexed_at,
        })
        .await
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

fn spawn_liquidity_pool_snapshot_prune(state: context::SharedState) {
    let interval_seconds = state.config.liquidity_pool_snapshot_prune_interval_seconds;
    if interval_seconds == 0 {
        return;
    }

    tokio::spawn(async move {
        loop {
            match state
                .db
                .prune_liquidity_pool_snapshots(
                    state.config.liquidity_pool_snapshot_retention_days,
                    state.config.watched_liquidity_pool_snapshot_retention_days,
                )
                .await
            {
                Ok(deleted_count) if deleted_count > 0 => {
                    info!(
                        deleted_count,
                        default_retention_days =
                            state.config.liquidity_pool_snapshot_retention_days,
                        watched_retention_days =
                            state.config.watched_liquidity_pool_snapshot_retention_days,
                        "pruned old liquidity pool snapshots",
                    );
                }
                Ok(_) => {
                    debug!("no liquidity pool snapshots pruned");
                }
                Err(error) => {
                    warn!(error = ?error, "failed to prune liquidity pool snapshots");
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(interval_seconds)).await;
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

fn print_token_liquidity_report(report: &TokenLiquidityScanReport) {
    println!(
        "Scanned liquidity for {} ({})",
        report.token_symbol, report.token_mint
    );
    println!(
        "History since: {} ({})",
        report.since_ts,
        format_unix_ts(report.since_ts)
    );
    println!("Markets discovered: {}", report.pools.len());
    println!("Wallet holders checked: {}", report.wallet_checks.len());
    for holding in &report.wallet_checks {
        println!(
            "Holder: {}\t{} {}",
            holding.wallet_address,
            format_amount(holding.token_amount),
            report.token_symbol
        );
    }

    for pool in &report.pools {
        println!();
        println!(
            "{} {}\t{}/{}\t{}",
            pool.dex_id,
            pool.labels.join(","),
            pool.base_token.symbol,
            pool.quote_token.symbol,
            pool.pair_address
        );
        if let Some(url) = pool.pair_url.as_deref() {
            println!("URL: {url}");
        }
        println!(
            "Signatures: {} total, {} successful, {} fetched tx, {} snapshots, {} tx errors",
            pool.signature_count,
            pool.successful_signature_count,
            pool.fetched_transaction_count,
            pool.snapshot_count,
            pool.failed_transaction_count
        );
        if let Some(liquidity_usd) = pool.current_liquidity_usd {
            println!("Estimated liquidity: {}", format_usd(liquidity_usd));
        }
        if let Some(reserve) = pool.current_watched_reserve {
            let reserve_usd = pool
                .current_watched_reserve_usd
                .map(format_usd)
                .unwrap_or_else(|| "unknown USD".to_owned());
            println!(
                "Current {} reserve: {} {} ({})",
                report.token_symbol,
                format_amount(reserve),
                report.token_symbol,
                reserve_usd
            );
        }
        if let Some(snapshot) = pool.latest_snapshot.as_ref() {
            println!(
                "Latest snapshot: slot {} {} tx {}",
                snapshot.slot,
                snapshot
                    .block_time
                    .map(format_unix_ts)
                    .unwrap_or_else(|| "unknown time".to_owned()),
                snapshot.signature
            );
        }
        if let Some(snapshot) = pool.first_low_snapshot.as_ref() {
            let reserve_usd = snapshot
                .watched_reserve_usd
                .map(format_usd)
                .unwrap_or_else(|| "unknown USD".to_owned());
            println!(
                "First low snapshot: slot {} {} reserve {} {} ({})",
                snapshot.slot,
                snapshot
                    .block_time
                    .map(format_unix_ts)
                    .unwrap_or_else(|| "unknown time".to_owned()),
                format_amount(snapshot.watched_reserve),
                report.token_symbol,
                reserve_usd
            );
        } else {
            println!("First low snapshot: none observed in lookback");
        }
        if let Some(transition) = pool.low_transition.as_ref() {
            print_pool_liquidity_transition("Low threshold transition", report, transition);
        }
        if let Some(transition) = pool.max_drop_transition.as_ref() {
            print_pool_liquidity_transition("Largest reserve drop", report, transition);
        }
        if let Some(buy_pressure) = pool.buy_pressure.as_ref() {
            print_pool_buy_pressure(report, buy_pressure);
        }
        if let Some(snapshot) = pool.min_snapshot.as_ref() {
            let reserve_usd = snapshot
                .watched_reserve_usd
                .map(format_usd)
                .unwrap_or_else(|| "unknown USD".to_owned());
            println!(
                "Minimum observed reserve: {} {} ({}) at slot {}",
                format_amount(snapshot.watched_reserve),
                report.token_symbol,
                reserve_usd,
                snapshot.slot
            );
        }
    }

    if report.inserted_event_count > 0 || report.delivered_event_count > 0 {
        println!();
        println!("Inserted alert events: {}", report.inserted_event_count);
        println!(
            "Queued Telegram deliveries: {}",
            report.delivered_event_count
        );
    }
}

fn print_pool_liquidity_transition(
    label: &str,
    report: &TokenLiquidityScanReport,
    transition: &PoolLiquidityTransition,
) {
    let ratio = transition
        .reserve_delta_ratio
        .map(|value| format!(" ({:.2}%)", value * 100.0))
        .unwrap_or_default();
    println!(
        "{}: slot {} -> {} reserve {} -> {} {} (delta {}{}); tx {}",
        label,
        transition.previous.slot,
        transition.current.slot,
        format_amount(transition.previous.watched_reserve),
        format_amount(transition.current.watched_reserve),
        report.token_symbol,
        format_amount(transition.reserve_delta),
        ratio,
        transition.current.signature
    );
}

fn print_pool_buy_pressure(report: &TokenLiquidityScanReport, pressure: &PoolBuyPressureReport) {
    let average_price = pressure
        .average_counter_per_watched
        .map(|value| format!("{value:.8}"))
        .unwrap_or_else(|| "unknown".to_owned());
    println!(
        "Buy pressure: {} {} bought from pool across {} txs by {} buyer(s); counter in {}; avg counter/{} {}",
        format_amount(pressure.watched_token_out),
        report.token_symbol,
        pressure.trade_count,
        pressure.unique_buyer_count,
        format_amount(pressure.counter_token_in),
        report.token_symbol,
        average_price
    );
    for buyer in &pressure.top_buyers {
        println!(
            "  Buyer: {}\t{} {}\t{} counter\t{} txs",
            buyer.owner,
            format_amount(buyer.watched_token_received),
            report.token_symbol,
            format_amount(buyer.counter_token_paid),
            buyer.trade_count
        );
    }
}

fn format_unix_ts(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| "invalid timestamp".to_owned())
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
    println!(
        "Parsed liquidity pool observations: {}",
        report.parsed_liquidity_pool_observation_count()
    );
    println!(
        "Inserted liquidity pools: {}",
        report.inserted_liquidity_pool_count()
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
    println!(
        "Parsed liquidity pool observations: {}",
        report.parsed_liquidity_pool_observation_count
    );
    println!(
        "Inserted liquidity pools: {}",
        report.inserted_liquidity_pool_count
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

fn print_liquidity_pool_dimension_report(report: &LiquidityPoolDimensionReport) {
    println!("Dimensioned liquidity pool registry");
    println!("Sources checked: {}", report.sources.len());
    println!("Failed sources: {}", report.failed_source_count());
    println!("Total pools: {}", report.total_pool_count());
    println!(
        "Decoded RPC JSON read: {}",
        format_bytes(report.total_rpc_response_bytes() as f64)
    );

    for source in &report.sources {
        match (
            source.account_count,
            source.rpc_response_bytes,
            source.error.as_deref(),
        ) {
            (Some(count), Some(bytes), None) => println!(
                "{}\t{}\t{}\t{}\t{} pools\t{}\t{:.2}s",
                source.protocol_slug,
                source.account_type,
                source.label,
                source.program_id,
                count,
                format_bytes(bytes as f64),
                source.elapsed_ms as f64 / 1000.0
            ),
            (_, _, Some(error)) => println!(
                "{}\t{}\t{}\t{}\tERROR\t{}\t{:.2}s",
                source.protocol_slug,
                source.account_type,
                source.label,
                source.program_id,
                error,
                source.elapsed_ms as f64 / 1000.0
            ),
            _ => {}
        }
    }

    let total = report.total_pool_count() as f64;
    println!(
        "SQLite registry estimate at 1KB/row: {}",
        format_bytes(total * 1024.0)
    );
    println!(
        "SQLite registry estimate at 1.5KB/row: {}",
        format_bytes(total * 1536.0)
    );
    println!(
        "Current-state side table at 256B/row: {}",
        format_bytes(total * 256.0)
    );
    println!(
        "History at 250B/row, hourly snapshots: {}/day",
        format_bytes(total * 24.0 * 250.0)
    );
    println!(
        "History at 250B/row, 10-minute snapshots: {}/day",
        format_bytes(total * 144.0 * 250.0)
    );
}

fn print_liquidity_pool_seed_report(report: &LiquidityPoolSeedReport) {
    println!(
        "Seeded liquidity pools for {}",
        default_seed_mint_label(report.mint.as_deref())
    );
    println!("DEX seed decoders checked: {}", report.dex_programs_checked);
    println!("Decoded pools: {}", report.decoded_pool_count);
    println!("Inserted pools: {}", report.inserted_pool_count);
    println!("Updated pools: {}", report.updated_pool_count);
    if !report.unsupported_programs.is_empty() {
        println!(
            "Seed decoders pending: {}",
            report.unsupported_programs.join(", ")
        );
    }

    for pool in &report.pools {
        println!(
            "{}\t{}\t{}\t{} / {}\treserves {} / {}",
            pool.protocol_slug,
            pool.program_label,
            pool.pool_address,
            pool.token_x_mint,
            pool.token_y_mint,
            pool.reserve_x.as_deref().unwrap_or("unknown"),
            pool.reserve_y.as_deref().unwrap_or("unknown")
        );
    }
}

fn print_protocol_authority_sync_report(report: &ProtocolAuthoritySyncReport) {
    println!("Synced protocol upgrade authorities");
    println!("Tracked programs checked: {}", report.program_count);
    println!(
        "Upgradeable programs with authority: {}",
        report.upgradeable_program_count
    );
    println!(
        "Immutable / skipped programs: {}",
        report.immutable_program_count
    );
    println!(
        "Squads multisigs loaded: {} ({})",
        report.squads_multisig_count,
        if report.discovered_all_squads {
            "full RPC discovery"
        } else {
            "indexed activity"
        }
    );
    println!(
        "Vault indexes checked per multisig: {}",
        report.checked_vault_index_count
    );
    println!("Linked programs: {}", report.linked_program_count);
    println!("Protocol multisig links found: {}", report.link_count);
    println!("Protocol multisig links stored: {}", report.inserted_count);
}

fn print_protocol_governance_sync_report(report: &ProtocolGovernanceSyncReport) {
    println!("Synced protocol governance realms");
    println!(
        "Realms governance programs checked: {}",
        report.governance_program_count
    );
    println!("Realm accounts loaded: {}", report.realm_count);
    println!("Governance accounts loaded: {}", report.governance_count);
    println!("Matched realms: {}", report.matched_realm_count);
    println!("Protocol governance links found: {}", report.link_count);
    println!(
        "Protocol governance links stored: {}",
        report.inserted_count
    );
}

fn format_bytes(bytes: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
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
