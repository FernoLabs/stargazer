use crate::{
    anchor_idl,
    context::SharedState,
    name_service, program_metadata, protocols,
    solana::{
        IngestOptions, ResolvedInnerInstructionSet, ResolvedInstruction, ResolvedTransaction,
        decode_transaction, ingest_resolved_transaction,
    },
};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures::{SinkExt, StreamExt, stream};
use serde::Deserialize;
use serde_json::{Value, json};
#[allow(deprecated)]
use solana_sdk::{
    bpf_loader, bpf_loader_deprecated, bpf_loader_upgradeable, loader_v4, pubkey::Pubkey,
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env,
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::debug;
use yellowstone_grpc_client::{ClientTlsConfig, GeyserGrpcClient};
use yellowstone_grpc_proto::{
    prelude::{
        CommitmentLevel, CompiledInstruction as ProtoCompiledInstruction,
        ConfirmedBlock as ProtoConfirmedBlock, InnerInstruction as ProtoInnerInstruction,
        SubscribeRequest, SubscribeRequestFilterBlocksMeta, SubscribeRequestFilterTransactions,
        SubscribeRequestPing, Transaction as ProtoTransaction,
        TransactionStatusMeta as ProtoTransactionStatusMeta, subscribe_update::UpdateOneof,
    },
    prost::Message as ProstMessage,
};

const GRPC_BACKFILL_FILTER_LABEL: &str = "firewatch-backfill";
const GRPC_BACKFILL_BLOCK_META_FILTER_LABEL: &str = "firewatch-block-meta-backfill";
const PROGRAM_HISTORY_SIGNATURE_PAGE_LIMIT: usize = 1_000;
const PROGRAM_HISTORY_MAX_SIGNATURE_PAGES: usize = 16;
const UPGRADEABLE_LOADER_PROGRAM_TAG: u32 = 2;

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockBackfillReport {
    pub(crate) slot: u64,
    pub(crate) url: String,
    pub(crate) transaction_count: usize,
    pub(crate) skipped_unwatched_transaction_count: usize,
    pub(crate) skipped_failed_transaction_count: usize,
    pub(crate) skipped_missing_meta_count: usize,
    pub(crate) matched_transaction_count: usize,
    pub(crate) parsed_event_count: usize,
    pub(crate) inserted_event_count: usize,
    pub(crate) parsed_metric_observation_count: usize,
    pub(crate) inserted_metric_observation_count: usize,
    pub(crate) parsed_liquidity_pool_observation_count: usize,
    pub(crate) inserted_liquidity_pool_count: usize,
    pub(crate) program_data_write_count: usize,
    pub(crate) program_idl_data_write_count: usize,
    pub(crate) program_version_update_count: usize,
    pub(crate) program_idl_version_update_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct BlockBackfillOutcome {
    pub(crate) slot: u64,
    pub(crate) attempts: usize,
    pub(crate) elapsed_ms: u128,
    pub(crate) report: Option<BlockBackfillReport>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BlockBackfillRangeReport {
    pub(crate) start_slot: u64,
    pub(crate) end_slot: u64,
    pub(crate) concurrency: usize,
    pub(crate) retries: usize,
    pub(crate) elapsed_ms: u128,
    pub(crate) outcomes: Vec<BlockBackfillOutcome>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GrpcBackfillReport {
    pub(crate) start_slot: u64,
    pub(crate) end_slot: u64,
    pub(crate) elapsed_ms: u128,
    pub(crate) observed_block_count: usize,
    pub(crate) transaction_count: usize,
    pub(crate) matched_transaction_count: usize,
    pub(crate) parsed_event_count: usize,
    pub(crate) inserted_event_count: usize,
    pub(crate) parsed_metric_observation_count: usize,
    pub(crate) inserted_metric_observation_count: usize,
    pub(crate) parsed_liquidity_pool_observation_count: usize,
    pub(crate) inserted_liquidity_pool_count: usize,
    pub(crate) program_data_write_count: usize,
    pub(crate) program_idl_data_write_count: usize,
    pub(crate) program_version_update_count: usize,
    pub(crate) program_idl_version_update_count: usize,
    pub(crate) last_observed_slot: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct ProgramUpgradeSignature {
    pub(crate) signature: String,
    pub(crate) slot: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct ProgramUpgradeBackfillWindow {
    pub(crate) start_slot: u64,
    pub(crate) end_slot: u64,
    pub(crate) report: BlockBackfillRangeReport,
}

#[derive(Clone, Debug)]
pub(crate) struct ProgramUpgradeHistoryBackfillReport {
    pub(crate) program_address: String,
    pub(crate) program_data_account: String,
    pub(crate) signatures: Vec<ProgramUpgradeSignature>,
    pub(crate) windows: Vec<ProgramUpgradeBackfillWindow>,
    pub(crate) elapsed_ms: u128,
}

impl BlockBackfillRangeReport {
    pub(crate) fn ok_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.report.is_some())
            .count()
    }

    pub(crate) fn error_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.error.is_some())
            .count()
    }

    pub(crate) fn transaction_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.transaction_count)
            .sum()
    }

    pub(crate) fn matched_transaction_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.matched_transaction_count)
            .sum()
    }

    pub(crate) fn parsed_event_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.parsed_event_count)
            .sum()
    }

    pub(crate) fn inserted_event_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.inserted_event_count)
            .sum()
    }

    pub(crate) fn parsed_metric_observation_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.parsed_metric_observation_count)
            .sum()
    }

    pub(crate) fn inserted_metric_observation_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.inserted_metric_observation_count)
            .sum()
    }

    pub(crate) fn parsed_liquidity_pool_observation_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.parsed_liquidity_pool_observation_count)
            .sum()
    }

    pub(crate) fn inserted_liquidity_pool_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.inserted_liquidity_pool_count)
            .sum()
    }

    pub(crate) fn program_data_write_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.program_data_write_count)
            .sum()
    }

    pub(crate) fn program_version_update_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.program_version_update_count)
            .sum()
    }

    pub(crate) fn program_idl_data_write_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.program_idl_data_write_count)
            .sum()
    }

    pub(crate) fn program_idl_version_update_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter_map(|outcome| outcome.report.as_ref())
            .map(|report| report.program_idl_version_update_count)
            .sum()
    }
}

pub(crate) async fn backfill_grpc_transactions(
    state: SharedState,
    start_slot: u64,
    end_slot: u64,
    enqueue_deliveries: bool,
) -> Result<GrpcBackfillReport> {
    let started = Instant::now();
    let mut builder =
        GeyserGrpcClient::build_from_shared(state.config.solana_grpc_endpoint.clone())?
            .x_token(state.config.solana_grpc_x_token.clone())?;

    if state.config.solana_grpc_endpoint.starts_with("https://") {
        builder = builder.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }

    let mut client = builder.connect().await?;
    let (mut subscribe_tx, mut stream) = client.subscribe().await?;
    subscribe_tx
        .send(grpc_backfill_subscription_request(&state, start_slot))
        .await?;

    let mut report = GrpcBackfillReport {
        start_slot,
        end_slot,
        ..Default::default()
    };
    let mut slot_times = BTreeMap::<u64, i64>::new();

    while let Some(message) = stream.next().await {
        let update = message?;

        match update.update_oneof {
            Some(UpdateOneof::Transaction(tx_update)) => {
                let slot = tx_update.slot;
                report.last_observed_slot = Some(slot);
                if slot > end_slot {
                    break;
                }
                if slot < start_slot {
                    continue;
                }

                let tx_info = tx_update
                    .transaction
                    .context("gRPC transaction update missing payload")?;
                let resolved = decode_transaction(tx_info)?;
                let block_time = slot_times.get(&slot).copied();
                let ingest_result = ingest_resolved_transaction(
                    state.clone(),
                    &resolved,
                    slot,
                    block_time,
                    IngestOptions {
                        enqueue_deliveries,
                        evaluate_metric_alerts: false,
                    },
                )
                .await
                .with_context(|| {
                    format!(
                        "ingest gRPC transaction {} from slot {slot}",
                        resolved.signature
                    )
                })?;

                report.transaction_count += 1;
                if ingest_result.matched {
                    report.matched_transaction_count += 1;
                }
                report.parsed_event_count += ingest_result.parsed_event_count;
                report.inserted_event_count += ingest_result.inserted_event_count;
                report.parsed_metric_observation_count +=
                    ingest_result.parsed_metric_observation_count;
                report.inserted_metric_observation_count +=
                    ingest_result.inserted_metric_observation_count;
                report.parsed_liquidity_pool_observation_count +=
                    ingest_result.parsed_liquidity_pool_observation_count;
                report.inserted_liquidity_pool_count += ingest_result.inserted_liquidity_pool_count;
                report.program_data_write_count += ingest_result.program_data_write_count;
                report.program_idl_data_write_count += ingest_result.program_idl_data_write_count;
                report.program_version_update_count += ingest_result.program_version_update_count;
                report.program_idl_version_update_count +=
                    ingest_result.program_idl_version_update_count;
            }
            Some(UpdateOneof::BlockMeta(block_meta)) => {
                report.last_observed_slot = Some(block_meta.slot);
                if let Some(block_time) = block_meta.block_time.map(|value| value.timestamp) {
                    slot_times.insert(block_meta.slot, block_time);
                    trim_backfill_slot_times(&mut slot_times);
                    if let Err(error) = state
                        .db
                        .update_block_time_for_slot(block_meta.slot, block_time)
                        .await
                    {
                        debug!(
                            slot = block_meta.slot,
                            block_time,
                            error = ?error,
                            "failed to fill missing block time from gRPC backfill block meta",
                        );
                    }
                }

                if (start_slot..=end_slot).contains(&block_meta.slot) {
                    report.observed_block_count += 1;
                    state.db.record_backfilled_slot(block_meta.slot).await?;
                }

                if block_meta.slot > end_slot {
                    break;
                }
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
            None => {}
        }
    }

    report.elapsed_ms = started.elapsed().as_millis();
    Ok(report)
}

fn grpc_backfill_subscription_request(state: &SharedState, start_slot: u64) -> SubscribeRequest {
    let mut watched_accounts = watched_backfill_accounts(state)
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    watched_accounts.sort();
    watched_accounts.dedup();

    SubscribeRequest {
        transactions: HashMap::from([(
            GRPC_BACKFILL_FILTER_LABEL.to_owned(),
            SubscribeRequestFilterTransactions {
                vote: Some(false),
                failed: Some(false),
                signature: None,
                account_include: watched_accounts,
                account_exclude: vec![],
                account_required: vec![],
            },
        )]),
        blocks_meta: HashMap::from([(
            GRPC_BACKFILL_BLOCK_META_FILTER_LABEL.to_owned(),
            SubscribeRequestFilterBlocksMeta {},
        )]),
        commitment: Some(CommitmentLevel::Finalized as i32),
        from_slot: Some(start_slot),
        ..Default::default()
    }
}

fn trim_backfill_slot_times(slot_times: &mut BTreeMap<u64, i64>) {
    while slot_times.len() > 4096 {
        if let Some(first_slot) = slot_times.keys().next().copied() {
            slot_times.remove(&first_slot);
        } else {
            break;
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FernoBlock {
    block_time: Option<i64>,
    ok: Option<bool>,
    #[serde(default)]
    empty: bool,
    #[serde(default)]
    transactions: Vec<FernoBlockTransaction>,
}

#[derive(Debug, Deserialize)]
struct FernoBlockTransaction {
    transaction: FernoTransaction,
    meta: Option<FernoTransactionMeta>,
}

#[derive(Debug, Deserialize)]
struct FernoTransaction {
    signatures: Vec<String>,
    message: FernoMessage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FernoMessage {
    account_keys: Vec<String>,
    instructions: Vec<FernoInstruction>,
    header: FernoMessageHeader,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FernoMessageHeader {
    num_required_signatures: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FernoInstruction {
    program_id_index: usize,
    #[serde(default)]
    accounts: Vec<usize>,
    #[serde(default)]
    data: String,
    stack_height: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FernoTransactionMeta {
    err: Option<Value>,
    log_messages: Option<Vec<String>>,
    inner_instructions: Option<Vec<FernoInnerInstructionSet>>,
    loaded_addresses: Option<FernoLoadedAddresses>,
}

#[derive(Debug, Deserialize)]
struct FernoInnerInstructionSet {
    index: usize,
    instructions: Vec<FernoInstruction>,
}

#[derive(Debug, Deserialize)]
struct FernoLoadedAddresses {
    writable: Option<Vec<String>>,
    readonly: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RpcBlockResponse {
    result: Option<FernoBlock>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcAccountInfoResponse {
    result: Option<RpcAccountInfoResult>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcAccountInfoResult {
    value: Option<RpcAccountInfo>,
}

#[derive(Debug, Deserialize)]
struct RpcAccountInfo {
    owner: String,
    data: Value,
}

#[derive(Debug, Deserialize)]
struct RpcSignaturesResponse {
    result: Option<Vec<RpcSignatureInfo>>,
    error: Option<RpcError>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcSignatureInfo {
    signature: String,
    slot: u64,
    err: Option<Value>,
}

pub(crate) async fn backfill_program_upgrade_history(
    state: SharedState,
    program_address: &str,
    before_slot: Option<u64>,
    limit: usize,
    lookback_slots: u64,
    concurrency: usize,
    retries: usize,
    enqueue_deliveries: bool,
) -> Result<ProgramUpgradeHistoryBackfillReport> {
    if limit == 0 {
        bail!("program upgrade history backfill limit must be greater than zero");
    }

    let started = Instant::now();
    let program_address = Pubkey::from_str(program_address)
        .with_context(|| format!("invalid program address {program_address:?}"))?;
    let program_data_account =
        resolve_upgradeable_program_data_account(&state, &program_address).await?;
    let signatures =
        fetch_program_upgrade_signatures(&state, &program_data_account, before_slot, limit).await?;
    let ranges = merged_program_upgrade_backfill_ranges(&signatures, lookback_slots);
    let mut windows = Vec::with_capacity(ranges.len());

    for (start_slot, end_slot) in ranges {
        let report = backfill_ferno_blocks(
            state.clone(),
            start_slot,
            end_slot,
            concurrency,
            retries,
            enqueue_deliveries,
        )
        .await;
        windows.push(ProgramUpgradeBackfillWindow {
            start_slot,
            end_slot,
            report,
        });
    }

    Ok(ProgramUpgradeHistoryBackfillReport {
        program_address: program_address.to_string(),
        program_data_account: program_data_account.to_string(),
        signatures,
        windows,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

async fn resolve_upgradeable_program_data_account(
    state: &SharedState,
    program_address: &Pubkey,
) -> Result<Pubkey> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": program_address.to_string(),
        "method": "getAccountInfo",
        "params": [
            program_address.to_string(),
            {
                "encoding": "base64",
                "commitment": "finalized"
            }
        ]
    });
    let response = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&request)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .with_context(|| format!("fetch RPC account info for {program_address}"))?
        .error_for_status()
        .with_context(|| {
            format!("RPC getAccountInfo endpoint returned an HTTP error for {program_address}")
        })?
        .json::<RpcAccountInfoResponse>()
        .await
        .with_context(|| format!("decode RPC getAccountInfo JSON for {program_address}"))?;

    if let Some(error) = response.error {
        bail!(
            "RPC getAccountInfo returned error {} for {}: {}",
            error.code,
            program_address,
            error.message
        );
    }

    let account = response
        .result
        .and_then(|result| result.value)
        .with_context(|| format!("program account {program_address} was not found"))?;
    if account.owner != bpf_loader_upgradeable::ID.to_string() {
        bail!(
            "program account {} is owned by {}, not the upgradeable loader",
            program_address,
            account.owner
        );
    }

    let account_data = decode_rpc_account_data(&account.data)
        .with_context(|| format!("decode program account data for {program_address}"))?;
    parse_upgradeable_program_data_address(&account_data)
        .with_context(|| format!("parse program data account for {program_address}"))
}

async fn fetch_program_upgrade_signatures(
    state: &SharedState,
    program_data_account: &Pubkey,
    before_slot: Option<u64>,
    limit: usize,
) -> Result<Vec<ProgramUpgradeSignature>> {
    let mut signatures = Vec::with_capacity(limit.min(PROGRAM_HISTORY_SIGNATURE_PAGE_LIMIT));
    let mut before_signature = None::<String>;

    for _ in 0..PROGRAM_HISTORY_MAX_SIGNATURE_PAGES {
        let mut config = json!({
            "limit": PROGRAM_HISTORY_SIGNATURE_PAGE_LIMIT
        });
        if let Some(signature) = before_signature.as_deref() {
            config["before"] = json!(signature);
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": program_data_account.to_string(),
            "method": "getSignaturesForAddress",
            "params": [
                program_data_account.to_string(),
                config
            ]
        });
        let response = state
            .http_client
            .post(&state.config.solana_rpc_http_url)
            .json(&request)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .with_context(|| format!("fetch signatures for program data {program_data_account}"))?
            .error_for_status()
            .with_context(|| {
                format!(
                    "RPC getSignaturesForAddress endpoint returned an HTTP error for {program_data_account}"
                )
            })?
            .json::<RpcSignaturesResponse>()
            .await
            .with_context(|| {
                format!("decode RPC getSignaturesForAddress JSON for {program_data_account}")
            })?;

        if let Some(error) = response.error {
            bail!(
                "RPC getSignaturesForAddress returned error {} for {}: {}",
                error.code,
                program_data_account,
                error.message
            );
        }

        let page = response.result.unwrap_or_default();
        if page.is_empty() {
            break;
        }

        for item in &page {
            if item.err.is_some() {
                continue;
            }
            if before_slot.is_some_and(|slot| item.slot >= slot) {
                continue;
            }
            signatures.push(ProgramUpgradeSignature {
                signature: item.signature.clone(),
                slot: item.slot,
            });
            if signatures.len() == limit {
                return Ok(signatures);
            }
        }

        before_signature = page.last().map(|item| item.signature.clone());
        if before_signature.is_none() {
            break;
        }
    }

    Ok(signatures)
}

fn decode_rpc_account_data(data: &Value) -> Result<Vec<u8>> {
    let encoded = data
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .or_else(|| data.as_str())
        .context("RPC account data is not base64 encoded")?;
    BASE64
        .decode(encoded)
        .context("RPC account data has invalid base64")
}

fn parse_upgradeable_program_data_address(data: &[u8]) -> Result<Pubkey> {
    if data.len() < 36 {
        bail!(
            "upgradeable program account data is too short: {} bytes",
            data.len()
        );
    }

    let tag = u32::from_le_bytes(
        data[0..4]
            .try_into()
            .context("upgradeable program account tag overflow")?,
    );
    if tag != UPGRADEABLE_LOADER_PROGRAM_TAG {
        bail!("upgradeable loader account is not a Program account");
    }

    let address = data[4..36]
        .try_into()
        .context("program data account address overflow")?;
    Ok(Pubkey::new_from_array(address))
}

fn merged_program_upgrade_backfill_ranges(
    signatures: &[ProgramUpgradeSignature],
    lookback_slots: u64,
) -> Vec<(u64, u64)> {
    let mut ranges = signatures
        .iter()
        .map(|signature| {
            (
                signature.slot.saturating_sub(lookback_slots),
                signature.slot,
            )
        })
        .collect::<Vec<_>>();
    ranges.sort_by_key(|(start_slot, end_slot)| (*start_slot, *end_slot));

    let mut merged = Vec::<(u64, u64)>::new();
    for (start_slot, end_slot) in ranges {
        if let Some((_, last_end_slot)) = merged.last_mut()
            && start_slot <= last_end_slot.saturating_add(1)
        {
            *last_end_slot = (*last_end_slot).max(end_slot);
            continue;
        }
        merged.push((start_slot, end_slot));
    }

    merged
}

pub(crate) async fn backfill_ferno_block(
    state: SharedState,
    slot: u64,
    enqueue_deliveries: bool,
) -> Result<BlockBackfillReport> {
    if use_experimental_protobuf_blocks() {
        return match backfill_ferno_block_proto(state.clone(), slot, enqueue_deliveries).await {
            Ok(report) => Ok(report),
            Err(proto_error) => {
                debug!(
                    slot,
                    error = %proto_error,
                    "falling back to Ferno JSON block backfill after protobuf fetch failed",
                );
                backfill_ferno_block_json(state, slot, enqueue_deliveries)
                    .await
                    .with_context(|| {
                        format!(
                            "Ferno protobuf block fetch failed before JSON fallback: {proto_error:#}"
                        )
                    })
            }
        };
    }

    backfill_ferno_block_json(state, slot, enqueue_deliveries).await
}

pub(crate) async fn backfill_rpc_block(
    state: SharedState,
    slot: u64,
    enqueue_deliveries: bool,
) -> Result<BlockBackfillReport> {
    let url = state.config.solana_rpc_http_url.clone();
    let request = json!({
        "jsonrpc": "2.0",
        "id": slot,
        "method": "getBlock",
        "params": [
            slot,
            {
                "encoding": "json",
                "transactionDetails": "full",
                "rewards": false,
                "maxSupportedTransactionVersion": 0
            }
        ]
    });
    let response = state
        .http_client
        .post(&url)
        .json(&request)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .with_context(|| format!("fetch Triton RPC block {slot}"))?
        .error_for_status()
        .with_context(|| format!("Triton RPC getBlock endpoint returned an HTTP error for {slot}"))?
        .json::<RpcBlockResponse>()
        .await
        .with_context(|| format!("decode Triton RPC getBlock JSON for {slot}"))?;

    if let Some(error) = response.error {
        if rpc_block_unavailable(&error) {
            state.db.record_backfilled_slot(slot).await?;
            return Ok(BlockBackfillReport {
                slot,
                url,
                ..Default::default()
            });
        }

        bail!(
            "Triton RPC getBlock returned error {} for slot {}: {}",
            error.code,
            slot,
            error.message
        );
    }

    let Some(block) = response.result else {
        state.db.record_backfilled_slot(slot).await?;
        return Ok(BlockBackfillReport {
            slot,
            url,
            ..Default::default()
        });
    };

    ingest_json_block(state, slot, url, block, enqueue_deliveries, "Triton RPC").await
}

fn rpc_block_unavailable(error: &RpcError) -> bool {
    let message = error.message.to_ascii_lowercase();
    message.contains("skipped")
        || message.contains("not available")
        || message.contains("was not found")
        || message.contains("block not found")
}

fn use_experimental_protobuf_blocks() -> bool {
    env::var("FERNO_BLOCK_FORMAT")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "bin" | "proto" | "protobuf"
            )
        })
        .unwrap_or(false)
}

async fn backfill_ferno_block_proto(
    state: SharedState,
    slot: u64,
    enqueue_deliveries: bool,
) -> Result<BlockBackfillReport> {
    let url = format!(
        "{}/{}.bin",
        state.config.ferno_block_base_url.trim_end_matches('/'),
        slot
    );
    let bytes = state
        .http_client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("fetch Ferno protobuf block {slot}"))?
        .error_for_status()
        .with_context(|| format!("Ferno protobuf block endpoint returned an error for {slot}"))?
        .bytes()
        .await
        .with_context(|| format!("read Ferno protobuf block bytes for {slot}"))?;
    let block = ProtoConfirmedBlock::decode(bytes.as_ref())
        .with_context(|| format!("decode Ferno protobuf block for {slot}"))?;

    let mut report = BlockBackfillReport {
        slot,
        url,
        ..Default::default()
    };
    let block_time = block.block_time.map(|value| value.timestamp);
    let watched_accounts = watched_backfill_account_bytes(&state);

    for (index, block_transaction) in block.transactions.into_iter().enumerate() {
        report.transaction_count += 1;
        let Some(transaction) = block_transaction.transaction else {
            report.skipped_missing_meta_count += 1;
            continue;
        };
        let Some(meta) = block_transaction.meta else {
            report.skipped_missing_meta_count += 1;
            continue;
        };

        if meta.err.is_some() {
            report.skipped_failed_transaction_count += 1;
            continue;
        }

        if !proto_transaction_touches_watched_accounts(&transaction, &meta, &watched_accounts) {
            report.skipped_unwatched_transaction_count += 1;
            continue;
        }

        let resolved = resolve_proto_block_transaction(transaction, meta).with_context(|| {
            format!("resolve transaction {index} from Ferno protobuf block {slot}")
        })?;
        let ingest_result = ingest_resolved_transaction(
            state.clone(),
            &resolved,
            slot,
            block_time,
            IngestOptions {
                enqueue_deliveries,
                evaluate_metric_alerts: false,
            },
        )
        .await
        .with_context(|| {
            format!(
                "ingest transaction {} from Ferno protobuf block {slot}",
                resolved.signature
            )
        })?;

        if ingest_result.matched {
            report.matched_transaction_count += 1;
        }
        report.parsed_event_count += ingest_result.parsed_event_count;
        report.inserted_event_count += ingest_result.inserted_event_count;
        report.parsed_metric_observation_count += ingest_result.parsed_metric_observation_count;
        report.inserted_metric_observation_count += ingest_result.inserted_metric_observation_count;
        report.parsed_liquidity_pool_observation_count +=
            ingest_result.parsed_liquidity_pool_observation_count;
        report.inserted_liquidity_pool_count += ingest_result.inserted_liquidity_pool_count;
        report.program_data_write_count += ingest_result.program_data_write_count;
        report.program_idl_data_write_count += ingest_result.program_idl_data_write_count;
        report.program_version_update_count += ingest_result.program_version_update_count;
        report.program_idl_version_update_count += ingest_result.program_idl_version_update_count;
    }

    state.db.record_backfilled_slot(slot).await?;

    Ok(report)
}

async fn backfill_ferno_block_json(
    state: SharedState,
    slot: u64,
    enqueue_deliveries: bool,
) -> Result<BlockBackfillReport> {
    let url = format!(
        "{}/{}",
        state.config.ferno_block_base_url.trim_end_matches('/'),
        slot
    );
    let block = state
        .http_client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("fetch Ferno block {slot}"))?
        .error_for_status()
        .with_context(|| format!("Ferno block endpoint returned an error for {slot}"))?
        .json::<FernoBlock>()
        .await
        .with_context(|| format!("decode Ferno block JSON for {slot}"))?;

    ingest_json_block(state, slot, url, block, enqueue_deliveries, "Ferno block").await
}

async fn ingest_json_block(
    state: SharedState,
    slot: u64,
    url: String,
    block: FernoBlock,
    enqueue_deliveries: bool,
    source_label: &str,
) -> Result<BlockBackfillReport> {
    let mut report = BlockBackfillReport {
        slot,
        url,
        ..Default::default()
    };

    if block.ok == Some(false) {
        bail!("{source_label} endpoint returned unsuccessful JSON for {slot}");
    }

    if block.empty && block.transactions.is_empty() {
        state.db.record_backfilled_slot(slot).await?;
        return Ok(report);
    }

    let block_time = block.block_time;
    let watched_accounts = watched_backfill_accounts(&state);

    for (index, block_transaction) in block.transactions.into_iter().enumerate() {
        report.transaction_count += 1;
        let Some(meta) = block_transaction.meta else {
            report.skipped_missing_meta_count += 1;
            continue;
        };

        if meta.err.is_some() {
            report.skipped_failed_transaction_count += 1;
            continue;
        }

        if !transaction_touches_watched_accounts(
            &block_transaction.transaction,
            &meta,
            &watched_accounts,
        ) {
            report.skipped_unwatched_transaction_count += 1;
            continue;
        }

        let resolved = resolve_block_transaction(block_transaction.transaction, meta)
            .with_context(|| format!("resolve transaction {index} from {source_label} {slot}"))?;
        let ingest_result = ingest_resolved_transaction(
            state.clone(),
            &resolved,
            slot,
            block_time,
            IngestOptions {
                enqueue_deliveries,
                evaluate_metric_alerts: false,
            },
        )
        .await
        .with_context(|| {
            format!(
                "ingest transaction {} from {source_label} {slot}",
                resolved.signature
            )
        })?;

        if ingest_result.matched {
            report.matched_transaction_count += 1;
        }
        report.parsed_event_count += ingest_result.parsed_event_count;
        report.inserted_event_count += ingest_result.inserted_event_count;
        report.parsed_metric_observation_count += ingest_result.parsed_metric_observation_count;
        report.inserted_metric_observation_count += ingest_result.inserted_metric_observation_count;
        report.parsed_liquidity_pool_observation_count +=
            ingest_result.parsed_liquidity_pool_observation_count;
        report.inserted_liquidity_pool_count += ingest_result.inserted_liquidity_pool_count;
        report.program_data_write_count += ingest_result.program_data_write_count;
        report.program_idl_data_write_count += ingest_result.program_idl_data_write_count;
        report.program_version_update_count += ingest_result.program_version_update_count;
        report.program_idl_version_update_count += ingest_result.program_idl_version_update_count;
    }

    state.db.record_backfilled_slot(slot).await?;

    Ok(report)
}

fn transaction_touches_watched_accounts(
    transaction: &FernoTransaction,
    meta: &FernoTransactionMeta,
    watched_accounts: &HashSet<String>,
) -> bool {
    transaction
        .message
        .account_keys
        .iter()
        .any(|key| watched_accounts.contains(key))
        || meta
            .loaded_addresses
            .as_ref()
            .and_then(|loaded_addresses| loaded_addresses.writable.as_ref())
            .is_some_and(|addresses| addresses.iter().any(|key| watched_accounts.contains(key)))
        || meta
            .loaded_addresses
            .as_ref()
            .and_then(|loaded_addresses| loaded_addresses.readonly.as_ref())
            .is_some_and(|addresses| addresses.iter().any(|key| watched_accounts.contains(key)))
}

fn proto_transaction_touches_watched_accounts(
    transaction: &ProtoTransaction,
    meta: &ProtoTransactionStatusMeta,
    watched_accounts: &HashSet<[u8; 32]>,
) -> bool {
    transaction.message.as_ref().is_some_and(|message| {
        message
            .account_keys
            .iter()
            .any(|key| pubkey_bytes(key).is_some_and(|pubkey| watched_accounts.contains(&pubkey)))
    }) || meta
        .loaded_writable_addresses
        .iter()
        .any(|key| pubkey_bytes(key).is_some_and(|pubkey| watched_accounts.contains(&pubkey)))
        || meta
            .loaded_readonly_addresses
            .iter()
            .any(|key| pubkey_bytes(key).is_some_and(|pubkey| watched_accounts.contains(&pubkey)))
}

fn base_watched_backfill_accounts() -> &'static HashSet<String> {
    static WATCHED_BACKFILL_ACCOUNTS: OnceLock<HashSet<String>> = OnceLock::new();
    WATCHED_BACKFILL_ACCOUNTS.get_or_init(|| {
        let mut accounts = HashSet::new();
        accounts.insert(bpf_loader::ID.to_string());
        accounts.insert(bpf_loader_deprecated::ID.to_string());
        accounts.insert(bpf_loader_upgradeable::ID.to_string());
        accounts.insert(loader_v4::ID.to_string());
        accounts.insert(name_service::program_id().to_string());
        accounts.insert(protocols::SQUADS_V4_PROGRAM_ID.to_owned());

        let watched_programs = protocols::watched_program_ids();
        accounts.extend(watched_programs.iter().map(ToString::to_string));
        accounts.extend(
            program_metadata::canonical_idl_metadata_addresses(&watched_programs)
                .into_iter()
                .map(|address| address.to_string()),
        );
        accounts.extend(
            anchor_idl::legacy_idl_addresses(&watched_programs)
                .into_iter()
                .map(|address| address.to_string()),
        );

        accounts
    })
}

fn watched_backfill_accounts(state: &SharedState) -> HashSet<String> {
    let mut accounts = base_watched_backfill_accounts().clone();
    if let Some(recipient) = state.config.solana_pay_recipient.as_deref() {
        if Pubkey::from_str(recipient).is_ok() {
            accounts.insert(recipient.to_owned());
        }
    }
    accounts
}

fn watched_backfill_account_bytes(state: &SharedState) -> HashSet<[u8; 32]> {
    watched_backfill_accounts(state)
        .iter()
        .filter_map(|account| {
            Pubkey::from_str(account)
                .ok()
                .map(|pubkey| pubkey.to_bytes())
        })
        .collect()
}

pub(crate) async fn backfill_ferno_blocks(
    state: SharedState,
    start_slot: u64,
    end_slot: u64,
    concurrency: usize,
    retries: usize,
    enqueue_deliveries: bool,
) -> BlockBackfillRangeReport {
    let started = Instant::now();
    let concurrency = concurrency.max(1);
    let mut outcomes = stream::iter(start_slot..=end_slot)
        .map(|slot| {
            let state = state.clone();
            async move {
                backfill_ferno_block_with_retries(state, slot, retries, enqueue_deliveries).await
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;
    outcomes.sort_by_key(|outcome| outcome.slot);

    BlockBackfillRangeReport {
        start_slot,
        end_slot,
        concurrency,
        retries,
        elapsed_ms: started.elapsed().as_millis(),
        outcomes,
    }
}

pub(crate) async fn backfill_rpc_blocks(
    state: SharedState,
    start_slot: u64,
    end_slot: u64,
    concurrency: usize,
    retries: usize,
    enqueue_deliveries: bool,
) -> BlockBackfillRangeReport {
    let started = Instant::now();
    let concurrency = concurrency.max(1);
    let mut outcomes = stream::iter(start_slot..=end_slot)
        .map(|slot| {
            let state = state.clone();
            async move {
                backfill_rpc_block_with_retries(state, slot, retries, enqueue_deliveries).await
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;
    outcomes.sort_by_key(|outcome| outcome.slot);

    BlockBackfillRangeReport {
        start_slot,
        end_slot,
        concurrency,
        retries,
        elapsed_ms: started.elapsed().as_millis(),
        outcomes,
    }
}

async fn backfill_ferno_block_with_retries(
    state: SharedState,
    slot: u64,
    retries: usize,
    enqueue_deliveries: bool,
) -> BlockBackfillOutcome {
    let started = Instant::now();
    let mut last_error = None;

    for attempt in 1..=(retries + 1) {
        match backfill_ferno_block(state.clone(), slot, enqueue_deliveries).await {
            Ok(report) => {
                return BlockBackfillOutcome {
                    slot,
                    attempts: attempt,
                    elapsed_ms: started.elapsed().as_millis(),
                    report: Some(report),
                    error: None,
                };
            }
            Err(error) => {
                last_error = Some(format!("{error:#}"));
                if attempt <= retries {
                    sleep(Duration::from_millis(250 * attempt as u64)).await;
                }
            }
        }
    }

    BlockBackfillOutcome {
        slot,
        attempts: retries + 1,
        elapsed_ms: started.elapsed().as_millis(),
        report: None,
        error: last_error,
    }
}

async fn backfill_rpc_block_with_retries(
    state: SharedState,
    slot: u64,
    retries: usize,
    enqueue_deliveries: bool,
) -> BlockBackfillOutcome {
    let started = Instant::now();
    let mut last_error = None;

    for attempt in 1..=(retries + 1) {
        match backfill_rpc_block(state.clone(), slot, enqueue_deliveries).await {
            Ok(report) => {
                return BlockBackfillOutcome {
                    slot,
                    attempts: attempt,
                    elapsed_ms: started.elapsed().as_millis(),
                    report: Some(report),
                    error: None,
                };
            }
            Err(error) => {
                last_error = Some(format!("{error:#}"));
                if attempt <= retries {
                    sleep(Duration::from_millis(250 * attempt as u64)).await;
                }
            }
        }
    }

    BlockBackfillOutcome {
        slot,
        attempts: retries + 1,
        elapsed_ms: started.elapsed().as_millis(),
        report: None,
        error: last_error,
    }
}

fn resolve_block_transaction(
    transaction: FernoTransaction,
    meta: FernoTransactionMeta,
) -> Result<ResolvedTransaction> {
    let mut account_keys = transaction
        .message
        .account_keys
        .iter()
        .map(|key| parse_pubkey(key))
        .collect::<Result<Vec<_>>>()?;

    if let Some(loaded_addresses) = &meta.loaded_addresses {
        if let Some(writable) = &loaded_addresses.writable {
            account_keys.extend(
                writable
                    .iter()
                    .map(|key| parse_pubkey(key))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
        if let Some(readonly) = &loaded_addresses.readonly {
            account_keys.extend(
                readonly
                    .iter()
                    .map(|key| parse_pubkey(key))
                    .collect::<Result<Vec<_>>>()?,
            );
        }
    }

    let primary_signer = if transaction.message.header.num_required_signatures > 0 {
        account_keys.first().copied()
    } else {
        None
    };

    let signature = transaction
        .signatures
        .first()
        .cloned()
        .context("transaction missing signature")?;

    let top_level_instructions = transaction
        .message
        .instructions
        .into_iter()
        .map(resolve_instruction)
        .collect::<Result<Vec<_>>>()?;

    let inner_instruction_sets = meta
        .inner_instructions
        .unwrap_or_default()
        .into_iter()
        .map(|set| {
            Ok(ResolvedInnerInstructionSet {
                index: set.index,
                instructions: set
                    .instructions
                    .into_iter()
                    .map(resolve_instruction)
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ResolvedTransaction {
        signature,
        primary_signer,
        account_keys,
        top_level_instructions,
        inner_instruction_sets,
        log_messages: meta.log_messages.unwrap_or_default(),
    })
}

fn resolve_proto_block_transaction(
    transaction: ProtoTransaction,
    meta: ProtoTransactionStatusMeta,
) -> Result<ResolvedTransaction> {
    let message = transaction
        .message
        .context("protobuf transaction missing message")?;
    let mut account_keys = message
        .account_keys
        .iter()
        .map(|key| parse_pubkey_bytes(key))
        .collect::<Result<Vec<_>>>()?;

    account_keys.extend(
        meta.loaded_writable_addresses
            .iter()
            .map(|key| parse_pubkey_bytes(key))
            .collect::<Result<Vec<_>>>()?,
    );
    account_keys.extend(
        meta.loaded_readonly_addresses
            .iter()
            .map(|key| parse_pubkey_bytes(key))
            .collect::<Result<Vec<_>>>()?,
    );

    let primary_signer = if message
        .header
        .as_ref()
        .is_some_and(|header| header.num_required_signatures > 0)
    {
        account_keys.first().copied()
    } else {
        None
    };

    let signature = transaction
        .signatures
        .first()
        .map(bs58::encode)
        .map(|signature| signature.into_string())
        .context("protobuf transaction missing signature")?;

    let top_level_instructions = message
        .instructions
        .into_iter()
        .map(resolve_proto_instruction)
        .collect::<Result<Vec<_>>>()?;

    let inner_instruction_sets = meta
        .inner_instructions
        .into_iter()
        .map(|set| {
            Ok(ResolvedInnerInstructionSet {
                index: set.index as usize,
                instructions: set
                    .instructions
                    .into_iter()
                    .map(resolve_proto_inner_instruction)
                    .collect::<Result<Vec<_>>>()?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ResolvedTransaction {
        signature,
        primary_signer,
        account_keys,
        top_level_instructions,
        inner_instruction_sets,
        log_messages: meta.log_messages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_upgradeable_program_data_address() -> Result<()> {
        let program_data = Pubkey::new_unique();
        let mut account_data = Vec::new();
        account_data.extend_from_slice(&UPGRADEABLE_LOADER_PROGRAM_TAG.to_le_bytes());
        account_data.extend_from_slice(&program_data.to_bytes());

        assert_eq!(
            parse_upgradeable_program_data_address(&account_data)?,
            program_data
        );
        Ok(())
    }

    #[test]
    fn merges_overlapping_program_upgrade_backfill_ranges() {
        let ranges = merged_program_upgrade_backfill_ranges(
            &[
                ProgramUpgradeSignature {
                    signature: "newer".to_owned(),
                    slot: 105,
                },
                ProgramUpgradeSignature {
                    signature: "older".to_owned(),
                    slot: 100,
                },
                ProgramUpgradeSignature {
                    signature: "separate".to_owned(),
                    slot: 130,
                },
            ],
            10,
        );

        assert_eq!(ranges, vec![(90, 105), (120, 130)]);
    }
}

fn resolve_instruction(instruction: FernoInstruction) -> Result<ResolvedInstruction> {
    Ok(ResolvedInstruction {
        program_id_index: instruction.program_id_index,
        accounts: instruction.accounts,
        data: bs58::decode(instruction.data)
            .into_vec()
            .context("decode base58 instruction data")?,
        stack_height: instruction.stack_height,
    })
}

fn resolve_proto_instruction(instruction: ProtoCompiledInstruction) -> Result<ResolvedInstruction> {
    Ok(ResolvedInstruction {
        program_id_index: instruction.program_id_index as usize,
        accounts: instruction
            .accounts
            .into_iter()
            .map(usize::from)
            .collect::<Vec<_>>(),
        data: instruction.data,
        stack_height: None,
    })
}

fn resolve_proto_inner_instruction(
    instruction: ProtoInnerInstruction,
) -> Result<ResolvedInstruction> {
    Ok(ResolvedInstruction {
        program_id_index: instruction.program_id_index as usize,
        accounts: instruction
            .accounts
            .into_iter()
            .map(usize::from)
            .collect::<Vec<_>>(),
        data: instruction.data,
        stack_height: instruction.stack_height,
    })
}

fn parse_pubkey(value: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).with_context(|| format!("invalid pubkey {value}"))
}

fn parse_pubkey_bytes(value: &[u8]) -> Result<Pubkey> {
    let bytes = pubkey_bytes(value).context("invalid protobuf pubkey length")?;
    Ok(Pubkey::new_from_array(bytes))
}

fn pubkey_bytes(value: &[u8]) -> Option<[u8; 32]> {
    value.try_into().ok()
}
