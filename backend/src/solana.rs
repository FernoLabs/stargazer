use crate::{
    anchor_idl::{self, AnchorIdlInstructionKind, ParsedAnchorIdlInstruction},
    context::SharedState,
    database::{BlockTimeUpdateReport, apply_idl_version_payload, apply_program_version_payload},
    delivery,
    models::{
        ChainEvent, MetricAlertRule, ProgramDataWrite, ProgramIdlDataWrite,
        ProtocolMetricObservation, event_kind, now_ts, target_type,
    },
    name_service::{self, DomainInstructionEvent, DomainInstructionKind},
    program_bytes::{instruction_order_key, sha256_hex},
    program_metadata::{self, ParsedProgramMetadataInstruction, ProgramMetadataInstructionKind},
    protocol_metrics::{self, MetricDecoderInput},
    protocols::{self, ProtocolInstructionSignal, ProtocolSignalKind},
};
use anyhow::{Context, Result, anyhow};
use futures::{SinkExt, StreamExt, stream};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    sync::OnceLock,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::prelude::{
    CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocksMeta,
    SubscribeRequestFilterTransactions, SubscribeRequestPing, subscribe_update::UpdateOneof,
};

#[allow(deprecated)]
use solana_sdk::{
    bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable,
    loader_instruction::LoaderInstruction,
    loader_upgradeable_instruction::UpgradeableLoaderInstruction, loader_v4,
    loader_v4_instruction::LoaderV4Instruction, pubkey::Pubkey, signature::Signature,
    system_program,
};

const FILTER_LABEL: &str = "firewatch";
const BLOCK_META_FILTER_LABEL: &str = "firewatch-block-meta";
const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ASSOCIATED_TOKEN_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
const SPL_TOKEN_DECIMALS: usize = 6;
const KAMINO_LEND_SLUG: &str = "kamino_lend";
const ORCA_WHIRLPOOLS_SLUG: &str = "orca_whirlpools";
const BLOCK_TIME_REPAIR_CONCURRENCY: usize = 8;
const ORCA_OFFICIAL_WHIRLPOOLS_CONFIGS: &[(&str, &str)] = &[
    (
        "2LecshUwdy9xi7meFgHtFJQNSKk4KdTrcpvaB56dP2NQ",
        "777H5H3Tp9U11uRVRzFwM8BinfiakbaLT8vQpeuhvEiH",
    ),
    (
        "FcrweFY1G9HJAHG5inkGB6pKg1HZ6x9UC2WioAfWrGkR",
        "475EJ7JqnRpVLoFVzp2ruEYvWWMCf6Z8KMWRujtXXNSU",
    ),
    (
        "FVG4oDbGv16hqTUbovjyGmtYikn6UBEnazz6RVDMEFwv",
        "9VrJciULifYcwu2CL8nbXdw4deqQgmv7VTzidwgQYBmm",
    ),
    (
        "FPydDjRdZu9sT7HVd6ANhfjh85KLq21Pefr5YWWMRPFp",
        "6gUEB962oFdZtwoVyXNya9TfGWnBEbYNYt8UdvzT6PSf",
    ),
];

#[derive(Clone, Debug)]
pub(crate) struct ResolvedInstruction {
    pub(crate) program_id_index: usize,
    pub(crate) accounts: Vec<usize>,
    pub(crate) data: Vec<u8>,
    pub(crate) stack_height: Option<u32>,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedInnerInstructionSet {
    pub(crate) index: usize,
    pub(crate) instructions: Vec<ResolvedInstruction>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockTimeRepairReport {
    pub(crate) checked_slots: usize,
    pub(crate) repaired_slots: usize,
    pub(crate) missing_slots: usize,
    pub(crate) failed_slots: usize,
    pub(crate) updated_rows: BlockTimeUpdateReport,
}

#[derive(Debug, Deserialize)]
struct RpcBlockTimeResponse {
    result: Option<i64>,
    error: Option<RpcBlockTimeError>,
}

#[derive(Debug, Deserialize)]
struct RpcBlockTimeError {
    code: i64,
    message: String,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedTransaction {
    pub(crate) signature: String,
    pub(crate) primary_signer: Option<Pubkey>,
    pub(crate) account_keys: Vec<Pubkey>,
    pub(crate) top_level_instructions: Vec<ResolvedInstruction>,
    pub(crate) inner_instruction_sets: Vec<ResolvedInnerInstructionSet>,
    pub(crate) log_messages: Vec<String>,
}

#[derive(Clone, Debug)]
struct ParentInstructionContext {
    parent_program: Option<Pubkey>,
    immediate_parent_program: Option<Pubkey>,
    squads_execution: Option<SquadsExecutionContext>,
}

#[derive(Clone, Debug)]
struct SquadsExecutionContext {
    multisig: String,
    proposal: Option<String>,
    transaction: Option<String>,
    member: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct ProtocolInstructionDetails {
    actor: Option<String>,
    high_impact_override: Option<bool>,
    payload_fields: Vec<(&'static str, String)>,
}

struct KaminoReserveConfigUpdate {
    mode: u64,
    mode_name: &'static str,
    value_len: usize,
    skip_validation: bool,
}

struct ParsedTransactionArtifacts {
    events: Vec<ChainEvent>,
    metric_observations: Vec<ProtocolMetricObservation>,
    program_data_writes: Vec<ProgramDataWrite>,
    program_idl_data_writes: Vec<ProgramIdlDataWrite>,
}

struct ParsedInstructionArtifacts {
    event: Option<ChainEvent>,
    metric_observations: Vec<ProtocolMetricObservation>,
    program_data_writes: Vec<ProgramDataWrite>,
    program_idl_data_writes: Vec<ProgramIdlDataWrite>,
}

impl ParsedInstructionArtifacts {
    fn event(event: Option<ChainEvent>) -> Self {
        Self {
            event,
            metric_observations: Vec::new(),
            program_data_writes: Vec::new(),
            program_idl_data_writes: Vec::new(),
        }
    }

    fn metrics(
        event: Option<ChainEvent>,
        metric_observations: Vec<ProtocolMetricObservation>,
    ) -> Self {
        Self {
            event,
            metric_observations,
            program_data_writes: Vec::new(),
            program_idl_data_writes: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct IngestOptions {
    pub(crate) enqueue_deliveries: bool,
    pub(crate) evaluate_metric_alerts: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct IngestTransactionResult {
    pub(crate) matched: bool,
    pub(crate) parsed_event_count: usize,
    pub(crate) inserted_event_count: usize,
    pub(crate) parsed_metric_observation_count: usize,
    pub(crate) inserted_metric_observation_count: usize,
    pub(crate) program_data_write_count: usize,
    pub(crate) program_idl_data_write_count: usize,
    pub(crate) program_version_update_count: usize,
    pub(crate) program_idl_version_update_count: usize,
}

#[derive(Clone, Debug)]
struct DomainProjectLink {
    protocol_slug: String,
    protocol_name: String,
    evidence: String,
}

#[derive(Clone, Debug, Default)]
struct DomainRiskContext {
    explicit_watch: bool,
    project_links: Vec<DomainProjectLink>,
}

pub(crate) async fn repair_recent_missing_block_times(
    state: SharedState,
    limit: i64,
) -> Result<BlockTimeRepairReport> {
    let slots = state
        .db
        .missing_block_time_slots(limit)
        .await
        .context("load slots with missing block times")?;
    let checked_slots = slots.len();
    let outcomes = stream::iter(slots)
        .map(|slot| {
            let state = state.clone();
            async move { (slot, repair_block_time_slot(state, slot).await) }
        })
        .buffer_unordered(BLOCK_TIME_REPAIR_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

    let mut report = BlockTimeRepairReport {
        checked_slots,
        ..Default::default()
    };

    for (slot, outcome) in outcomes {
        match outcome {
            Ok(Some(update)) => {
                if update.total() > 0 {
                    report.repaired_slots += 1;
                    report.updated_rows.merge(&update);
                }
            }
            Ok(None) => {
                report.missing_slots += 1;
            }
            Err(error) => {
                report.failed_slots += 1;
                warn!(slot, error = ?error, "failed to repair missing block time");
            }
        }
    }

    Ok(report)
}

pub(crate) async fn repair_block_time_slot(
    state: SharedState,
    slot: u64,
) -> Result<Option<BlockTimeUpdateReport>> {
    let Some(block_time) = fetch_rpc_block_time(&state, slot)
        .await
        .with_context(|| format!("fetch block time for slot {slot}"))?
    else {
        return Ok(None);
    };

    let report = state
        .db
        .update_block_time_for_slot(slot, block_time)
        .await
        .with_context(|| format!("update missing block times for slot {slot}"))?;
    Ok(Some(report))
}

async fn fetch_rpc_block_time(state: &SharedState, slot: u64) -> Result<Option<i64>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": slot,
        "method": "getBlockTime",
        "params": [slot]
    });
    let response = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&request)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .with_context(|| format!("fetch RPC block time for slot {slot}"))?
        .error_for_status()
        .with_context(|| format!("RPC getBlockTime endpoint returned an HTTP error for {slot}"))?
        .json::<RpcBlockTimeResponse>()
        .await
        .with_context(|| format!("decode RPC getBlockTime JSON for slot {slot}"))?;

    if let Some(error) = response.error {
        debug!(
            slot,
            code = error.code,
            message = %error.message,
            "RPC getBlockTime returned no timestamp",
        );
        return Ok(None);
    }

    Ok(response.result)
}

pub async fn run_indexer(state: SharedState) -> Result<()> {
    let mut slot_times = BTreeMap::<u64, i64>::new();

    loop {
        let resume_from_slot = state.db.last_processed_slot().await?;
        let mut retry_immediately = false;
        match connect_and_stream(state.clone(), resume_from_slot, &mut slot_times).await {
            Ok(()) => warn!("yellowstone stream ended unexpectedly"),
            Err(error) => {
                if let Some(last_available_slot) = extract_last_available_slot(&error) {
                    warn!(
                        resume_from_slot,
                        last_available_slot,
                        "yellowstone resume slot is outside the provider replay window, switching to live tail",
                    );
                    state.db.clear_last_processed_slot().await?;
                    retry_immediately = true;
                } else {
                    warn!(error = ?error, "yellowstone stream disconnected");
                }
            }
        }

        if retry_immediately {
            continue;
        }

        sleep(Duration::from_secs(state.config.reconnect_delay_seconds)).await;
    }
}

async fn connect_and_stream(
    state: SharedState,
    resume_from_slot: Option<u64>,
    slot_times: &mut BTreeMap<u64, i64>,
) -> Result<()> {
    let mut builder =
        GeyserGrpcClient::build_from_shared(state.config.solana_grpc_endpoint.clone())?
            .x_token(state.config.solana_grpc_x_token.clone())?;

    if state.config.solana_grpc_endpoint.starts_with("https://") {
        builder = builder.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }

    let mut client = builder.connect().await?;
    let (mut subscribe_tx, mut stream) = client.subscribe().await?;

    subscribe_tx
        .send(subscription_request(&state, resume_from_slot)?)
        .await?;
    info!(resume_from_slot, "connected to yellowstone gRPC");

    while let Some(message) = stream.next().await {
        let update = message?;

        match update.update_oneof {
            Some(UpdateOneof::Transaction(tx_update)) => {
                let slot = tx_update.slot;
                let block_time = slot_times.get(&slot).copied();
                let tx_info = tx_update
                    .transaction
                    .ok_or_else(|| anyhow!("transaction update missing payload"))?;

                let resolved = decode_transaction(tx_info)?;
                state
                    .db
                    .increment_processed_transaction_count()
                    .await
                    .context("increment processed transaction count")?;
                let ingest_result = ingest_resolved_transaction(
                    state.clone(),
                    &resolved,
                    slot,
                    block_time,
                    IngestOptions {
                        enqueue_deliveries: true,
                        evaluate_metric_alerts: true,
                    },
                )
                .await?;

                if ingest_result.parsed_event_count > 0 {
                    info!(
                        slot,
                        signature = %resolved.signature,
                        event_count = ingest_result.parsed_event_count,
                        primary_signer = resolved.primary_signer.map(|value| value.to_string()),
                        "parsed matching events from transaction",
                    );
                }

                if ingest_result.matched {
                    state
                        .db
                        .increment_matched_transaction_count()
                        .await
                        .context("increment matched transaction count")?;
                }

                state
                    .db
                    .set_last_processed_slot(slot)
                    .await
                    .context("set last processed transaction slot")?;
            }
            Some(UpdateOneof::BlockMeta(block_meta)) => {
                if let Some(block_time) = block_meta.block_time.map(|value| value.timestamp) {
                    slot_times.insert(block_meta.slot, block_time);
                    trim_slot_times(slot_times);
                    match state
                        .db
                        .update_block_time_for_slot(block_meta.slot, block_time)
                        .await
                    {
                        Ok(report) if report.total() > 0 => {
                            info!(
                                slot = block_meta.slot,
                                block_time,
                                updated_rows = report.total(),
                                "filled missing block time from yellowstone block meta",
                            );
                        }
                        Ok(_) => {}
                        Err(error) => {
                            warn!(
                                slot = block_meta.slot,
                                block_time,
                                error = ?error,
                                "failed to fill missing block time from yellowstone block meta",
                            );
                        }
                    }
                }

                state
                    .db
                    .set_last_processed_slot(block_meta.slot)
                    .await
                    .context("set last processed block meta slot")?;
            }
            Some(UpdateOneof::Ping(_)) => {
                subscribe_tx
                    .send(SubscribeRequest {
                        ping: Some(SubscribeRequestPing { id: 1 }),
                        ..Default::default()
                    })
                    .await?;
            }
            Some(UpdateOneof::Pong(_)) => {}
            Some(_) => {}
            None => error!("yellowstone update without payload"),
        }
    }

    Ok(())
}

fn subscription_request(
    state: &SharedState,
    resume_from_slot: Option<u64>,
) -> Result<SubscribeRequest> {
    let mut watched_programs = vec![
        bpf_loader::ID.to_string(),
        bpf_loader_deprecated::ID.to_string(),
        bpf_loader_upgradeable::ID.to_string(),
        loader_v4::ID.to_string(),
        name_service::program_id().to_string(),
        squads_program_id().to_string(),
    ];
    watched_programs.extend(
        protocols::watched_program_ids()
            .into_iter()
            .map(|program_id| program_id.to_string()),
    );
    watched_programs.extend(
        program_metadata::canonical_idl_metadata_addresses(&protocols::watched_program_ids())
            .into_iter()
            .map(|metadata_account| metadata_account.to_string()),
    );
    watched_programs.extend(
        anchor_idl::legacy_idl_addresses(&protocols::watched_program_ids())
            .into_iter()
            .map(|metadata_account| metadata_account.to_string()),
    );
    if let Some(recipient) = state.config.solana_pay_recipient.as_deref() {
        if Pubkey::from_str(recipient).is_ok() {
            watched_programs.push(recipient.to_owned());
        }
    }
    watched_programs.sort();
    watched_programs.dedup();

    Ok(SubscribeRequest {
        transactions: HashMap::from([(
            FILTER_LABEL.to_owned(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include: watched_programs,
                account_exclude: vec![],
                account_required: vec![],
            },
        )]),
        blocks_meta: HashMap::from([(
            BLOCK_META_FILTER_LABEL.to_owned(),
            SubscribeRequestFilterBlocksMeta {},
        )]),
        commitment: Some(CommitmentLevel::Finalized as i32),
        from_slot: resume_from_slot,
        ..Default::default()
    })
}

fn extract_last_available_slot(error: &anyhow::Error) -> Option<u64> {
    let message = error.to_string();
    let needle = "last available:";
    let (_, suffix) = message.split_once(needle)?;
    let digits = suffix
        .trim()
        .chars()
        .skip_while(|char| !char.is_ascii_digit())
        .take_while(|char| char.is_ascii_digit())
        .collect::<String>();

    if digits.is_empty() {
        None
    } else {
        digits.parse::<u64>().ok()
    }
}

pub(crate) fn decode_transaction(
    tx_info: yellowstone_grpc_proto::prelude::SubscribeUpdateTransactionInfo,
) -> Result<ResolvedTransaction> {
    let transaction = tx_info
        .transaction
        .ok_or_else(|| anyhow!("missing transaction body"))?;
    let message = transaction
        .message
        .ok_or_else(|| anyhow!("missing transaction message"))?;
    let header = message
        .header
        .ok_or_else(|| anyhow!("missing transaction header"))?;
    let meta = tx_info
        .meta
        .ok_or_else(|| anyhow!("missing transaction meta"))?;

    let static_account_keys = message
        .account_keys
        .iter()
        .map(|bytes| parse_pubkey(bytes))
        .collect::<Result<Vec<_>>>()?;

    let signer_count = header.num_required_signatures as usize;
    let primary_signer = static_account_keys.get(0).copied();

    let mut account_keys = static_account_keys;
    account_keys.extend(
        meta.loaded_writable_addresses
            .iter()
            .map(|bytes| parse_pubkey(bytes))
            .collect::<Result<Vec<_>>>()?,
    );
    account_keys.extend(
        meta.loaded_readonly_addresses
            .iter()
            .map(|bytes| parse_pubkey(bytes))
            .collect::<Result<Vec<_>>>()?,
    );

    if signer_count > account_keys.len() {
        return Err(anyhow!(
            "transaction declared more signers than account keys"
        ));
    }

    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| anyhow!("transaction missing signature"))?;
    let signature = Signature::try_from(signature.as_slice())
        .context("invalid transaction signature")?
        .to_string();

    let top_level_instructions = message
        .instructions
        .into_iter()
        .map(|instruction| ResolvedInstruction {
            program_id_index: instruction.program_id_index as usize,
            accounts: instruction.accounts.into_iter().map(usize::from).collect(),
            data: instruction.data,
            stack_height: Some(1),
        })
        .collect();

    let inner_instruction_sets = meta
        .inner_instructions
        .into_iter()
        .map(|set| ResolvedInnerInstructionSet {
            index: set.index as usize,
            instructions: set
                .instructions
                .into_iter()
                .map(|instruction| ResolvedInstruction {
                    program_id_index: instruction.program_id_index as usize,
                    accounts: instruction.accounts.into_iter().map(usize::from).collect(),
                    data: instruction.data,
                    stack_height: instruction.stack_height,
                })
                .collect(),
        })
        .collect();

    Ok(ResolvedTransaction {
        signature,
        primary_signer,
        account_keys,
        top_level_instructions,
        inner_instruction_sets,
        log_messages: meta.log_messages,
    })
}

pub(crate) async fn ingest_resolved_transaction(
    state: SharedState,
    tx: &ResolvedTransaction,
    slot: u64,
    block_time: Option<i64>,
    options: IngestOptions,
) -> Result<IngestTransactionResult> {
    let parsed = parse_transaction_events(state.clone(), tx, slot, block_time).await?;
    let detected_payment_count = detect_solana_payments(state.clone(), tx, block_time)
        .await
        .context("detect Solana Pay payment")?;
    let mut events = parsed.events;
    let mut result = IngestTransactionResult {
        matched: !events.is_empty()
            || !parsed.metric_observations.is_empty()
            || !parsed.program_data_writes.is_empty()
            || !parsed.program_idl_data_writes.is_empty()
            || detected_payment_count > 0,
        parsed_event_count: events.len(),
        parsed_metric_observation_count: parsed.metric_observations.len(),
        program_data_write_count: parsed.program_data_writes.len(),
        program_idl_data_write_count: parsed.program_idl_data_writes.len(),
        ..Default::default()
    };

    for write in parsed.program_data_writes {
        if state
            .db
            .insert_program_data_write(&write)
            .await
            .context("insert program data write")?
        {
            result.program_version_update_count += state
                .db
                .upsert_program_versions_for_write_account(&write.write_account)
                .await
                .context("refresh program versions for program data write")?;
        }
    }

    for write in parsed.program_idl_data_writes {
        if state
            .db
            .insert_program_idl_data_write(&write)
            .await
            .context("insert program IDL data write")?
        {
            result.program_idl_version_update_count += state
                .db
                .upsert_program_idl_versions_for_source_account(&write.source_account)
                .await
                .context("refresh program IDL versions for IDL data write")?;
            let direct_version = state
                .db
                .upsert_anchor_legacy_direct_idl_version_for_write(&write)
                .await
                .context("refresh direct legacy Anchor IDL version")?;
            if direct_version.is_some()
                || state
                    .db
                    .upsert_latest_anchor_legacy_direct_idl_version_for_source_account(
                        &write.source_account,
                    )
                    .await
                    .context("refresh latest direct legacy Anchor IDL version")?
                    .is_some()
            {
                result.program_idl_version_update_count += 1;
            }
        }
    }

    for observation in parsed.metric_observations {
        let metric_alerts = if options.evaluate_metric_alerts {
            Some((
                state
                    .db
                    .previous_protocol_metric_observation(&observation)
                    .await
                    .context("load previous protocol metric observation")?,
                state
                    .db
                    .matching_metric_alert_rules(&observation)
                    .await
                    .context("load matching metric alert rules")?,
            ))
        } else {
            None
        };

        if state
            .db
            .insert_protocol_metric_observation(&observation)
            .await
            .context("insert protocol metric observation")?
        {
            result.inserted_metric_observation_count += 1;

            if let Some((previous_observation, matching_rules)) = metric_alerts {
                let metric_alert_events = build_metric_alert_events(
                    tx,
                    &observation,
                    previous_observation.as_ref(),
                    &matching_rules,
                );
                result.parsed_event_count += metric_alert_events.len();
                events.extend(metric_alert_events);
            }
        }
    }

    for event in events {
        let mut event = event;
        if let Some(summary) = state
            .db
            .upsert_program_version_for_event(&event)
            .await
            .context("upsert program version for event")?
        {
            apply_program_version_payload(&mut event.payload, &summary)
                .context("annotate program event with version bytes")?;
            result.program_version_update_count += 1;
        }
        if let Some(idl_version) = state
            .db
            .upsert_program_idl_version_for_event(&event)
            .await
            .context("upsert program IDL version for event")?
        {
            apply_idl_version_payload(&mut event.payload, &idl_version)
                .context("annotate program event with IDL bytes")?;
            result.program_idl_version_update_count += 1;
        }
        annotate_multisig_signer_patterns(&state, &mut event)
            .await
            .context("annotate multisig signer patterns")?;
        annotate_multisig_project_links(&state, &mut event)
            .await
            .context("annotate multisig project links")?;
        event = name_service::hydrate_event(&state, event).await;
        if state
            .db
            .insert_event(&event)
            .await
            .context("insert chain event")?
        {
            result.inserted_event_count += 1;
            info!(
                slot = event.slot,
                signature = %event.signature,
                kind = %event.kind,
                target_type = %event.target_type,
                target_address = %event.target_address,
                actor = event.actor.as_deref().unwrap_or("unknown"),
                instruction_path = %event.instruction_path,
                "indexed chain event",
            );
            if options.enqueue_deliveries {
                delivery::enqueue_telegram_deliveries(state.clone(), &event).await?;
            }
        }
    }

    Ok(result)
}

async fn parse_transaction_events(
    state: SharedState,
    tx: &ResolvedTransaction,
    slot: u64,
    block_time: Option<i64>,
) -> Result<ParsedTransactionArtifacts> {
    let mut events = Vec::new();
    let mut metric_observations = Vec::new();
    let mut program_data_writes = Vec::new();
    let mut program_idl_data_writes = Vec::new();

    for (index, instruction) in tx.top_level_instructions.iter().enumerate() {
        let mut top_level_artifacts = parse_instruction(
            state.clone(),
            tx,
            instruction,
            slot,
            block_time,
            &index.to_string(),
            None,
        )
        .await?;
        metric_observations.extend(top_level_artifacts.metric_observations);
        program_data_writes.extend(top_level_artifacts.program_data_writes);
        program_idl_data_writes.extend(top_level_artifacts.program_idl_data_writes);
        let mut inner_events = Vec::new();

        if let Some(inner_set) = tx
            .inner_instruction_sets
            .iter()
            .find(|set| set.index == index)
        {
            let inner_contexts = inner_parent_contexts_for_instruction_set(
                tx,
                instruction,
                &inner_set.instructions,
            )?;
            for (inner_index, inner_instruction) in inner_set.instructions.iter().enumerate() {
                let path = format!("{index}.{inner_index}");
                let artifacts = parse_instruction(
                    state.clone(),
                    tx,
                    inner_instruction,
                    slot,
                    block_time,
                    &path,
                    inner_contexts.get(inner_index),
                )
                .await?;
                metric_observations.extend(artifacts.metric_observations);
                program_data_writes.extend(artifacts.program_data_writes);
                program_idl_data_writes.extend(artifacts.program_idl_data_writes);
                if let Some(event) = artifacts.event {
                    inner_events.push(event);
                }
            }
        }

        if let Some(event) = top_level_artifacts.event.as_mut() {
            enrich_multisig_event(event, &inner_events);
        }

        if let Some(event) = top_level_artifacts.event {
            events.push(event);
        }

        events.extend(inner_events);
    }

    Ok(ParsedTransactionArtifacts {
        events,
        metric_observations,
        program_data_writes,
        program_idl_data_writes,
    })
}

async fn detect_solana_payments(
    state: SharedState,
    tx: &ResolvedTransaction,
    block_time: Option<i64>,
) -> Result<usize> {
    let Some(recipient) = state
        .config
        .solana_pay_recipient
        .as_deref()
        .and_then(|value| Pubkey::from_str(value).ok())
    else {
        return Ok(0);
    };

    let references = tx
        .account_keys
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let now = now_ts();
    let pending_links = state
        .db
        .pending_payment_links_by_references(&references, now)
        .await?;
    if pending_links.is_empty() {
        return Ok(0);
    }

    let paid_at = block_time.unwrap_or(now);
    let mut detected = 0;

    for payment_link in pending_links {
        let payment_matches = if let Some(spl_token) = payment_link.spl_token.as_deref() {
            let Some(mint) = Pubkey::from_str(spl_token).ok() else {
                continue;
            };
            let Some(expected_amount) =
                decimal_amount_to_base_units(&payment_link.amount, SPL_TOKEN_DECIMALS)
            else {
                continue;
            };
            spl_token_amount_transferred(tx, &mint, &recipient) >= expected_amount
        } else {
            if !tx.account_keys.iter().any(|account| account == &recipient) {
                continue;
            }
            let Some(expected_lamports) = sol_amount_to_lamports(&payment_link.amount) else {
                continue;
            };
            sol_transferred_to_recipient(tx, &recipient) >= expected_lamports
        };

        if !payment_matches {
            continue;
        }

        if state
            .db
            .mark_payment_link_paid(&payment_link.id, paid_at)
            .await?
        {
            state
                .db
                .grant_user_entitlement(
                    &payment_link.user_id,
                    &payment_link.plan_id,
                    "solana_pay",
                    Some(crate::models::payment_plan_duration_months(
                        &payment_link.plan_id,
                    )),
                    None,
                )
                .await?;
            detected += 1;
            info!(
                signature = %tx.signature,
                reference = %payment_link.reference,
                user_id = %payment_link.user_id,
                amount = %payment_link.amount,
                spl_token = ?payment_link.spl_token,
                "confirmed Solana Pay payment",
            );
        }
    }

    Ok(detected)
}

fn spl_token_amount_transferred(
    tx: &ResolvedTransaction,
    mint: &Pubkey,
    recipient: &Pubkey,
) -> u64 {
    let Some(token_program_id) = Pubkey::from_str(TOKEN_PROGRAM_ID).ok() else {
        return 0;
    };
    let Some(recipient_token_account) = associated_token_account(recipient, mint) else {
        return 0;
    };
    let mut amount = 0_u64;
    for instruction in tx.top_level_instructions.iter().chain(
        tx.inner_instruction_sets
            .iter()
            .flat_map(|set| set.instructions.iter()),
    ) {
        if program_id(tx, instruction) != Some(token_program_id) {
            continue;
        }

        if token_transfer_checked_mint(tx, instruction) != Some(*mint) {
            continue;
        }

        if token_transfer_checked_destination(tx, instruction) != Some(recipient_token_account) {
            continue;
        }

        if let Some(value) = token_transfer_checked_amount(&instruction.data) {
            amount = amount.saturating_add(value);
        }
    }

    amount
}

fn token_transfer_checked_mint(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
) -> Option<Pubkey> {
    if instruction.data.first().copied()? != 12 {
        return None;
    }

    instruction_account(tx, instruction, 1).ok()
}

fn token_transfer_checked_destination(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
) -> Option<Pubkey> {
    if instruction.data.first().copied()? != 12 {
        return None;
    }

    instruction_account(tx, instruction, 2).ok()
}

fn token_transfer_checked_amount(data: &[u8]) -> Option<u64> {
    if data.first().copied()? != 12 {
        return None;
    }

    let amount = data.get(1..9)?.try_into().ok()?;
    Some(u64::from_le_bytes(amount))
}

fn associated_token_account(owner: &Pubkey, mint: &Pubkey) -> Option<Pubkey> {
    let token_program_id = Pubkey::from_str(TOKEN_PROGRAM_ID).ok()?;
    let associated_token_program_id = Pubkey::from_str(ASSOCIATED_TOKEN_PROGRAM_ID).ok()?;
    Some(
        Pubkey::find_program_address(
            &[owner.as_ref(), token_program_id.as_ref(), mint.as_ref()],
            &associated_token_program_id,
        )
        .0,
    )
}

fn sol_transferred_to_recipient(tx: &ResolvedTransaction, recipient: &Pubkey) -> u64 {
    let mut lamports = 0_u64;
    for instruction in tx.top_level_instructions.iter().chain(
        tx.inner_instruction_sets
            .iter()
            .flat_map(|set| set.instructions.iter()),
    ) {
        if program_id(tx, instruction) != Some(system_program::ID) {
            continue;
        }

        let Ok(destination) = instruction_account(tx, instruction, 1) else {
            continue;
        };
        if &destination != recipient {
            continue;
        }

        if let Some(value) = system_transfer_lamports(&instruction.data) {
            lamports = lamports.saturating_add(value);
        }
    }

    lamports
}

fn system_transfer_lamports(data: &[u8]) -> Option<u64> {
    let discriminator = data.get(0..4)?;
    if discriminator != [2, 0, 0, 0] {
        return None;
    }

    let lamports = data.get(4..12)?.try_into().ok()?;
    Some(u64::from_le_bytes(lamports))
}

fn sol_amount_to_lamports(amount: &str) -> Option<u64> {
    decimal_amount_to_base_units(amount, 9)
}

fn decimal_amount_to_base_units(amount: &str, decimals: usize) -> Option<u64> {
    let amount = amount.trim();
    if amount.is_empty() || amount.starts_with('-') {
        return None;
    }

    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    if whole.is_empty() || !whole.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    if !fraction.chars().all(|character| character.is_ascii_digit()) || fraction.len() > decimals {
        return None;
    }

    let unit = 10_u64.checked_pow(decimals as u32)?;
    let whole_units = whole.parse::<u64>().ok()?.checked_mul(unit)?;
    let mut fractional = fraction.to_owned();
    while fractional.len() < decimals {
        fractional.push('0');
    }
    let fractional_units = if fractional.is_empty() {
        0
    } else {
        fractional.parse::<u64>().ok()?
    };

    whole_units.checked_add(fractional_units)
}

async fn parse_instruction(
    state: SharedState,
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<ParsedInstructionArtifacts> {
    let Some(program_id) = program_id(tx, instruction) else {
        return Ok(ParsedInstructionArtifacts::event(None));
    };

    if program_id == bpf_loader_upgradeable::ID {
        let (event, write) = parse_upgradeable_loader_artifacts(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            parent_context,
        )?;
        return Ok(ParsedInstructionArtifacts {
            event,
            metric_observations: Vec::new(),
            program_data_writes: write.into_iter().collect(),
            program_idl_data_writes: Vec::new(),
        });
    }

    if program_id == loader_v4::ID {
        let (event, write) = parse_loader_v4_artifacts(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            parent_context,
        )?;
        return Ok(ParsedInstructionArtifacts {
            event,
            metric_observations: Vec::new(),
            program_data_writes: write.into_iter().collect(),
            program_idl_data_writes: Vec::new(),
        });
    }

    if program_id == bpf_loader::ID || program_id == bpf_loader_deprecated::ID {
        let (event, write) = parse_legacy_loader_artifacts(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            program_id,
        )?;
        return Ok(ParsedInstructionArtifacts {
            event,
            metric_observations: Vec::new(),
            program_data_writes: write.into_iter().collect(),
            program_idl_data_writes: Vec::new(),
        });
    }

    if program_id == squads_program_id() {
        return Ok(ParsedInstructionArtifacts::event(parse_squads_event(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
        )?));
    }

    if program_id == name_service::program_id() {
        return Ok(ParsedInstructionArtifacts::event(
            parse_name_service_event(&state, tx, instruction, slot, block_time, instruction_path)
                .await?,
        ));
    }

    if program_id == program_metadata::program_id() {
        let (event, write) = parse_program_metadata_artifacts(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            parent_context,
        )?;
        return Ok(ParsedInstructionArtifacts {
            event,
            metric_observations: Vec::new(),
            program_data_writes: Vec::new(),
            program_idl_data_writes: write.into_iter().collect(),
        });
    }

    if protocols::protocol_for_program(program_id).is_some() {
        let (idl_event, idl_write) = parse_anchor_legacy_idl_artifacts(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            program_id,
            parent_context,
        )?;
        if idl_event.is_some() || idl_write.is_some() {
            return Ok(ParsedInstructionArtifacts {
                event: idl_event,
                metric_observations: Vec::new(),
                program_data_writes: Vec::new(),
                program_idl_data_writes: idl_write.into_iter().collect(),
            });
        }

        let (event, metric_observations) = parse_protocol_instruction_event(
            tx,
            instruction,
            slot,
            block_time,
            instruction_path,
            parent_context,
        )?;
        return Ok(ParsedInstructionArtifacts::metrics(
            event,
            metric_observations,
        ));
    }

    Ok(ParsedInstructionArtifacts::event(None))
}

fn parse_upgradeable_loader_artifacts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<(Option<ChainEvent>, Option<ProgramDataWrite>)> {
    let parsed: UpgradeableLoaderInstruction = match bincode::deserialize(&instruction.data) {
        Ok(parsed) => parsed,
        Err(_) => return Ok((None, None)),
    };

    let parent_program = parent_context
        .and_then(|context| context.parent_program)
        .map(|value| value.to_string());

    let event = match parsed {
        UpgradeableLoaderInstruction::Write { offset, bytes } => {
            return Ok((
                None,
                Some(build_program_data_write(
                    tx,
                    slot,
                    block_time,
                    instruction_path,
                    "upgradeable".to_owned(),
                    instruction_account(tx, instruction, 0)?,
                    instruction_account(tx, instruction, 1).ok(),
                    offset,
                    bytes,
                )?),
            ));
        }
        UpgradeableLoaderInstruction::DeployWithMaxDataLen { max_data_len } => {
            let target_program = instruction_account(tx, instruction, 2)?;
            let authority = instruction_account(tx, instruction, 7)?;
            Some(build_event(
                tx,
                event_kind::PROGRAM_DEPLOYED,
                target_type::PROGRAM,
                target_program.to_string(),
                Some(authority.to_string()),
                slot,
                block_time,
                instruction_path,
                summary_for_program("Program deployed", &target_program, parent_context),
                program_update_tags(&target_program, parent_context),
                json!({
                    "loader": "upgradeable",
                    "program": target_program.to_string(),
                    "program_data_account": instruction_account(tx, instruction, 1)?.to_string(),
                    "buffer_account": instruction_account(tx, instruction, 3)?.to_string(),
                    "authority": authority.to_string(),
                    "max_data_len": max_data_len,
                    "parent_program": parent_program,
                }),
            ))
        }
        UpgradeableLoaderInstruction::Upgrade => {
            let target_program = instruction_account(tx, instruction, 1)?;
            let authority = instruction_account(tx, instruction, 6)?;
            Some(build_event(
                tx,
                event_kind::PROGRAM_UPGRADED,
                target_type::PROGRAM,
                target_program.to_string(),
                Some(authority.to_string()),
                slot,
                block_time,
                instruction_path,
                summary_for_program("Program upgraded", &target_program, parent_context),
                program_update_tags(&target_program, parent_context),
                json!({
                    "loader": "upgradeable",
                    "program": target_program.to_string(),
                    "program_data_account": instruction_account(tx, instruction, 0)?.to_string(),
                    "buffer_account": instruction_account(tx, instruction, 2)?.to_string(),
                    "authority": authority.to_string(),
                    "spill_account": instruction_account(tx, instruction, 3)?.to_string(),
                    "parent_program": parent_program,
                }),
            ))
        }
        UpgradeableLoaderInstruction::Migrate => {
            let target_program = instruction_account(tx, instruction, 1)?;
            let authority = instruction_account(tx, instruction, 2)?;
            Some(build_event(
                tx,
                event_kind::PROGRAM_MIGRATED_TO_LOADER_V4,
                target_type::PROGRAM,
                target_program.to_string(),
                Some(authority.to_string()),
                slot,
                block_time,
                instruction_path,
                summary_for_program(
                    "Program migrated to loader v4",
                    &target_program,
                    parent_context,
                ),
                program_update_tags(&target_program, parent_context),
                json!({
                    "loader": "upgradeable",
                    "program": target_program.to_string(),
                    "program_data_account": instruction_account(tx, instruction, 0)?.to_string(),
                    "authority": authority.to_string(),
                    "parent_program": parent_program,
                }),
            ))
        }
        _ => None,
    };

    Ok((event, None))
}

fn parse_loader_v4_artifacts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<(Option<ChainEvent>, Option<ProgramDataWrite>)> {
    let parsed: LoaderV4Instruction = match bincode::deserialize(&instruction.data) {
        Ok(parsed) => parsed,
        Err(_) => return Ok((None, None)),
    };

    let parent_program = parent_context
        .and_then(|context| context.parent_program)
        .map(|value| value.to_string());
    let target_program = instruction_account(tx, instruction, 0)?;
    let authority = instruction_account(tx, instruction, 1).ok();

    let event = match parsed {
        LoaderV4Instruction::Write { offset, bytes } => {
            return Ok((
                None,
                Some(build_program_data_write(
                    tx,
                    slot,
                    block_time,
                    instruction_path,
                    "v4".to_owned(),
                    target_program,
                    authority,
                    offset,
                    bytes,
                )?),
            ));
        }
        LoaderV4Instruction::Deploy => Some(build_event(
            tx,
            event_kind::PROGRAM_DEPLOYED,
            target_type::PROGRAM,
            target_program.to_string(),
            authority.map(|value| value.to_string()),
            slot,
            block_time,
            instruction_path,
            summary_for_program("Loader v4 deploy", &target_program, parent_context),
            program_update_tags(&target_program, parent_context),
            json!({
                "loader": "v4",
                "program": target_program.to_string(),
                "authority": authority.map(|value| value.to_string()),
                "source_program": instruction_account(tx, instruction, 2).ok().map(|value| value.to_string()),
                "parent_program": parent_program,
            }),
        )),
        LoaderV4Instruction::Finalize => Some(build_event(
            tx,
            event_kind::PROGRAM_FINALIZED,
            target_type::PROGRAM,
            target_program.to_string(),
            authority.map(|value| value.to_string()),
            slot,
            block_time,
            instruction_path,
            summary_for_program("Loader v4 finalize", &target_program, parent_context),
            program_update_tags(&target_program, parent_context),
            json!({
                "loader": "v4",
                "program": target_program.to_string(),
                "authority": authority.map(|value| value.to_string()),
                "next_program": instruction_account(tx, instruction, 2).ok().map(|value| value.to_string()),
                "parent_program": parent_program,
            }),
        )),
        _ => None,
    };

    Ok((event, None))
}

fn parse_legacy_loader_artifacts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    loader_program: Pubkey,
) -> Result<(Option<ChainEvent>, Option<ProgramDataWrite>)> {
    let parsed: LoaderInstruction = match bincode::deserialize(&instruction.data) {
        Ok(parsed) => parsed,
        Err(_) => return Ok((None, None)),
    };

    let event = match parsed {
        LoaderInstruction::Write { offset, bytes } => {
            return Ok((
                None,
                Some(build_program_data_write(
                    tx,
                    slot,
                    block_time,
                    instruction_path,
                    loader_program.to_string(),
                    instruction_account(tx, instruction, 0)?,
                    instruction_account(tx, instruction, 0).ok(),
                    offset,
                    bytes,
                )?),
            ));
        }
        LoaderInstruction::Finalize => {
            let target_program = instruction_account(tx, instruction, 0)?;
            Some(build_event(
                tx,
                event_kind::PROGRAM_DEPLOYED,
                target_type::PROGRAM,
                target_program.to_string(),
                tx.primary_signer.map(|value| value.to_string()),
                slot,
                block_time,
                instruction_path,
                format!("Legacy BPF loader finalized {target_program}"),
                program_update_tags(&target_program, None),
                json!({
                    "loader": loader_program.to_string(),
                    "program": target_program.to_string(),
                    "actor": tx.primary_signer.map(|value| value.to_string()),
                }),
            ))
        }
    };

    Ok((event, None))
}

fn parse_squads_event(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
) -> Result<Option<ChainEvent>> {
    let discriminator = instruction
        .data
        .get(..8)
        .ok_or_else(|| anyhow!("squads instruction missing discriminator"))?;

    let (kind, summary, actor, mut payload) =
        if discriminator == anchor_discriminator("proposal_activate").as_slice() {
            let multisig = instruction_account(tx, instruction, 0)?;
            let member = instruction_account(tx, instruction, 1)?;
            let proposal = instruction_account(tx, instruction, 2)?;
            (
                event_kind::MULTISIG_PROPOSAL_ACTIVATED,
                format!("Squads proposal activated on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                }),
            )
        } else if discriminator == anchor_discriminator("proposal_approve").as_slice() {
            let multisig = instruction_account(tx, instruction, 0)?;
            let member = instruction_account(tx, instruction, 1)?;
            let proposal = instruction_account(tx, instruction, 2)?;
            (
                event_kind::MULTISIG_PROPOSAL_APPROVED,
                format!("Squads approval on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                }),
            )
        } else if discriminator == anchor_discriminator("proposal_reject").as_slice() {
            let multisig = instruction_account(tx, instruction, 0)?;
            let member = instruction_account(tx, instruction, 1)?;
            let proposal = instruction_account(tx, instruction, 2)?;
            (
                event_kind::MULTISIG_PROPOSAL_REJECTED,
                format!("Squads rejection on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                }),
            )
        } else if discriminator == anchor_discriminator("proposal_cancel").as_slice()
            || discriminator == anchor_discriminator("proposal_cancel_v2").as_slice()
        {
            let multisig = instruction_account(tx, instruction, 0)?;
            let member = instruction_account(tx, instruction, 1)?;
            let proposal = instruction_account(tx, instruction, 2)?;
            (
                event_kind::MULTISIG_PROPOSAL_CANCELLED,
                format!("Squads cancellation on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                }),
            )
        } else if discriminator == anchor_discriminator("vault_transaction_execute").as_slice() {
            let multisig = instruction_account(tx, instruction, 0)?;
            let proposal = instruction_account(tx, instruction, 1)?;
            let transaction = instruction_account(tx, instruction, 2)?;
            let member = instruction_account(tx, instruction, 3)?;
            (
                event_kind::MULTISIG_VAULT_EXECUTED,
                format!("Squads vault execution on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                    "transaction": transaction.to_string(),
                }),
            )
        } else if discriminator == anchor_discriminator("config_transaction_execute").as_slice() {
            let multisig = instruction_account(tx, instruction, 0)?;
            let member = instruction_account(tx, instruction, 1)?;
            let proposal = instruction_account(tx, instruction, 2)?;
            let transaction = instruction_account(tx, instruction, 3)?;
            (
                event_kind::MULTISIG_CONFIG_EXECUTED,
                format!("Squads config execution on {multisig}"),
                Some(member.to_string()),
                json!({
                    "protocol": "squads",
                    "protocol_name": "Squads",
                    "multisig": multisig.to_string(),
                    "member": member.to_string(),
                    "proposal": proposal.to_string(),
                    "transaction": transaction.to_string(),
                }),
            )
        } else {
            return Ok(None);
        };

    let multisig = payload
        .get("multisig")
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("missing multisig in squads payload"))?
        .to_owned();

    let mut tags = vec![
        "target:multisig".to_owned(),
        "signal:multisig".to_owned(),
        "source:squads".to_owned(),
        "protocol:squads".to_owned(),
    ];
    if kind == event_kind::MULTISIG_CONFIG_EXECUTED {
        tags.push("signal:multisig_config_change".to_owned());
        tags.push("instruction:config_transaction_execute".to_owned());

        if let Some(payload) = payload.as_object_mut() {
            payload.insert(
                "instruction_name".to_owned(),
                json!("config_transaction_execute"),
            );
            payload.insert("classification".to_owned(), json!("multisig_config"));
        }
    }

    Ok(Some(build_event(
        tx,
        kind,
        target_type::MULTISIG,
        multisig,
        actor,
        slot,
        block_time,
        instruction_path,
        summary,
        tags,
        payload,
    )))
}

async fn parse_name_service_event(
    state: &SharedState,
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
) -> Result<Option<ChainEvent>> {
    let Some(domain_event) =
        name_service::parse_domain_instruction_event(state, tx, instruction, Some(slot)).await?
    else {
        return Ok(None);
    };

    let actor = match &domain_event.kind {
        DomainInstructionKind::Create { .. } => instruction_account(tx, instruction, 1).ok(),
        DomainInstructionKind::Update { .. } => instruction_account(tx, instruction, 1).ok(),
        DomainInstructionKind::Transfer { .. } => instruction_account(tx, instruction, 1).ok(),
        DomainInstructionKind::Delete { .. } => instruction_account(tx, instruction, 1).ok(),
        DomainInstructionKind::Realloc { .. } => instruction_account(tx, instruction, 3).ok(),
    }
    .map(|value| value.to_string())
    .or_else(|| tx.primary_signer.map(|value| value.to_string()));

    let risk_context = build_domain_risk_context(state, &domain_event, actor.as_deref()).await?;
    let (kind, summary, tags, payload) =
        build_name_service_event_parts(&domain_event, tx, instruction, &risk_context)?;

    Ok(Some(build_event(
        tx,
        kind,
        target_type::DOMAIN,
        domain_event.target_account.to_string(),
        actor,
        slot,
        block_time,
        instruction_path,
        summary,
        tags,
        payload,
    )))
}

fn build_name_service_event_parts(
    domain_event: &DomainInstructionEvent,
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    risk_context: &DomainRiskContext,
) -> Result<(&'static str, String, Vec<String>, serde_json::Value)> {
    let mut tags = vec![
        "target:domain".to_owned(),
        "sector:identity".to_owned(),
        "source:sns".to_owned(),
        "watch:sol_domain".to_owned(),
        if domain_event.root_domain {
            "domain:root".to_owned()
        } else {
            "domain:subdomain".to_owned()
        },
    ];

    let sensitive_domain = risk_context.explicit_watch || !risk_context.project_links.is_empty();
    if risk_context.explicit_watch {
        tags.push("watch:critical_domain".to_owned());
        tags.push("domain:explicit_watch".to_owned());
    }
    if !risk_context.project_links.is_empty() {
        tags.push("sector:defi".to_owned());
        tags.push("watch:defi".to_owned());
        tags.push("domain:project_linked".to_owned());
        for link in &risk_context.project_links {
            let protocol_tag = format!("protocol:{}", link.protocol_slug);
            if !tags.contains(&protocol_tag) {
                tags.push(protocol_tag);
            }
        }
    }

    let (kind, summary, mut payload) = match &domain_event.kind {
        DomainInstructionKind::Create { space, lamports } => (
            event_kind::NAME_SERVICE_DOMAIN_CREATED,
            format!("SNS domain created: {}", domain_event.target_name),
            json!({
                "domain_name": domain_event.target_name,
                "space": space,
                "lamports": lamports,
            }),
        ),
        DomainInstructionKind::Update { offset, data_len } => {
            tags.push("signal:domain_record_update".to_owned());
            if sensitive_domain {
                tags.push("severity:high".to_owned());
                tags.push("impact:high".to_owned());
                tags.push("alert:review_now".to_owned());
            }
            (
                event_kind::NAME_SERVICE_DOMAIN_UPDATED,
                format!("SNS domain updated: {}", domain_event.target_name),
                json!({
                    "domain_name": domain_event.target_name,
                    "offset": offset,
                    "data_len": data_len,
                }),
            )
        }
        DomainInstructionKind::Transfer { new_owner } => {
            tags.push("signal:domain_owner_change".to_owned());
            if sensitive_domain {
                tags.push("severity:critical".to_owned());
                tags.push("impact:high".to_owned());
                tags.push("alert:review_now".to_owned());
            }
            (
                event_kind::NAME_SERVICE_DOMAIN_TRANSFERRED,
                format!("SNS domain ownership changed: {}", domain_event.target_name),
                json!({
                    "domain_name": domain_event.target_name,
                    "new_owner": new_owner,
                }),
            )
        }
        DomainInstructionKind::Delete { refund_target } => {
            tags.push("signal:domain_deleted".to_owned());
            if sensitive_domain {
                tags.push("severity:critical".to_owned());
                tags.push("impact:high".to_owned());
                tags.push("alert:review_now".to_owned());
            }
            (
                event_kind::NAME_SERVICE_DOMAIN_DELETED,
                format!("SNS domain deleted: {}", domain_event.target_name),
                json!({
                    "domain_name": domain_event.target_name,
                    "refund_target": refund_target,
                }),
            )
        }
        DomainInstructionKind::Realloc { space } => {
            if sensitive_domain {
                tags.push("severity:high".to_owned());
                tags.push("impact:high".to_owned());
                tags.push("alert:review_now".to_owned());
            }
            (
                event_kind::NAME_SERVICE_DOMAIN_REALLOCATED,
                format!("SNS domain resized: {}", domain_event.target_name),
                json!({
                    "domain_name": domain_event.target_name,
                    "space": space,
                }),
            )
        }
    };

    let payload_object = payload
        .as_object_mut()
        .ok_or_else(|| anyhow!("SNS payload object creation failed"))?;
    payload_object.insert(
        "domain_account".to_owned(),
        json!(domain_event.target_account.to_string()),
    );
    if !domain_event.owner.is_empty() {
        payload_object.insert("owner".to_owned(), json!(domain_event.owner.clone()));
    }
    if !domain_event.class.is_empty() {
        payload_object.insert("class".to_owned(), json!(domain_event.class.clone()));
    }
    payload_object.insert("root_domain".to_owned(), json!(domain_event.root_domain));
    payload_object.insert(
        "parent_domain_account".to_owned(),
        json!(domain_event.parent.clone()),
    );
    payload_object.insert(
        "program".to_owned(),
        json!(name_service::program_id().to_string()),
    );
    payload_object.insert("stack_height".to_owned(), json!(instruction.stack_height));
    payload_object.insert(
        "primary_signer".to_owned(),
        json!(tx.primary_signer.map(|value| value.to_string())),
    );
    payload_object.insert("critical_domain".to_owned(), json!(sensitive_domain));
    payload_object.insert(
        "critical_domain_reasons".to_owned(),
        json!({
            "explicit_watch": risk_context.explicit_watch,
            "project_link_count": risk_context.project_links.len(),
        }),
    );
    payload_object.insert(
        "linked_protocols".to_owned(),
        json!(
            risk_context
                .project_links
                .iter()
                .map(|link| json!({
                    "slug": link.protocol_slug,
                    "name": link.protocol_name,
                    "evidence": link.evidence,
                }))
                .collect::<Vec<_>>()
        ),
    );

    Ok((kind, summary, tags, payload))
}

async fn build_domain_risk_context(
    state: &SharedState,
    domain_event: &DomainInstructionEvent,
    actor: Option<&str>,
) -> Result<DomainRiskContext> {
    let mut addresses = vec![domain_event.owner.clone()];

    match &domain_event.kind {
        DomainInstructionKind::Transfer { new_owner } => addresses.push(new_owner.clone()),
        DomainInstructionKind::Delete { refund_target } => addresses.push(refund_target.clone()),
        DomainInstructionKind::Create { .. }
        | DomainInstructionKind::Update { .. }
        | DomainInstructionKind::Realloc { .. } => {}
    }

    if let Some(actor) = actor {
        addresses.push(actor.to_owned());
    }

    addresses.sort();
    addresses.dedup();

    let mut seen_protocols = HashMap::<String, String>::new();
    let mut project_links = Vec::<DomainProjectLink>::new();

    if let Some(protocol) = protocols::protocol_for_sol_domain(&domain_event.target_name) {
        seen_protocols.insert(protocol.slug.clone(), "sol_domain".to_owned());
        project_links.push(DomainProjectLink {
            protocol_slug: protocol.slug,
            protocol_name: protocol.name,
            evidence: humanize_domain_link_evidence("sol_domain"),
        });
    }

    for address in addresses {
        if address.is_empty() {
            continue;
        }

        for link in state.db.address_protocol_links(&address, 64).await? {
            if seen_protocols.contains_key(&link.protocol_slug) {
                continue;
            }

            if let Some(protocol) = protocols::curated_protocol_descriptor(&link.protocol_slug) {
                seen_protocols.insert(link.protocol_slug.clone(), link.evidence.clone());
                project_links.push(DomainProjectLink {
                    protocol_slug: link.protocol_slug,
                    protocol_name: protocol.name,
                    evidence: humanize_domain_link_evidence(&link.evidence),
                });
            }
        }
    }

    Ok(DomainRiskContext {
        explicit_watch: state
            .config
            .critical_sol_domains
            .iter()
            .any(|domain| domain == &domain_event.target_name.to_ascii_lowercase())
            || protocols::protocol_for_sol_domain(&domain_event.target_name).is_some(),
        project_links,
    })
}

fn humanize_domain_link_evidence(evidence: &str) -> String {
    match evidence {
        "sol_domain" => "tracked .sol domain".to_owned(),
        "program_surface" => "official program".to_owned(),
        "multisig" => "related multisig".to_owned(),
        "multisig_signer" => "known multisig signer".to_owned(),
        "actor" => "known protocol actor".to_owned(),
        "domain_owner" => "current domain owner".to_owned(),
        "domain_new_owner" => "new domain owner".to_owned(),
        "domain_refund_target" => "domain refund target".to_owned(),
        _ => "related activity".to_owned(),
    }
}

fn parse_program_metadata_artifacts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<(Option<ChainEvent>, Option<ProgramIdlDataWrite>)> {
    let Some(parsed) = program_metadata::parse_instruction(&instruction.data) else {
        return Ok((None, None));
    };

    let metadata_account = instruction_account(tx, instruction, 0).ok();
    let target_from_program_account =
        program_metadata_target_from_program_account(tx, instruction, metadata_account, &parsed)?;
    let target = target_from_program_account.or_else(|| {
        metadata_account.and_then(|account| protocols::protocol_for_idl_metadata_account(account))
    });

    let Some(target) = target else {
        return Ok((None, None));
    };

    let canonical_metadata_account =
        program_metadata::canonical_idl_metadata_address(target.program_id);
    if metadata_account != Some(canonical_metadata_account) {
        return Ok((None, None));
    }

    if parsed
        .seed
        .as_deref()
        .is_some_and(|seed| seed != program_metadata::CANONICAL_IDL_SEED)
    {
        return Ok((None, None));
    }

    let actor = parent_context
        .and_then(|context| context.squads_execution.as_ref())
        .and_then(|context| context.member.clone())
        .or_else(|| tx.primary_signer.map(|value| value.to_string()));
    let idl_write = program_metadata_write_bytes(&instruction.data, &parsed)
        .map(|bytes| {
            build_program_idl_data_write(
                tx,
                slot,
                block_time,
                instruction_path,
                anchor_idl::PROGRAM_METADATA_IDL_ORIGIN.to_owned(),
                canonical_metadata_account,
                canonical_metadata_account,
                target.program_id,
                actor.as_deref(),
                parsed.offset.map(i64::from).or(Some(0)),
                bytes,
            )
        })
        .transpose()?;

    let event_kind = match parsed.kind {
        ProgramMetadataInstructionKind::Initialize => event_kind::PROGRAM_IDL_INITIALIZED,
        ProgramMetadataInstructionKind::SetAuthority => event_kind::PROGRAM_IDL_AUTHORITY_CHANGED,
        ProgramMetadataInstructionKind::SetData => event_kind::PROGRAM_IDL_UPDATED,
        ProgramMetadataInstructionKind::SetImmutable => event_kind::PROGRAM_IDL_IMMUTABLE,
        ProgramMetadataInstructionKind::Trim | ProgramMetadataInstructionKind::Extend => {
            event_kind::PROGRAM_IDL_RESIZED
        }
        ProgramMetadataInstructionKind::Close => event_kind::PROGRAM_IDL_CLOSED,
        ProgramMetadataInstructionKind::Write | ProgramMetadataInstructionKind::Allocate => {
            return Ok((None, idl_write));
        }
    };

    Ok((
        Some(build_event(
            tx,
            event_kind,
            target_type::PROGRAM,
            target.program_id.to_string(),
            actor.clone(),
            slot,
            block_time,
            instruction_path,
            summary_for_idl_event(&target, &parsed, parent_context),
            idl_update_tags(&target, &parsed, parent_context),
            idl_update_payload(
                &target,
                canonical_metadata_account,
                &parsed,
                instruction,
                parent_context,
            ),
        )),
        idl_write,
    ))
}

fn program_metadata_target_from_program_account(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    metadata_account: Option<Pubkey>,
    parsed: &ParsedProgramMetadataInstruction,
) -> Result<Option<protocols::DefiProtocolMatch>> {
    let program_account_position = match parsed.kind {
        ProgramMetadataInstructionKind::Initialize
        | ProgramMetadataInstructionKind::SetAuthority
        | ProgramMetadataInstructionKind::SetData
        | ProgramMetadataInstructionKind::SetImmutable
        | ProgramMetadataInstructionKind::Trim
        | ProgramMetadataInstructionKind::Close
        | ProgramMetadataInstructionKind::Extend => Some(2),
        ProgramMetadataInstructionKind::Write | ProgramMetadataInstructionKind::Allocate => None,
    };

    let Some(program_account_position) = program_account_position else {
        return Ok(None);
    };
    let Some(program) = instruction_account(tx, instruction, program_account_position).ok() else {
        return Ok(None);
    };
    let Some(protocol) = protocols::protocol_for_program(program) else {
        return Ok(None);
    };

    if metadata_account == Some(program_metadata::canonical_idl_metadata_address(program)) {
        Ok(Some(protocol))
    } else {
        Ok(None)
    }
}

fn program_metadata_write_bytes(
    instruction_data: &[u8],
    parsed: &ParsedProgramMetadataInstruction,
) -> Option<Vec<u8>> {
    let start = match parsed.kind {
        ProgramMetadataInstructionKind::Write => 5,
        ProgramMetadataInstructionKind::Initialize => 21,
        ProgramMetadataInstructionKind::SetData => 5,
        _ => return None,
    };
    let len = parsed.inline_data_len?;
    Some(instruction_data.get(start..start + len)?.to_vec())
}

fn parse_anchor_legacy_idl_artifacts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    program_id: Pubkey,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<(Option<ChainEvent>, Option<ProgramIdlDataWrite>)> {
    let Some(parsed) = anchor_idl::parse_instruction(&instruction.data) else {
        return Ok((None, None));
    };
    let Some(target) = protocols::protocol_for_program(program_id) else {
        return Ok((None, None));
    };
    let Some(canonical_idl_account) = anchor_idl::legacy_idl_address(program_id) else {
        return Ok((None, None));
    };

    let actor = parent_context
        .and_then(|context| context.squads_execution.as_ref())
        .and_then(|context| context.member.clone())
        .or_else(|| anchor_legacy_idl_authority(tx, instruction, &parsed))
        .or_else(|| tx.primary_signer.map(|value| value.to_string()));

    let idl_write = if parsed.kind == AnchorIdlInstructionKind::Write {
        let source_account = instruction_account(tx, instruction, 0).ok();
        match (source_account, parsed.write_data.clone()) {
            (Some(source_account), Some(bytes)) => Some(build_program_idl_data_write(
                tx,
                slot,
                block_time,
                instruction_path,
                anchor_idl::LEGACY_ANCHOR_IDL_ORIGIN.to_owned(),
                canonical_idl_account,
                source_account,
                program_id,
                actor.as_deref(),
                None,
                bytes,
            )?),
            _ => None,
        }
    } else {
        None
    };

    let Some(event_kind) = anchor_legacy_idl_event_kind(parsed.kind) else {
        return Ok((None, idl_write));
    };
    let Some((metadata_account, source_account)) =
        anchor_legacy_idl_accounts(tx, instruction, parsed.kind, canonical_idl_account)
    else {
        return Ok((None, idl_write));
    };

    if metadata_account != canonical_idl_account {
        return Ok((None, idl_write));
    }

    Ok((
        Some(build_event(
            tx,
            event_kind,
            target_type::PROGRAM,
            program_id.to_string(),
            actor.clone(),
            slot,
            block_time,
            instruction_path,
            summary_for_anchor_legacy_idl_event(&target, &parsed, parent_context),
            anchor_legacy_idl_tags(&target, &parsed, parent_context),
            anchor_legacy_idl_payload(
                &target,
                metadata_account,
                source_account,
                &parsed,
                instruction,
                parent_context,
                actor.as_deref(),
            ),
        )),
        idl_write,
    ))
}

fn anchor_legacy_idl_event_kind(kind: AnchorIdlInstructionKind) -> Option<&'static str> {
    match kind {
        AnchorIdlInstructionKind::Create => Some(event_kind::PROGRAM_IDL_INITIALIZED),
        AnchorIdlInstructionKind::SetBuffer => Some(event_kind::PROGRAM_IDL_UPDATED),
        AnchorIdlInstructionKind::SetAuthority => Some(event_kind::PROGRAM_IDL_AUTHORITY_CHANGED),
        AnchorIdlInstructionKind::Close => Some(event_kind::PROGRAM_IDL_CLOSED),
        AnchorIdlInstructionKind::Resize => Some(event_kind::PROGRAM_IDL_RESIZED),
        AnchorIdlInstructionKind::CreateBuffer | AnchorIdlInstructionKind::Write => None,
    }
}

fn anchor_legacy_idl_accounts(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    kind: AnchorIdlInstructionKind,
    canonical_idl_account: Pubkey,
) -> Option<(Pubkey, Pubkey)> {
    match kind {
        AnchorIdlInstructionKind::Create => instruction_account(tx, instruction, 1)
            .ok()
            .map(|metadata_account| (metadata_account, metadata_account)),
        AnchorIdlInstructionKind::SetBuffer => {
            let source_account = instruction_account(tx, instruction, 0).ok()?;
            let metadata_account = instruction_account(tx, instruction, 1).ok()?;
            Some((metadata_account, source_account))
        }
        AnchorIdlInstructionKind::SetAuthority
        | AnchorIdlInstructionKind::Close
        | AnchorIdlInstructionKind::Resize => instruction_account(tx, instruction, 0)
            .ok()
            .map(|metadata_account| (metadata_account, metadata_account)),
        AnchorIdlInstructionKind::CreateBuffer | AnchorIdlInstructionKind::Write => Some((
            canonical_idl_account,
            instruction_account(tx, instruction, 0).ok()?,
        )),
    }
}

fn anchor_legacy_idl_authority(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    parsed: &ParsedAnchorIdlInstruction,
) -> Option<String> {
    let account_position = match parsed.kind {
        AnchorIdlInstructionKind::Create => 0,
        AnchorIdlInstructionKind::CreateBuffer => 1,
        AnchorIdlInstructionKind::Write => 1,
        AnchorIdlInstructionKind::SetBuffer => 2,
        AnchorIdlInstructionKind::SetAuthority => 1,
        AnchorIdlInstructionKind::Close => 1,
        AnchorIdlInstructionKind::Resize => 1,
    };
    instruction_account(tx, instruction, account_position)
        .ok()
        .map(|value| value.to_string())
}

fn parse_protocol_instruction_event(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    parent_context: Option<&ParentInstructionContext>,
) -> Result<(Option<ChainEvent>, Vec<ProtocolMetricObservation>)> {
    let target_program = program_id(tx, instruction)
        .ok_or_else(|| anyhow!("known protocol instruction missing program id"))?;
    let signal = match protocols::resolve_protocol_instruction(target_program, &instruction.data) {
        Some(signal) => signal,
        None => return Ok((None, Vec::new())),
    };
    let metric_observations = decode_protocol_metric_observations(
        tx,
        instruction,
        slot,
        block_time,
        instruction_path,
        &signal,
    );

    let Some(signal_kind) = signal.signal_kind else {
        return Ok((None, metric_observations));
    };

    if !should_emit_protocol_signal(&signal, target_program, parent_context) {
        debug!(
            signature = %tx.signature,
            slot,
            instruction_path,
            protocol = signal.protocol.slug,
            discriminator = %signal.discriminator_hex,
            parent_program = parent_context
                .and_then(|context| context.parent_program)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            immediate_parent_program = parent_context
                .and_then(|context| context.immediate_parent_program)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            "suppressed protocol signal that matches an internal helper pattern",
        );
        return Ok((None, metric_observations));
    }

    let instruction_details = protocol_instruction_details(tx, instruction, &signal);
    let high_impact = instruction_details
        .high_impact_override
        .unwrap_or(signal.high_impact);
    let actor = parent_context
        .and_then(|context| context.squads_execution.as_ref())
        .and_then(|context| context.member.clone())
        .or_else(|| instruction_details.actor.clone())
        .or_else(|| tx.primary_signer.map(|value| value.to_string()));
    let summary = summary_for_protocol_signal(&signal, parent_context, high_impact);

    Ok((
        Some(build_event(
            tx,
            match signal_kind {
                ProtocolSignalKind::Admin => event_kind::PROGRAM_ADMIN_INSTRUCTION,
                ProtocolSignalKind::Unknown => event_kind::PROGRAM_UNKNOWN_INSTRUCTION,
            },
            target_type::PROGRAM,
            target_program.to_string(),
            actor,
            slot,
            block_time,
            instruction_path,
            summary,
            protocol_signal_tags(&signal, parent_context, high_impact),
            protocol_signal_payload(
                &signal,
                instruction,
                parent_context,
                high_impact,
                &instruction_details,
            ),
        )),
        metric_observations,
    ))
}

fn idl_update_tags(
    target: &protocols::DefiProtocolMatch,
    parsed: &ParsedProgramMetadataInstruction,
    parent_context: Option<&ParentInstructionContext>,
) -> Vec<String> {
    let mut tags = vec![
        "target:program".to_owned(),
        "sector:defi".to_owned(),
        "watch:defi".to_owned(),
        format!("protocol:{}", target.slug),
        "signal:idl_update".to_owned(),
        "metadata:idl".to_owned(),
        "source:program_metadata".to_owned(),
        format!("instruction:{}", parsed.kind.as_str()),
    ];

    if idl_update_high_impact(parsed.kind) {
        tags.push("severity:high".to_owned());
        tags.push("impact:high".to_owned());
    }

    if parent_context
        .and_then(|context| context.parent_program)
        .is_some_and(|program| program == squads_program_id())
    {
        tags.push("source:squads".to_owned());
    }

    tags
}

fn idl_update_payload(
    target: &protocols::DefiProtocolMatch,
    metadata_account: Pubkey,
    parsed: &ParsedProgramMetadataInstruction,
    instruction: &ResolvedInstruction,
    parent_context: Option<&ParentInstructionContext>,
) -> serde_json::Value {
    json!({
        "program": target.program_id.to_string(),
        "program_label": target.program_label,
        "protocol": target.slug,
        "protocol_name": target.name,
        "instruction_name": parsed.kind.as_str(),
        "classification": "idl_update",
        "severity": if idl_update_high_impact(parsed.kind) { "high" } else { "standard" },
        "high_impact": idl_update_high_impact(parsed.kind),
        "idl_origin": anchor_idl::PROGRAM_METADATA_IDL_ORIGIN,
        "idl_source_account": metadata_account.to_string(),
        "metadata_account": metadata_account.to_string(),
        "metadata_seed": program_metadata::CANONICAL_IDL_SEED,
        "encoding": parsed.encoding.map(|value| value.as_str()),
        "compression": parsed.compression.map(|value| value.as_str()),
        "format": parsed.format.map(|value| value.as_str()),
        "data_source": parsed.data_source.map(|value| value.as_str()),
        "inline_data_len": parsed.inline_data_len,
        "offset": parsed.offset,
        "length": parsed.length,
        "new_authority": parsed.new_authority.clone().flatten(),
        "parent_program": parent_context
            .and_then(|context| context.parent_program)
            .map(|value| value.to_string()),
        "immediate_parent_program": parent_context
            .and_then(|context| context.immediate_parent_program)
            .map(|value| value.to_string()),
        "call_origin": call_origin_label(target.program_id, parent_context),
        "stack_height": instruction.stack_height,
        "via_multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .is_some(),
        "multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .map(|context| context.multisig.clone()),
        "proposal": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.proposal.clone()),
        "transaction": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.transaction.clone()),
        "member": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.member.clone()),
    })
}

fn summary_for_idl_event(
    target: &protocols::DefiProtocolMatch,
    parsed: &ParsedProgramMetadataInstruction,
    parent_context: Option<&ParentInstructionContext>,
) -> String {
    let surface = if target.program_label == target.name {
        target.name.to_owned()
    } else {
        format!("{} {}", target.name, target.program_label)
    };

    let prefix = match parsed.kind {
        ProgramMetadataInstructionKind::Initialize => "IDL initialized for",
        ProgramMetadataInstructionKind::SetAuthority => "IDL authority changed for",
        ProgramMetadataInstructionKind::SetData => "IDL updated for",
        ProgramMetadataInstructionKind::SetImmutable => "IDL marked immutable for",
        ProgramMetadataInstructionKind::Trim | ProgramMetadataInstructionKind::Extend => {
            "IDL resized for"
        }
        ProgramMetadataInstructionKind::Close => "IDL closed for",
        ProgramMetadataInstructionKind::Write | ProgramMetadataInstructionKind::Allocate => {
            "IDL activity for"
        }
    };

    if let Some(context) = parent_context.and_then(|context| context.squads_execution.as_ref()) {
        return format!(
            "{prefix} {surface} via Squads multisig {}",
            context.multisig
        );
    }

    format!("{prefix} {surface}")
}

fn idl_update_high_impact(kind: ProgramMetadataInstructionKind) -> bool {
    matches!(
        kind,
        ProgramMetadataInstructionKind::SetAuthority
            | ProgramMetadataInstructionKind::SetImmutable
            | ProgramMetadataInstructionKind::Close
    )
}

fn anchor_legacy_idl_tags(
    target: &protocols::DefiProtocolMatch,
    parsed: &ParsedAnchorIdlInstruction,
    parent_context: Option<&ParentInstructionContext>,
) -> Vec<String> {
    let mut tags = vec![
        "target:program".to_owned(),
        "sector:defi".to_owned(),
        "watch:defi".to_owned(),
        format!("protocol:{}", target.slug),
        "signal:idl_update".to_owned(),
        "metadata:idl".to_owned(),
        "source:anchor_legacy".to_owned(),
        format!("instruction:{}", parsed.kind.as_str()),
    ];

    if matches!(
        parsed.kind,
        AnchorIdlInstructionKind::SetAuthority | AnchorIdlInstructionKind::Close
    ) {
        tags.push("severity:high".to_owned());
        tags.push("impact:high".to_owned());
    }

    if parent_context
        .and_then(|context| context.parent_program)
        .is_some_and(|program| program == squads_program_id())
    {
        tags.push("source:squads".to_owned());
    }

    tags
}

fn anchor_legacy_idl_payload(
    target: &protocols::DefiProtocolMatch,
    metadata_account: Pubkey,
    source_account: Pubkey,
    parsed: &ParsedAnchorIdlInstruction,
    instruction: &ResolvedInstruction,
    parent_context: Option<&ParentInstructionContext>,
    actor: Option<&str>,
) -> serde_json::Value {
    json!({
        "program": target.program_id.to_string(),
        "program_label": target.program_label,
        "protocol": target.slug,
        "protocol_name": target.name,
        "instruction_name": parsed.kind.as_str(),
        "classification": "idl_update",
        "severity": if matches!(parsed.kind, AnchorIdlInstructionKind::SetAuthority | AnchorIdlInstructionKind::Close) { "high" } else { "standard" },
        "high_impact": matches!(parsed.kind, AnchorIdlInstructionKind::SetAuthority | AnchorIdlInstructionKind::Close),
        "idl_origin": anchor_idl::LEGACY_ANCHOR_IDL_ORIGIN,
        "idl_source_account": source_account.to_string(),
        "metadata_account": metadata_account.to_string(),
        "metadata_seed": anchor_idl::LEGACY_ANCHOR_IDL_SEED,
        "encoding": "utf8",
        "compression": "zlib",
        "format": "json",
        "data_source": "anchor_idl_account",
        "data_len": parsed.data_len,
        "inline_data_len": parsed.write_data.as_ref().map(Vec::len),
        "new_authority": parsed.new_authority.clone(),
        "authority": actor,
        "parent_program": parent_context
            .and_then(|context| context.parent_program)
            .map(|value| value.to_string()),
        "immediate_parent_program": parent_context
            .and_then(|context| context.immediate_parent_program)
            .map(|value| value.to_string()),
        "call_origin": call_origin_label(target.program_id, parent_context),
        "stack_height": instruction.stack_height,
        "via_multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .is_some(),
        "multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .map(|context| context.multisig.clone()),
        "proposal": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.proposal.clone()),
        "transaction": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.transaction.clone()),
        "member": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.member.clone()),
    })
}

fn summary_for_anchor_legacy_idl_event(
    target: &protocols::DefiProtocolMatch,
    parsed: &ParsedAnchorIdlInstruction,
    parent_context: Option<&ParentInstructionContext>,
) -> String {
    let surface = if target.program_label == target.name {
        target.name.to_owned()
    } else {
        format!("{} {}", target.name, target.program_label)
    };

    let prefix = match parsed.kind {
        AnchorIdlInstructionKind::Create => "Legacy Anchor IDL initialized for",
        AnchorIdlInstructionKind::SetBuffer => "Legacy Anchor IDL updated for",
        AnchorIdlInstructionKind::SetAuthority => "Legacy Anchor IDL authority changed for",
        AnchorIdlInstructionKind::Close => "Legacy Anchor IDL closed for",
        AnchorIdlInstructionKind::Resize => "Legacy Anchor IDL resized for",
        AnchorIdlInstructionKind::CreateBuffer | AnchorIdlInstructionKind::Write => {
            "Legacy Anchor IDL activity for"
        }
    };

    if let Some(context) = parent_context.and_then(|context| context.squads_execution.as_ref()) {
        return format!(
            "{prefix} {surface} via Squads multisig {}",
            context.multisig
        );
    }

    format!("{prefix} {surface}")
}

fn parent_context_for_instruction(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
) -> Result<ParentInstructionContext> {
    let parent_program = program_id(tx, instruction);
    let squads_execution = match parent_program {
        Some(program) if program == squads_program_id() => {
            extract_squads_execution_context(tx, instruction)?
        }
        _ => None,
    };

    Ok(ParentInstructionContext {
        parent_program,
        immediate_parent_program: parent_program,
        squads_execution,
    })
}

fn inner_parent_contexts_for_instruction_set(
    tx: &ResolvedTransaction,
    top_level_instruction: &ResolvedInstruction,
    inner_instructions: &[ResolvedInstruction],
) -> Result<Vec<ParentInstructionContext>> {
    let base_context = parent_context_for_instruction(tx, top_level_instruction)?;
    let mut active_programs = HashMap::<u32, Pubkey>::new();

    if let Some(parent_program) = base_context.parent_program {
        active_programs.insert(1, parent_program);
    }

    let mut contexts = Vec::with_capacity(inner_instructions.len());
    for instruction in inner_instructions {
        let stack_height = instruction.stack_height.unwrap_or(2).max(2);
        active_programs.retain(|height, _| *height < stack_height);

        let immediate_parent_program = stack_height
            .checked_sub(1)
            .and_then(|height| active_programs.get(&height).copied())
            .or(base_context.parent_program);

        contexts.push(ParentInstructionContext {
            parent_program: base_context.parent_program,
            immediate_parent_program,
            squads_execution: base_context.squads_execution.clone(),
        });

        if let Some(program) = program_id(tx, instruction) {
            active_programs.insert(stack_height, program);
        }
    }

    Ok(contexts)
}

fn extract_squads_execution_context(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
) -> Result<Option<SquadsExecutionContext>> {
    let Some(discriminator) = instruction.data.get(..8) else {
        return Ok(None);
    };

    if discriminator == anchor_discriminator("vault_transaction_execute").as_slice() {
        return Ok(Some(SquadsExecutionContext {
            multisig: instruction_account(tx, instruction, 0)?.to_string(),
            proposal: Some(instruction_account(tx, instruction, 1)?.to_string()),
            transaction: Some(instruction_account(tx, instruction, 2)?.to_string()),
            member: Some(instruction_account(tx, instruction, 3)?.to_string()),
        }));
    }

    if discriminator == anchor_discriminator("config_transaction_execute").as_slice() {
        return Ok(Some(SquadsExecutionContext {
            multisig: instruction_account(tx, instruction, 0)?.to_string(),
            proposal: Some(instruction_account(tx, instruction, 2)?.to_string()),
            transaction: Some(instruction_account(tx, instruction, 3)?.to_string()),
            member: Some(instruction_account(tx, instruction, 1)?.to_string()),
        }));
    }

    Ok(None)
}

fn enrich_multisig_event(event: &mut ChainEvent, inner_events: &[ChainEvent]) {
    if !matches!(
        event.kind.as_str(),
        event_kind::MULTISIG_VAULT_EXECUTED | event_kind::MULTISIG_CONFIG_EXECUTED
    ) {
        return;
    }

    let Some(decoded_event) = inner_events.iter().find(|candidate| {
        candidate.has_tag("watch:defi") || is_program_update_kind(candidate.kind.as_str())
    }) else {
        return;
    };

    event.summary = format!("{} -> {}", event.summary, decoded_event.summary);

    let Some(payload) = event.payload.as_object_mut() else {
        return;
    };

    payload.insert(
        "decoded_program".to_owned(),
        json!(decoded_event.target_address.clone()),
    );
    payload.insert(
        "decoded_summary".to_owned(),
        json!(decoded_event.summary.clone()),
    );
    payload.insert("decoded_kind".to_owned(), json!(decoded_event.kind.clone()));

    if let Some(protocol) = decoded_event.payload.get("protocol").cloned() {
        payload.insert("decoded_protocol".to_owned(), protocol);
    }

    if let Some(instruction_name) = decoded_event.payload.get("instruction_name").cloned() {
        payload.insert("decoded_instruction_name".to_owned(), instruction_name);
    }
}

fn decode_protocol_metric_observations(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    signal: &ProtocolInstructionSignal,
) -> Vec<ProtocolMetricObservation> {
    let Some(instruction_name) = signal.instruction_name.as_deref() else {
        return Vec::new();
    };

    let account_addresses = instruction
        .accounts
        .iter()
        .filter_map(|account_index| tx.account_keys.get(*account_index))
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let program_id = signal.protocol.program_id.to_string();

    protocol_metrics::decode_metric_observations(&MetricDecoderInput {
        signature: &tx.signature,
        instruction_path,
        slot: slot as i64,
        block_time,
        protocol_slug: &signal.protocol.slug,
        protocol_name: &signal.protocol.name,
        program_id: &program_id,
        program_label: &signal.protocol.program_label,
        instruction_name: Some(instruction_name),
        account_addresses: &account_addresses,
        log_messages: &tx.log_messages,
    })
}

fn build_metric_alert_events(
    tx: &ResolvedTransaction,
    observation: &ProtocolMetricObservation,
    previous_observation: Option<&ProtocolMetricObservation>,
    rules: &[MetricAlertRule],
) -> Vec<ChainEvent> {
    rules
        .iter()
        .filter_map(|rule| build_metric_alert_event(tx, observation, previous_observation, rule))
        .collect()
}

fn build_metric_alert_event(
    tx: &ResolvedTransaction,
    observation: &ProtocolMetricObservation,
    previous_observation: Option<&ProtocolMetricObservation>,
    rule: &MetricAlertRule,
) -> Option<ChainEvent> {
    let below_min = rule
        .min_value
        .is_some_and(|min_value| observation.metric_value < min_value);
    let above_max = rule
        .max_value
        .is_some_and(|max_value| observation.metric_value > max_value);

    if !below_min && !above_max {
        return None;
    }

    let direction = if below_min { "below" } else { "above" };
    let actor = tx.primary_signer.map(|value| value.to_string());
    let mut payload = json!({
        "program": observation.program_id,
        "program_label": observation.program_label,
        "protocol": observation.protocol_slug,
        "protocol_name": observation.protocol_name,
        "instruction_name": observation.instruction_name,
        "classification": "metric_anomaly",
        "severity": "high",
        "metric_key": observation.metric_key,
        "metric_label": observation.metric_label,
        "metric_value": observation.metric_value,
        "subject_address": observation.subject_address,
        "subject_kind": observation.subject_kind,
        "subject_label": observation.subject_label,
        "rule_id": rule.id,
        "rule_label": rule.label,
        "rule_min_value": rule.min_value,
        "rule_max_value": rule.max_value,
        "threshold_direction": direction,
    });
    if let Some(previous) = previous_observation {
        let delta = observation.metric_value - previous.metric_value;
        if previous.metric_value != 0.0 {
            payload["previous_metric_value"] = json!(previous.metric_value);
            payload["metric_delta"] = json!(delta);
            payload["metric_delta_ratio"] = json!(delta / previous.metric_value);
        }
    }

    Some(ChainEvent {
        id: format!(
            "{}:{}:{}:{}",
            tx.signature,
            observation.instruction_path,
            event_kind::PROGRAM_METRIC_ALERT,
            rule.id
        ),
        signature: tx.signature.clone(),
        instruction_path: observation.instruction_path.clone(),
        kind: event_kind::PROGRAM_METRIC_ALERT.to_owned(),
        target_type: target_type::PROGRAM.to_owned(),
        target_address: observation.program_id.clone(),
        actor,
        slot: observation.slot,
        block_time: observation.block_time,
        indexed_at: now_ts(),
        summary: format!(
            "{} {} moved {} threshold for {}",
            observation.protocol_name, observation.metric_label, direction, rule.label
        ),
        tags: normalize_tags(vec![
            "target:program".to_owned(),
            "sector:defi".to_owned(),
            "watch:defi".to_owned(),
            format!("protocol:{}", observation.protocol_slug),
            format!("instruction:{}", observation.instruction_name),
            format!("metric:{}", observation.metric_key),
            "signal:metric_anomaly".to_owned(),
            "severity:high".to_owned(),
            "impact:high".to_owned(),
        ]),
        payload,
        delivery_attempts: 0,
        delivered_at: None,
        last_error: None,
        next_retry_at: now_ts(),
    })
}

fn program_update_tags(
    target_program: &Pubkey,
    parent_context: Option<&ParentInstructionContext>,
) -> Vec<String> {
    let mut tags = vec![
        "target:program".to_owned(),
        "signal:program_update".to_owned(),
    ];

    if parent_context
        .and_then(|context| context.parent_program)
        .is_some_and(|program| program == squads_program_id())
    {
        tags.push("source:squads".to_owned());
    }

    if let Some(protocol) = protocols::protocol_for_program(*target_program) {
        tags.push("sector:defi".to_owned());
        tags.push("watch:defi".to_owned());
        tags.push(format!("protocol:{}", protocol.slug));
    }

    tags
}

fn protocol_signal_tags(
    signal: &ProtocolInstructionSignal,
    parent_context: Option<&ParentInstructionContext>,
    high_impact: bool,
) -> Vec<String> {
    let mut tags = vec![
        "target:program".to_owned(),
        "sector:defi".to_owned(),
        "watch:defi".to_owned(),
        format!("protocol:{}", signal.protocol.slug),
    ];

    match signal.signal_kind {
        Some(ProtocolSignalKind::Admin) => {
            tags.push("signal:admin_instruction".to_owned());
            if high_impact {
                tags.push("severity:high".to_owned());
            }
        }
        Some(ProtocolSignalKind::Unknown) => {
            tags.push("signal:unknown_instruction".to_owned());
            tags.push("severity:critical".to_owned());
            tags.push("alert:review_now".to_owned());
        }
        None => {}
    }

    if high_impact {
        tags.push("impact:high".to_owned());
    }

    if let Some(instruction_name) = &signal.instruction_name {
        tags.push(format!("instruction:{instruction_name}"));
    }

    if parent_context
        .and_then(|context| context.parent_program)
        .is_some_and(|program| program == squads_program_id())
    {
        tags.push("source:squads".to_owned());
    }

    tags
}

fn protocol_signal_payload(
    signal: &ProtocolInstructionSignal,
    instruction: &ResolvedInstruction,
    parent_context: Option<&ParentInstructionContext>,
    high_impact: bool,
    instruction_details: &ProtocolInstructionDetails,
) -> serde_json::Value {
    let target_program = signal.protocol.program_id;
    let mut payload = json!({
        "program": signal.protocol.program_id.to_string(),
        "program_label": signal.protocol.program_label,
        "protocol": signal.protocol.slug,
        "protocol_name": signal.protocol.name,
        "instruction_name": signal.instruction_name,
        "classification": match signal.signal_kind {
            Some(ProtocolSignalKind::Admin) => "admin",
            Some(ProtocolSignalKind::Unknown) => "unknown",
            None => "known",
        },
        "severity": protocol_signal_severity(signal, high_impact),
        "high_impact": high_impact,
        "discriminator": signal.discriminator_hex,
        "parent_program": parent_context
            .and_then(|context| context.parent_program)
            .map(|value| value.to_string()),
        "immediate_parent_program": parent_context
            .and_then(|context| context.immediate_parent_program)
            .map(|value| value.to_string()),
        "call_origin": call_origin_label(target_program, parent_context),
        "stack_height": instruction.stack_height,
        "via_multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .is_some(),
        "multisig": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .map(|context| context.multisig.clone()),
        "proposal": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.proposal.clone()),
        "transaction": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.transaction.clone()),
        "member": parent_context
            .and_then(|context| context.squads_execution.as_ref())
            .and_then(|context| context.member.clone()),
    });

    if let Some(object) = payload.as_object_mut() {
        for (key, value) in &instruction_details.payload_fields {
            object.insert((*key).to_owned(), json!(value));
        }
    }

    payload
}

fn summary_for_protocol_signal(
    signal: &ProtocolInstructionSignal,
    parent_context: Option<&ParentInstructionContext>,
    high_impact: bool,
) -> String {
    let mut summary = match signal.signal_kind {
        Some(ProtocolSignalKind::Admin) => match signal.instruction_name.as_deref() {
            Some(instruction_name) => {
                format!(
                    "{} admin instruction {instruction_name}",
                    signal.protocol.name
                )
            }
            None => format!("{} admin instruction", signal.protocol.name),
        },
        Some(ProtocolSignalKind::Unknown) => {
            format!(
                "{} unknown instruction ({})",
                signal.protocol.name, signal.discriminator_hex
            )
        }
        None => match signal.instruction_name.as_deref() {
            Some(instruction_name) => {
                format!("{} instruction {instruction_name}", signal.protocol.name)
            }
            None => format!("{} instruction", signal.protocol.name),
        },
    };

    if high_impact {
        summary = format!("High-impact {summary}");
    }

    if let Some(context) = parent_context.and_then(|context| context.squads_execution.as_ref()) {
        summary = format!("{summary} via Squads multisig {}", context.multisig);
    }

    summary
}

fn should_emit_protocol_signal(
    signal: &ProtocolInstructionSignal,
    target_program: Pubkey,
    parent_context: Option<&ParentInstructionContext>,
) -> bool {
    !matches!(signal.signal_kind, Some(ProtocolSignalKind::Unknown))
        || parent_context.and_then(|context| context.immediate_parent_program)
            != Some(target_program)
}

fn call_origin_label(
    target_program: Pubkey,
    parent_context: Option<&ParentInstructionContext>,
) -> &'static str {
    let Some(context) = parent_context else {
        return "top_level";
    };

    if context.squads_execution.is_some() {
        return "squads_execution";
    }

    if context.immediate_parent_program == Some(target_program) {
        return "self_cpi";
    }

    if context.parent_program.is_some() {
        return "routed_cpi";
    }

    "top_level"
}

fn protocol_signal_severity(signal: &ProtocolInstructionSignal, high_impact: bool) -> &'static str {
    match signal.signal_kind {
        Some(ProtocolSignalKind::Unknown) => "critical",
        Some(ProtocolSignalKind::Admin) if high_impact => "high",
        _ => "standard",
    }
}

fn protocol_instruction_details(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    signal: &ProtocolInstructionSignal,
) -> ProtocolInstructionDetails {
    if signal.protocol.slug == KAMINO_LEND_SLUG
        && signal.instruction_name.as_deref() == Some("update_reserve_config")
    {
        return kamino_reserve_config_details(tx, instruction);
    }

    if signal.protocol.slug == ORCA_WHIRLPOOLS_SLUG {
        match signal.instruction_name.as_deref() {
            Some("set_config_extension_authority") => {
                return orca_config_extension_authority_details(
                    tx,
                    instruction,
                    signal.high_impact,
                    "new_config_extension_authority",
                    "New config extension authority",
                );
            }
            Some("set_token_badge_authority") => {
                return orca_config_extension_authority_details(
                    tx,
                    instruction,
                    signal.high_impact,
                    "new_token_badge_authority",
                    "New token badge authority",
                );
            }
            _ => {}
        }
    }

    ProtocolInstructionDetails::default()
}

fn kamino_reserve_config_details(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
) -> ProtocolInstructionDetails {
    let lending_market_owner = instruction_account(tx, instruction, 0).ok();
    let lending_market = instruction_account(tx, instruction, 1).ok();
    let reserve = instruction_account(tx, instruction, 2).ok();
    let parsed = parse_kamino_reserve_config_update(&instruction.data);

    let mut payload_fields = Vec::new();
    if let Some(lending_market_owner) = lending_market_owner {
        payload_fields.push(("lending_market_owner", lending_market_owner.to_string()));
    }
    if let Some(lending_market) = lending_market {
        payload_fields.push(("lending_market", lending_market.to_string()));
    }
    if let Some(reserve) = reserve {
        payload_fields.push(("reserve", reserve.to_string()));
    }
    if let Some(parsed) = parsed.as_ref() {
        payload_fields.push(("kamino_update_config_mode", parsed.mode_name.to_owned()));
        payload_fields.push(("kamino_update_config_mode_id", parsed.mode.to_string()));
        payload_fields.push((
            "kamino_update_config_value_len",
            parsed.value_len.to_string(),
        ));
        payload_fields.push((
            "kamino_update_config_skip_validation",
            parsed.skip_validation.to_string(),
        ));
    }

    ProtocolInstructionDetails {
        actor: lending_market_owner.map(|owner| owner.to_string()),
        high_impact_override: parsed
            .as_ref()
            .map(kamino_reserve_config_update_high_impact),
        payload_fields,
    }
}

fn parse_kamino_reserve_config_update(data: &[u8]) -> Option<KaminoReserveConfigUpdate> {
    if data.get(..8)? != anchor_discriminator("update_reserve_config").as_slice() {
        return None;
    }

    let mode = u64::from_le_bytes(data.get(8..16)?.try_into().ok()?);
    let value_len = u32::from_le_bytes(data.get(16..20)?.try_into().ok()?) as usize;
    let skip_validation_offset = 20_usize.checked_add(value_len)?;
    data.get(20..skip_validation_offset)?;
    let skip_validation = match data.get(skip_validation_offset)? {
        0 => false,
        1 => true,
        _ => return None,
    };

    Some(KaminoReserveConfigUpdate {
        mode,
        mode_name: kamino_update_config_mode_name(mode),
        value_len,
        skip_validation,
    })
}

fn kamino_update_config_mode_name(mode: u64) -> &'static str {
    match mode {
        1 => "update_loan_to_value_pct",
        2 => "update_max_liquidation_bonus_bps",
        3 => "update_liquidation_threshold_pct",
        4 => "update_protocol_liquidation_fee",
        5 => "update_protocol_take_rate",
        6 => "update_fees_origination_fee",
        7 => "update_fees_flash_loan_fee",
        8 => "deprecated_update_fees_referral_fee_bps",
        9 => "update_deposit_limit",
        10 => "update_borrow_limit",
        11 => "update_token_info_lower_heuristic",
        12 => "update_token_info_upper_heuristic",
        13 => "update_token_info_exp_heuristic",
        14 => "update_token_info_twap_divergence",
        15 => "update_token_info_scope_twap",
        16 => "update_token_info_scope_chain",
        17 => "update_token_info_name",
        18 => "update_token_info_price_max_age",
        19 => "update_token_info_twap_max_age",
        20 => "update_scope_price_feed",
        21 => "update_pyth_price",
        22 => "update_switchboard_feed",
        23 => "update_switchboard_twap_feed",
        24 => "update_borrow_rate_curve",
        25 => "update_entire_reserve_config",
        26 => "update_debt_withdrawal_cap",
        27 => "update_deposit_withdrawal_cap",
        28 => "deprecated_update_debt_withdrawal_cap_current_total",
        29 => "deprecated_update_deposit_withdrawal_cap_current_total",
        30 => "update_bad_debt_liquidation_bonus_bps",
        31 => "update_min_liquidation_bonus_bps",
        32 => "update_deleveraging_margin_call_period",
        33 => "update_borrow_factor",
        34 => "deprecated_update_asset_tier",
        35 => "update_elevation_group",
        36 => "update_deleveraging_threshold_decrease_bps_per_day",
        37 => "deprecated_update_multiplier_side_boost",
        38 => "deprecated_update_multiplier_tag_boost",
        39 => "update_reserve_status",
        40 => "update_farm_collateral",
        41 => "update_farm_debt",
        42 => "update_disable_usage_as_collateral_outside_emode",
        43 => "update_block_borrowing_above_utilization_pct",
        44 => "update_block_price_usage",
        45 => "update_borrow_limit_outside_elevation_group",
        46 => "update_borrow_limits_in_elevation_group_against_this_reserve",
        47 => "update_host_fixed_interest_rate_bps",
        48 => "update_autodeleverage_enabled",
        49 => "update_deleveraging_bonus_increase_bps_per_day",
        50 => "update_protocol_order_execution_fee",
        51 => "update_proposer_authority_lock",
        52 => "update_min_deleveraging_bonus_bps",
        53 => "update_block_c_token_usage",
        54 => "update_debt_maturity_timestamp",
        55 => "update_debt_term_seconds",
        _ => "unknown",
    }
}

fn kamino_reserve_config_update_high_impact(update: &KaminoReserveConfigUpdate) -> bool {
    update.skip_validation
        || update.mode_name == "unknown"
        || matches!(
            update.mode,
            1..=3
                | 4..=7
                | 11..=16
                | 18..=25
                | 30..=36
                | 39
                | 42..=44
                | 47
                | 48
                | 49
                | 50
                | 51..=55
        )
}

fn orca_config_extension_authority_details(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    high_impact: bool,
    specific_new_authority_key: &'static str,
    specific_new_authority_label: &'static str,
) -> ProtocolInstructionDetails {
    let Ok(whirlpools_config) = instruction_account(tx, instruction, 0) else {
        return ProtocolInstructionDetails::default();
    };
    let Ok(whirlpools_config_extension) = instruction_account(tx, instruction, 1) else {
        return ProtocolInstructionDetails::default();
    };
    let Ok(config_extension_authority) = instruction_account(tx, instruction, 2) else {
        return ProtocolInstructionDetails::default();
    };
    let Ok(new_authority) = instruction_account(tx, instruction, 3) else {
        return ProtocolInstructionDetails::default();
    };

    let official_config =
        is_official_orca_whirlpools_config_pair(whirlpools_config, whirlpools_config_extension);
    let surface_scope = if official_config {
        "official_orca_whirlpools_config"
    } else {
        "third_party_whirlpools_config"
    };
    let new_authority = new_authority.to_string();

    ProtocolInstructionDetails {
        actor: Some(config_extension_authority.to_string()),
        high_impact_override: Some(high_impact && official_config),
        payload_fields: vec![
            ("whirlpools_config", whirlpools_config.to_string()),
            (
                "whirlpools_config_extension",
                whirlpools_config_extension.to_string(),
            ),
            (
                "config_extension_authority",
                config_extension_authority.to_string(),
            ),
            ("authority", config_extension_authority.to_string()),
            ("authority_label", "Config extension authority".to_owned()),
            (specific_new_authority_key, new_authority.clone()),
            ("new_authority", new_authority),
            (
                "new_authority_label",
                specific_new_authority_label.to_owned(),
            ),
            ("subject_label", "Whirlpools config extension".to_owned()),
            ("subject_address", whirlpools_config_extension.to_string()),
            ("surface_scope", surface_scope.to_owned()),
        ],
    }
}

fn is_official_orca_whirlpools_config_pair(
    whirlpools_config: Pubkey,
    whirlpools_config_extension: Pubkey,
) -> bool {
    ORCA_OFFICIAL_WHIRLPOOLS_CONFIGS
        .iter()
        .any(|(expected_config, expected_extension)| {
            let Ok(expected_config) = Pubkey::from_str(expected_config) else {
                return false;
            };
            let Ok(expected_extension) = Pubkey::from_str(expected_extension) else {
                return false;
            };

            whirlpools_config == expected_config
                && whirlpools_config_extension == expected_extension
        })
}

async fn annotate_multisig_signer_patterns(
    state: &SharedState,
    event: &mut ChainEvent,
) -> Result<()> {
    if event.target_type != target_type::PROGRAM {
        return Ok(());
    }

    let multisig = event
        .payload
        .get("multisig")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let member = event
        .payload
        .get("member")
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .or_else(|| event.actor.clone());

    let (Some(multisig), Some(member)) = (multisig, member) else {
        return Ok(());
    };

    let prior_member_actions = state
        .db
        .prior_program_multisig_member_actions(&event.target_address, &multisig, &member)
        .await?;
    let prior_total_actions = state
        .db
        .prior_program_multisig_total_actions(&event.target_address, &multisig)
        .await?;
    let top_signer = state
        .db
        .top_program_multisig_signer(&event.target_address, &multisig)
        .await?;

    if let Some(payload) = event.payload.as_object_mut() {
        payload.insert(
            "program_multisig_prior_member_actions".to_owned(),
            json!(prior_member_actions),
        );
        payload.insert(
            "program_multisig_prior_total_actions".to_owned(),
            json!(prior_total_actions),
        );

        if let Some((top_member, top_count)) = top_signer.as_ref() {
            payload.insert("program_multisig_top_signer".to_owned(), json!(top_member));
            payload.insert(
                "program_multisig_top_signer_actions".to_owned(),
                json!(top_count),
            );
        }
    }

    if prior_total_actions >= 5 && prior_member_actions == 0 {
        add_tags(
            event,
            &[
                "signal:new_multisig_signer",
                "severity:critical",
                "impact:high",
                "alert:review_now",
            ],
        );
        event.summary = format!("New signer review: {}", event.summary);
        return Ok(());
    }

    if let Some((top_member, top_count)) = top_signer {
        let signer_is_not_top = top_member != member;
        let signer_is_rare = prior_member_actions <= 1;
        let strong_baseline = prior_total_actions >= 10 && top_count >= 4;

        if signer_is_not_top && signer_is_rare && strong_baseline {
            add_tags(
                event,
                &[
                    "signal:unusual_multisig_signer",
                    "severity:high",
                    "alert:review_now",
                ],
            );
            if let Some(payload) = event.payload.as_object_mut() {
                payload.insert(
                    "unusual_signer_reason".to_owned(),
                    json!("Signer is rare for this program / multisig pair"),
                );
            }
            event.summary = format!("Unusual signer review: {}", event.summary);
        }
    }

    Ok(())
}

async fn annotate_multisig_project_links(
    state: &SharedState,
    event: &mut ChainEvent,
) -> Result<()> {
    if event.target_type != target_type::MULTISIG {
        return Ok(());
    }

    let mut links = HashMap::<String, String>::new();

    if let Some(decoded_protocol) = event
        .payload
        .get("decoded_protocol")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "squads")
    {
        links
            .entry(decoded_protocol.to_owned())
            .or_insert_with(|| "decoded_execution".to_owned());
    }

    let mut lookup_addresses = Vec::<String>::new();
    push_unique_address(&mut lookup_addresses, &event.target_address);
    for payload_key in ["multisig", "member", "decoded_program"] {
        if let Some(address) = event
            .payload
            .get(payload_key)
            .and_then(|value| value.as_str())
        {
            push_unique_address(&mut lookup_addresses, address);
        }
    }
    if let Some(actor) = event.actor.as_deref() {
        push_unique_address(&mut lookup_addresses, actor);
    }

    for address in lookup_addresses {
        for link in state
            .db
            .address_protocol_links(&address, 40)
            .await
            .with_context(|| format!("load protocol links for {address}"))?
        {
            if link.protocol_slug == "squads" {
                continue;
            }
            links.entry(link.protocol_slug).or_insert(link.evidence);
        }
    }

    if links.is_empty() {
        return Ok(());
    }

    let mut linked_protocols = links.into_iter().collect::<Vec<_>>();
    linked_protocols.sort_by(|(left, _), (right, _)| left.cmp(right));

    event.tags.extend([
        "sector:defi".to_owned(),
        "watch:defi".to_owned(),
        "multisig:project_linked".to_owned(),
    ]);
    event.tags.extend(
        linked_protocols
            .iter()
            .map(|(slug, _)| format!("protocol:{slug}")),
    );
    event.tags = normalize_tags(std::mem::take(&mut event.tags));

    if let Some(payload) = event.payload.as_object_mut() {
        let payload_links = linked_protocols
            .into_iter()
            .map(|(slug, evidence)| {
                let protocol_name = protocols::curated_protocol_descriptor(&slug)
                    .map(|descriptor| descriptor.name)
                    .unwrap_or_else(|| slug.clone());
                json!({
                    "protocol": slug,
                    "protocol_name": protocol_name,
                    "evidence": evidence,
                })
            })
            .collect::<Vec<_>>();
        payload.insert("linked_protocols".to_owned(), json!(payload_links));
    }

    Ok(())
}

fn push_unique_address(addresses: &mut Vec<String>, address: &str) {
    let address = address.trim();
    if address.is_empty() || addresses.iter().any(|candidate| candidate == address) {
        return;
    }
    addresses.push(address.to_owned());
}

fn add_tags(event: &mut ChainEvent, extra_tags: &[&str]) {
    event
        .tags
        .extend(extra_tags.iter().map(|tag| (*tag).to_owned()));
    event.tags = normalize_tags(std::mem::take(&mut event.tags));
}

fn build_event(
    tx: &ResolvedTransaction,
    kind: &str,
    target_type: &str,
    target_address: String,
    actor: Option<String>,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    summary: String,
    tags: Vec<String>,
    payload: serde_json::Value,
) -> ChainEvent {
    let indexed_at = now_ts();
    ChainEvent {
        id: format!(
            "{}:{instruction_path}:{kind}:{target_address}",
            tx.signature
        ),
        signature: tx.signature.clone(),
        instruction_path: instruction_path.to_owned(),
        kind: kind.to_owned(),
        target_type: target_type.to_owned(),
        target_address,
        actor,
        slot: slot as i64,
        block_time,
        indexed_at,
        summary,
        tags: normalize_tags(tags),
        payload,
        delivery_attempts: 0,
        delivered_at: None,
        last_error: None,
        next_retry_at: indexed_at,
    }
}

fn build_program_data_write(
    tx: &ResolvedTransaction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    loader: String,
    write_account: Pubkey,
    authority: Option<Pubkey>,
    offset: u32,
    bytes: Vec<u8>,
) -> Result<ProgramDataWrite> {
    let indexed_at = now_ts();
    let bytes_sha256 = sha256_hex(&bytes);
    Ok(ProgramDataWrite {
        id: format!(
            "{}:{instruction_path}:program_data_write:{write_account}:{offset}",
            tx.signature
        ),
        signature: tx.signature.clone(),
        instruction_path: instruction_path.to_owned(),
        instruction_order: instruction_order_key(instruction_path),
        loader,
        write_account: write_account.to_string(),
        authority: authority.map(|value| value.to_string()),
        slot: slot as i64,
        block_time,
        indexed_at,
        offset: i64::from(offset),
        bytes,
        bytes_sha256,
    })
}

fn build_program_idl_data_write(
    tx: &ResolvedTransaction,
    slot: u64,
    block_time: Option<i64>,
    instruction_path: &str,
    origin: String,
    metadata_account: Pubkey,
    source_account: Pubkey,
    program_address: Pubkey,
    authority: Option<&str>,
    offset: Option<i64>,
    bytes: Vec<u8>,
) -> Result<ProgramIdlDataWrite> {
    let indexed_at = now_ts();
    let bytes_sha256 = sha256_hex(&bytes);
    let offset_label = offset
        .map(|value| value.to_string())
        .unwrap_or_else(|| "append".to_owned());
    Ok(ProgramIdlDataWrite {
        id: format!(
            "{}:{instruction_path}:program_idl_write:{source_account}:{offset_label}",
            tx.signature
        ),
        signature: tx.signature.clone(),
        instruction_path: instruction_path.to_owned(),
        instruction_order: instruction_order_key(instruction_path),
        origin,
        metadata_account: metadata_account.to_string(),
        source_account: source_account.to_string(),
        program_address: program_address.to_string(),
        authority: authority.map(str::to_owned),
        slot: slot as i64,
        block_time,
        indexed_at,
        offset,
        bytes,
        bytes_sha256,
    })
}

pub(crate) fn instruction_account(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    account_position: usize,
) -> Result<Pubkey> {
    let account_index = instruction
        .accounts
        .get(account_position)
        .ok_or_else(|| anyhow!("instruction missing account position {account_position}"))?;
    tx.account_keys
        .get(*account_index)
        .copied()
        .ok_or_else(|| anyhow!("instruction referenced missing account index {account_index}"))
}

fn program_id(tx: &ResolvedTransaction, instruction: &ResolvedInstruction) -> Option<Pubkey> {
    tx.account_keys.get(instruction.program_id_index).copied()
}

fn parse_pubkey(bytes: &[u8]) -> Result<Pubkey> {
    Pubkey::try_from(bytes).map_err(|error| anyhow!("invalid pubkey: {error}"))
}

fn squads_program_id() -> Pubkey {
    static PROGRAM_ID: OnceLock<Pubkey> = OnceLock::new();
    *PROGRAM_ID.get_or_init(|| {
        Pubkey::from_str(protocols::SQUADS_V4_PROGRAM_ID).expect("valid squads program id")
    })
}

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}").as_bytes());
    let hash = hasher.finalize();
    let mut discriminator = [0_u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

fn trim_slot_times(slot_times: &mut BTreeMap<u64, i64>) {
    while slot_times.len() > 4_096 {
        if let Some(oldest_slot) = slot_times.keys().next().copied() {
            slot_times.remove(&oldest_slot);
        } else {
            break;
        }
    }
}

fn summary_for_program(
    prefix: &str,
    program: &Pubkey,
    parent_context: Option<&ParentInstructionContext>,
) -> String {
    if let Some(context) = parent_context.and_then(|context| context.squads_execution.as_ref()) {
        return format!(
            "{prefix} for {program} via Squads multisig {}",
            context.multisig
        );
    }

    match parent_context.and_then(|context| context.parent_program) {
        Some(parent) => format!("{prefix} for {program} via {parent}"),
        None => format!("{prefix} for {program}"),
    }
}

fn is_program_update_kind(kind: &str) -> bool {
    matches!(
        kind,
        event_kind::PROGRAM_DEPLOYED
            | event_kind::PROGRAM_UPGRADED
            | event_kind::PROGRAM_FINALIZED
            | event_kind::PROGRAM_MIGRATED_TO_LOADER_V4
            | event_kind::PROGRAM_IDL_INITIALIZED
            | event_kind::PROGRAM_IDL_UPDATED
            | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
            | event_kind::PROGRAM_IDL_IMMUTABLE
            | event_kind::PROGRAM_IDL_RESIZED
            | event_kind::PROGRAM_IDL_CLOSED
    )
}

fn normalize_tags(mut tags: Vec<String>) -> Vec<String> {
    tags.sort();
    tags.dedup();
    tags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::TelegramEventScope, name_service::DomainInstructionEvent,
        protocols::DefiProtocolMatch,
    };

    fn sample_signal(signal_kind: ProtocolSignalKind) -> ProtocolInstructionSignal {
        ProtocolInstructionSignal {
            protocol: DefiProtocolMatch {
                slug: "meteora_dlmm".to_owned(),
                name: "Meteora DLMM".to_owned(),
                program_id: Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo")
                    .unwrap(),
                program_label: "DLMM".to_owned(),
            },
            instruction_name: None,
            signal_kind: Some(signal_kind),
            high_impact: true,
            discriminator_hex: "e445a52e51cb9a1d".to_owned(),
        }
    }

    fn sample_domain_context() -> (ResolvedTransaction, ResolvedInstruction) {
        (
            ResolvedTransaction {
                signature: "sns-test-signature".to_owned(),
                primary_signer: None,
                account_keys: vec![],
                top_level_instructions: vec![],
                inner_instruction_sets: vec![],
                log_messages: vec![],
            },
            ResolvedInstruction {
                program_id_index: 0,
                accounts: vec![],
                data: vec![],
                stack_height: Some(1),
            },
        )
    }

    fn sample_orca_config_extension_authority_event(
        whirlpools_config: Pubkey,
        whirlpools_config_extension: Pubkey,
    ) -> ChainEvent {
        let new_authority =
            Pubkey::from_str("WzMaL78srutrF6CsxEkWuhMaDF5HZA6jNRaEPengqpb").unwrap();
        let config_extension_authority =
            Pubkey::from_str("99VXriv7RXJSypeJDBQtGRsak1n5o2NBzbtMXhHW2RNG").unwrap();
        let orca_program = Pubkey::from_str("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc").unwrap();
        let tx = ResolvedTransaction {
            signature: "orca-config-extension-test".to_owned(),
            primary_signer: Some(new_authority),
            account_keys: vec![
                new_authority,
                config_extension_authority,
                whirlpools_config,
                whirlpools_config_extension,
                orca_program,
            ],
            top_level_instructions: vec![],
            inner_instruction_sets: vec![],
            log_messages: vec![],
        };
        let instruction = ResolvedInstruction {
            program_id_index: 4,
            accounts: vec![2, 3, 1, 0],
            data: anchor_discriminator("set_config_extension_authority").to_vec(),
            stack_height: Some(1),
        };

        parse_protocol_instruction_event(&tx, &instruction, 421137051, Some(1779340560), "3", None)
            .unwrap()
            .0
            .expect("orca config extension authority event")
    }

    fn sample_kamino_reserve_config_event(mode: u64, skip_validation: bool) -> ChainEvent {
        let lending_market_owner = Pubkey::new_unique();
        let lending_market = Pubkey::new_unique();
        let reserve = Pubkey::new_unique();
        let kamino_program =
            Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let tx = ResolvedTransaction {
            signature: "kamino-reserve-config-test".to_owned(),
            primary_signer: Some(lending_market_owner),
            account_keys: vec![
                lending_market_owner,
                lending_market,
                reserve,
                kamino_program,
            ],
            top_level_instructions: vec![],
            inner_instruction_sets: vec![],
            log_messages: vec![],
        };
        let instruction = ResolvedInstruction {
            program_id_index: 3,
            accounts: vec![0, 1, 2],
            data: kamino_reserve_config_update_data(mode, &[], skip_validation),
            stack_height: Some(1),
        };

        parse_protocol_instruction_event(&tx, &instruction, 421137051, Some(1779340560), "4", None)
            .unwrap()
            .0
            .expect("kamino reserve config event")
    }

    fn kamino_reserve_config_update_data(
        mode: u64,
        value: &[u8],
        skip_validation: bool,
    ) -> Vec<u8> {
        let mut data = anchor_discriminator("update_reserve_config").to_vec();
        data.extend_from_slice(&mode.to_le_bytes());
        data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        data.extend_from_slice(value);
        data.push(u8::from(skip_validation));
        data
    }

    #[test]
    fn captures_upgradeable_loader_write_bytes() {
        let buffer = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let tx = ResolvedTransaction {
            signature: "write-test-signature".to_owned(),
            primary_signer: Some(authority),
            account_keys: vec![buffer, authority, bpf_loader_upgradeable::ID],
            top_level_instructions: vec![],
            inner_instruction_sets: vec![],
            log_messages: vec![],
        };
        let instruction = ResolvedInstruction {
            program_id_index: 2,
            accounts: vec![0, 1],
            data: bincode::serialize(&UpgradeableLoaderInstruction::Write {
                offset: 4,
                bytes: b"ELF".to_vec(),
            })
            .unwrap(),
            stack_height: Some(1),
        };

        let (event, write) =
            parse_upgradeable_loader_artifacts(&tx, &instruction, 12, Some(34), "0", None).unwrap();
        let write = write.expect("program data write");

        assert!(event.is_none());
        assert_eq!(write.write_account, buffer.to_string());
        assert_eq!(write.authority, Some(authority.to_string()));
        assert_eq!(write.offset, 4);
        assert_eq!(write.bytes, b"ELF");
        assert_eq!(write.instruction_order, "00000000");
    }

    #[test]
    fn suppresses_unknown_self_cpi_protocol_signal() {
        let target_program =
            Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let parent_context = ParentInstructionContext {
            parent_program: Some(
                Pubkey::from_str("9i7QdbBHx7fjLAAaFfcDRgkdS4oNfKv8duzyE27a1kJ6").unwrap(),
            ),
            immediate_parent_program: Some(target_program),
            squads_execution: None,
        };

        assert!(!should_emit_protocol_signal(
            &sample_signal(ProtocolSignalKind::Unknown),
            target_program,
            Some(&parent_context),
        ));
    }

    #[test]
    fn demotes_orca_config_extension_authority_for_third_party_config() {
        let event = sample_orca_config_extension_authority_event(
            Pubkey::from_str("4HiRwcQ8vo3BSn1jna9btRnjg5r5Ltwaiyd7caoWCw1E").unwrap(),
            Pubkey::from_str("JC5GWccEp9MBGTTpH9hdhmaazgs8QsrJD7NjCHFiPPcw").unwrap(),
        );

        assert_eq!(event.kind, event_kind::PROGRAM_ADMIN_INSTRUCTION);
        assert_eq!(
            event.actor.as_deref(),
            Some("99VXriv7RXJSypeJDBQtGRsak1n5o2NBzbtMXhHW2RNG")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|tag| tag == "protocol:orca_whirlpools")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|tag| tag == "instruction:set_config_extension_authority")
        );
        assert!(!event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(!event.tags.iter().any(|tag| tag == "impact:high"));
        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
        assert_eq!(
            event
                .payload
                .get("surface_scope")
                .and_then(|value| value.as_str()),
            Some("third_party_whirlpools_config")
        );
        assert_eq!(
            event
                .payload
                .get("new_config_extension_authority")
                .and_then(|value| value.as_str()),
            Some("WzMaL78srutrF6CsxEkWuhMaDF5HZA6jNRaEPengqpb")
        );
    }

    #[test]
    fn keeps_orca_config_extension_authority_high_for_official_config() {
        let event = sample_orca_config_extension_authority_event(
            Pubkey::from_str("2LecshUwdy9xi7meFgHtFJQNSKk4KdTrcpvaB56dP2NQ").unwrap(),
            Pubkey::from_str("777H5H3Tp9U11uRVRzFwM8BinfiakbaLT8vQpeuhvEiH").unwrap(),
        );

        assert!(event.summary.starts_with("High-impact "));
        assert!(event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(event.tags.iter().any(|tag| tag == "impact:high"));
        assert!(TelegramEventScope::DefiWatch.matches_event(&event));
        assert_eq!(
            event
                .payload
                .get("surface_scope")
                .and_then(|value| value.as_str()),
            Some("official_orca_whirlpools_config")
        );
        assert_eq!(
            event
                .payload
                .get("whirlpools_config_extension")
                .and_then(|value| value.as_str()),
            Some("777H5H3Tp9U11uRVRzFwM8BinfiakbaLT8vQpeuhvEiH")
        );
    }

    #[test]
    fn keeps_kamino_borrow_limit_update_as_standard_admin() {
        let event = sample_kamino_reserve_config_event(10, false);

        assert_eq!(event.kind, event_kind::PROGRAM_ADMIN_INSTRUCTION);
        assert!(
            event
                .tags
                .iter()
                .any(|tag| tag == "instruction:update_reserve_config")
        );
        assert!(!event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(!event.tags.iter().any(|tag| tag == "impact:high"));
        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
        assert_eq!(
            event
                .payload
                .get("kamino_update_config_mode")
                .and_then(|value| value.as_str()),
            Some("update_borrow_limit")
        );
    }

    #[test]
    fn escalates_kamino_liquidation_threshold_update() {
        let event = sample_kamino_reserve_config_event(3, false);

        assert!(event.summary.starts_with("High-impact "));
        assert!(event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(event.tags.iter().any(|tag| tag == "impact:high"));
        assert!(TelegramEventScope::DefiWatch.matches_event(&event));
        assert_eq!(
            event
                .payload
                .get("kamino_update_config_mode")
                .and_then(|value| value.as_str()),
            Some("update_liquidation_threshold_pct")
        );
    }

    #[test]
    fn escalates_kamino_fee_updates_without_direction_history() {
        let event = sample_kamino_reserve_config_event(5, false);

        assert!(event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(event.tags.iter().any(|tag| tag == "impact:high"));
        assert_eq!(
            event
                .payload
                .get("kamino_update_config_mode")
                .and_then(|value| value.as_str()),
            Some("update_protocol_take_rate")
        );
    }

    #[test]
    fn escalates_kamino_reserve_config_skip_validation() {
        let event = sample_kamino_reserve_config_event(10, true);

        assert!(event.tags.iter().any(|tag| tag == "severity:high"));
        assert!(event.tags.iter().any(|tag| tag == "impact:high"));
        assert_eq!(
            event
                .payload
                .get("kamino_update_config_skip_validation")
                .and_then(|value| value.as_str()),
            Some("true")
        );
    }

    #[test]
    fn keeps_admin_self_cpi_protocol_signal() {
        let target_program =
            Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let parent_context = ParentInstructionContext {
            parent_program: Some(
                Pubkey::from_str("9i7QdbBHx7fjLAAaFfcDRgkdS4oNfKv8duzyE27a1kJ6").unwrap(),
            ),
            immediate_parent_program: Some(target_program),
            squads_execution: None,
        };

        assert!(should_emit_protocol_signal(
            &sample_signal(ProtocolSignalKind::Admin),
            target_program,
            Some(&parent_context),
        ));
    }

    #[test]
    fn plain_domain_update_is_not_critical_without_links() {
        let (tx, instruction) = sample_domain_context();
        let domain_event = DomainInstructionEvent {
            target_account: Pubkey::new_unique(),
            target_name: "random.sol".to_owned(),
            owner: Pubkey::new_unique().to_string(),
            class: Pubkey::default().to_string(),
            parent: None,
            root_domain: true,
            kind: DomainInstructionKind::Update {
                offset: 0,
                data_len: 32,
            },
        };

        let (_, _, tags, payload) = build_name_service_event_parts(
            &domain_event,
            &tx,
            &instruction,
            &DomainRiskContext::default(),
        )
        .unwrap();

        assert!(!tags.iter().any(|tag| tag == "severity:critical"));
        assert!(!tags.iter().any(|tag| tag == "alert:review_now"));
        assert_eq!(
            payload
                .get("critical_domain")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn allowlisted_domain_transfer_is_critical() {
        let (tx, instruction) = sample_domain_context();
        let domain_event = DomainInstructionEvent {
            target_account: Pubkey::new_unique(),
            target_name: "ferno.sol".to_owned(),
            owner: Pubkey::new_unique().to_string(),
            class: Pubkey::default().to_string(),
            parent: None,
            root_domain: true,
            kind: DomainInstructionKind::Transfer {
                new_owner: Pubkey::new_unique().to_string(),
            },
        };

        let (_, _, tags, payload) = build_name_service_event_parts(
            &domain_event,
            &tx,
            &instruction,
            &DomainRiskContext {
                explicit_watch: true,
                project_links: Vec::new(),
            },
        )
        .unwrap();

        assert!(tags.iter().any(|tag| tag == "severity:critical"));
        assert!(tags.iter().any(|tag| tag == "alert:review_now"));
        assert_eq!(
            payload
                .get("critical_domain")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn ferno_domain_event_carries_protocol_tag() {
        let (tx, instruction) = sample_domain_context();
        let domain_event = DomainInstructionEvent {
            target_account: Pubkey::new_unique(),
            target_name: "ferno.sol".to_owned(),
            owner: Pubkey::new_unique().to_string(),
            class: Pubkey::default().to_string(),
            parent: None,
            root_domain: true,
            kind: DomainInstructionKind::Update {
                offset: 0,
                data_len: 32,
            },
        };

        let (_, _, tags, payload) = build_name_service_event_parts(
            &domain_event,
            &tx,
            &instruction,
            &DomainRiskContext {
                explicit_watch: true,
                project_links: vec![DomainProjectLink {
                    protocol_slug: "ferno".to_owned(),
                    protocol_name: "Ferno".to_owned(),
                    evidence: "tracked .sol domain".to_owned(),
                }],
            },
        )
        .unwrap();

        assert!(tags.iter().any(|tag| tag == "protocol:ferno"));
        assert!(tags.iter().any(|tag| tag == "domain:project_linked"));
        assert!(tags.iter().any(|tag| tag == "watch:defi"));
        assert!(
            payload
                .get("linked_protocols")
                .and_then(|value| value.as_array())
                .is_some_and(|items| !items.is_empty())
        );
    }

    #[test]
    fn squads_config_execution_stays_out_of_default_defi_watch() {
        let multisig = Pubkey::new_unique();
        let member = Pubkey::new_unique();
        let proposal = Pubkey::new_unique();
        let transaction = Pubkey::new_unique();
        let squads_program = squads_program_id();
        let tx = ResolvedTransaction {
            signature: "squads-config-test".to_owned(),
            primary_signer: Some(member),
            account_keys: vec![multisig, member, proposal, transaction, squads_program],
            top_level_instructions: vec![],
            inner_instruction_sets: vec![],
            log_messages: vec![],
        };
        let instruction = ResolvedInstruction {
            program_id_index: 4,
            accounts: vec![0, 1, 2, 3],
            data: anchor_discriminator("config_transaction_execute").to_vec(),
            stack_height: Some(1),
        };

        let event = parse_squads_event(&tx, &instruction, 1, None, "0")
            .unwrap()
            .expect("squads config event");

        assert_eq!(event.kind, event_kind::MULTISIG_CONFIG_EXECUTED);
        assert!(event.tags.iter().any(|tag| tag == "protocol:squads"));
        assert!(
            event
                .tags
                .iter()
                .any(|tag| tag == "signal:multisig_config_change")
        );
        assert!(
            event
                .tags
                .iter()
                .any(|tag| tag == "instruction:config_transaction_execute")
        );
        assert!(!event.tags.iter().any(|tag| tag == "watch:defi"));
        assert!(!event.tags.iter().any(|tag| tag == "sector:defi"));
        assert!(!event.tags.iter().any(|tag| tag == "impact:high"));
        assert!(!event.tags.iter().any(|tag| tag == "severity:critical"));
        assert!(!event.tags.iter().any(|tag| tag == "alert:review_now"));
        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
    }

    #[test]
    fn derives_dlmm_as_immediate_parent_for_meteora_nested_helper_call() {
        let jupiter = Pubkey::from_str("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4").unwrap();
        let shrimp = Pubkey::from_str("9H6tua7jkLhdm3w8BvgpTn5LZNU7g4ZynDmCiNN3q6Rp").unwrap();
        let alpha = Pubkey::from_str("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA").unwrap();
        let dlmm = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let spl_token = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA").unwrap();

        let tx = ResolvedTransaction {
            signature: "5LGVojs8VvpCj9chAUQZVrXM1TWWXV5aYzWojVgjC2reUhGTkVjy7CXRi8tYmD8iPPLQg8Hy7sg6HqwdJRLsAfk7"
                .to_owned(),
            primary_signer: None,
            account_keys: vec![jupiter, shrimp, alpha, dlmm, spl_token],
            top_level_instructions: vec![ResolvedInstruction {
                program_id_index: 0,
                accounts: vec![],
                data: vec![],
                stack_height: Some(1),
            }],
            inner_instruction_sets: vec![],
            log_messages: vec![],
        };

        let top_level_jupiter_route = &tx.top_level_instructions[0];
        let inner_instructions = vec![
            ResolvedInstruction {
                program_id_index: 1,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 0,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            ResolvedInstruction {
                program_id_index: 2,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 0,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            ResolvedInstruction {
                program_id_index: 3,
                accounts: vec![],
                data: vec![],
                stack_height: Some(2),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 4,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
            ResolvedInstruction {
                program_id_index: 3,
                accounts: vec![],
                data: vec![],
                stack_height: Some(3),
            },
        ];

        let parent_contexts = inner_parent_contexts_for_instruction_set(
            &tx,
            top_level_jupiter_route,
            &inner_instructions,
        )
        .unwrap();

        assert_eq!(parent_contexts[11].parent_program, Some(jupiter));
        assert_eq!(parent_contexts[11].immediate_parent_program, Some(dlmm));
        assert!(!should_emit_protocol_signal(
            &sample_signal(ProtocolSignalKind::Unknown),
            dlmm,
            Some(&parent_contexts[11]),
        ));
    }
}
