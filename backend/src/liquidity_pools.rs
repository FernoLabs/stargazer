use crate::{
    context::SharedState,
    models::now_ts,
    protocols,
    solana::{ResolvedInstruction, ResolvedTransaction, instruction_account},
    token_liquidity::{LOMBARD_LBTC_MINT, LOMBARD_LBTC_SYMBOL},
};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashSet,
    str::FromStr,
    time::{Duration, Instant},
};

pub(crate) const METEORA_DLMM_PROGRAM_ID: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";
pub(crate) const ORCA_WHIRLPOOLS_PROGRAM_ID: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
pub(crate) const METEORA_AMM_V2_PROGRAM_ID: &str = "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG";
pub(crate) const RAYDIUM_CLMM_PROGRAM_ID: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";
pub(crate) const RAYDIUM_CPMM_PROGRAM_ID: &str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";
pub(crate) const RAYDIUM_AMM_V4_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";

const METEORA_DLMM_DEX_ID: &str = "meteora";
const METEORA_DLMM_LB_PAIR_ACCOUNT_SIZE: usize = 904;
const METEORA_DLMM_LB_PAIR_DECODE_LEN: usize = 216;
const METEORA_DLMM_LB_PAIR_DISCRIMINATOR: [u8; 8] = [33, 11, 49, 98, 181, 101, 177, 13];
const METEORA_DLMM_TOKEN_X_MINT_OFFSET: usize = 88;
const METEORA_DLMM_TOKEN_Y_MINT_OFFSET: usize = 120;
const METEORA_DLMM_RESERVE_X_OFFSET: usize = 152;
const METEORA_DLMM_RESERVE_Y_OFFSET: usize = 184;

const INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR: [u8; 8] =
    [46, 39, 41, 135, 111, 183, 200, 64];
const INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR2: [u8; 8] =
    [243, 73, 129, 126, 51, 19, 241, 107];
const INITIALIZE_LB_PAIR: [u8; 8] = [45, 154, 237, 210, 221, 15, 166, 92];
const INITIALIZE_LB_PAIR2: [u8; 8] = [73, 59, 36, 120, 237, 83, 108, 198];
const INITIALIZE_PERMISSION_LB_PAIR: [u8; 8] = [108, 102, 213, 85, 251, 3, 53, 21];

#[derive(Clone, Debug)]
pub(crate) struct LiquidityPoolRecord {
    pub(crate) pool_address: String,
    pub(crate) protocol_slug: String,
    pub(crate) protocol_name: String,
    pub(crate) dex_id: String,
    pub(crate) program_id: String,
    pub(crate) program_label: String,
    pub(crate) token_x_mint: String,
    pub(crate) token_y_mint: String,
    pub(crate) reserve_x: Option<String>,
    pub(crate) reserve_y: Option<String>,
    pub(crate) source: String,
    pub(crate) first_seen_slot: Option<i64>,
    pub(crate) first_seen_signature: Option<String>,
    pub(crate) last_seen_slot: Option<i64>,
    pub(crate) last_seen_signature: Option<String>,
    pub(crate) updated_at: i64,
}

#[allow(dead_code)]
pub(crate) const LIQUIDITY_POOL_SNAPSHOT_RETENTION_DEFAULT: &str = "default";
pub(crate) const LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED: &str = "watched";

#[derive(Clone, Debug)]
pub(crate) struct LiquidityPoolLatestStateRecord {
    pub(crate) pool_address: String,
    pub(crate) dex_id: String,
    pub(crate) base_token_mint: String,
    pub(crate) quote_token_mint: String,
    pub(crate) base_reserve: Option<f64>,
    pub(crate) quote_reserve: Option<f64>,
    pub(crate) base_vault: Option<String>,
    pub(crate) quote_vault: Option<String>,
    pub(crate) estimated_liquidity_usd: Option<f64>,
    pub(crate) latest_signature: String,
    pub(crate) latest_slot: i64,
    pub(crate) latest_block_time: Option<i64>,
    pub(crate) updated_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct LiquidityPoolSnapshotRecord {
    pub(crate) id: String,
    pub(crate) pool_address: String,
    pub(crate) dex_id: String,
    pub(crate) base_token_mint: String,
    pub(crate) quote_token_mint: String,
    pub(crate) signature: String,
    pub(crate) slot: i64,
    pub(crate) block_time: Option<i64>,
    pub(crate) base_reserve: Option<f64>,
    pub(crate) quote_reserve: Option<f64>,
    pub(crate) base_vault: Option<String>,
    pub(crate) quote_vault: Option<String>,
    pub(crate) estimated_liquidity_usd: Option<f64>,
    pub(crate) retention_class: String,
    pub(crate) observed_at: i64,
}

#[derive(Clone, Debug)]
pub(crate) struct LiquidityPoolSeedOptions {
    pub(crate) mint: Option<String>,
    pub(crate) include_all: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LiquidityPoolSeedReport {
    pub(crate) mint: Option<String>,
    pub(crate) dex_programs_checked: usize,
    pub(crate) decoded_pool_count: usize,
    pub(crate) inserted_pool_count: usize,
    pub(crate) updated_pool_count: usize,
    pub(crate) unsupported_programs: Vec<String>,
    pub(crate) pools: Vec<LiquidityPoolRecord>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct LiquidityPoolDimensionReport {
    pub(crate) sources: Vec<LiquidityPoolDimensionSourceReport>,
}

impl LiquidityPoolDimensionReport {
    pub(crate) fn total_pool_count(&self) -> usize {
        self.sources
            .iter()
            .filter_map(|source| source.account_count)
            .sum()
    }

    pub(crate) fn total_rpc_response_bytes(&self) -> usize {
        self.sources
            .iter()
            .filter_map(|source| source.rpc_response_bytes)
            .sum()
    }

    pub(crate) fn failed_source_count(&self) -> usize {
        self.sources
            .iter()
            .filter(|source| source.error.is_some())
            .count()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LiquidityPoolDimensionSourceReport {
    pub(crate) label: String,
    pub(crate) protocol_slug: String,
    pub(crate) program_id: String,
    pub(crate) account_type: String,
    pub(crate) account_count: Option<usize>,
    pub(crate) rpc_response_bytes: Option<usize>,
    pub(crate) elapsed_ms: u128,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RpcProgramAccountsResponse {
    result: Option<Vec<RpcProgramAccount>>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcProgramAccount {
    pubkey: String,
    account: RpcAccount,
}

#[derive(Debug, Deserialize)]
struct RpcAccount {
    data: RpcAccountData,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RpcAccountData {
    Encoded(Vec<String>),
    Raw(String),
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcProgramAccountsCountResponse {
    result: Option<Vec<RpcProgramAccountKeyOnly>>,
    error: Option<RpcError>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct RpcProgramAccountKeyOnly {
    pubkey: String,
}

#[derive(Clone, Debug)]
struct PoolAccountSourceSpec {
    label: &'static str,
    protocol_slug: &'static str,
    program_id: &'static str,
    account_type: &'static str,
    filters: Vec<Value>,
}

#[derive(Clone, Copy, Debug)]
struct MeteoraInitializeLayout {
    instruction_name: &'static str,
    pool_position: usize,
    token_x_mint_position: usize,
    token_y_mint_position: usize,
    reserve_x_position: usize,
    reserve_y_position: usize,
}

pub(crate) async fn seed_liquidity_pools(
    state: SharedState,
    options: LiquidityPoolSeedOptions,
) -> Result<LiquidityPoolSeedReport> {
    if options.include_all && options.mint.is_some() {
        bail!("use either --all or --mint, not both");
    }

    let mint = if options.include_all {
        None
    } else {
        Some(options.mint.unwrap_or_else(|| LOMBARD_LBTC_MINT.to_owned()))
    };

    if let Some(mint) = mint.as_deref() {
        Pubkey::from_str(mint).with_context(|| format!("invalid mint address {mint}"))?;
    }

    let mut report = LiquidityPoolSeedReport {
        mint,
        dex_programs_checked: 1,
        unsupported_programs: unsupported_seed_programs(),
        ..Default::default()
    };

    let pools = discover_meteora_dlmm_pools(state.clone(), report.mint.as_deref()).await?;
    report.decoded_pool_count = pools.len();

    for pool in pools {
        if state
            .db
            .upsert_liquidity_pool(&pool)
            .await
            .context("upsert seeded liquidity pool")?
        {
            report.inserted_pool_count += 1;
        } else {
            report.updated_pool_count += 1;
        }
        report.pools.push(pool);
    }

    Ok(report)
}

pub(crate) async fn dimension_liquidity_pools(
    state: SharedState,
) -> Result<LiquidityPoolDimensionReport> {
    let mut report = LiquidityPoolDimensionReport::default();

    for source in liquidity_pool_account_sources() {
        let started = Instant::now();
        let outcome = get_program_account_count(state.clone(), &source).await;
        let elapsed_ms = started.elapsed().as_millis();

        match outcome {
            Ok((account_count, response_bytes)) => {
                report.sources.push(LiquidityPoolDimensionSourceReport {
                    label: source.label.to_owned(),
                    protocol_slug: source.protocol_slug.to_owned(),
                    program_id: source.program_id.to_owned(),
                    account_type: source.account_type.to_owned(),
                    account_count: Some(account_count),
                    rpc_response_bytes: Some(response_bytes),
                    elapsed_ms,
                    error: None,
                });
            }
            Err(error) => {
                report.sources.push(LiquidityPoolDimensionSourceReport {
                    label: source.label.to_owned(),
                    protocol_slug: source.protocol_slug.to_owned(),
                    program_id: source.program_id.to_owned(),
                    account_type: source.account_type.to_owned(),
                    account_count: None,
                    rpc_response_bytes: None,
                    elapsed_ms,
                    error: Some(error.to_string()),
                });
            }
        }
    }

    Ok(report)
}

pub(crate) fn pool_observation_from_instruction(
    tx: &ResolvedTransaction,
    instruction: &ResolvedInstruction,
    slot: u64,
    block_time: Option<i64>,
    _instruction_path: &str,
) -> Option<LiquidityPoolRecord> {
    let program_id = tx.account_keys.get(instruction.program_id_index).copied()?;
    if program_id.to_string() != METEORA_DLMM_PROGRAM_ID {
        return None;
    }

    let layout = meteora_initialize_layout(&instruction.data)?;
    let pool_address = instruction_account(tx, instruction, layout.pool_position).ok()?;
    let token_x_mint = instruction_account(tx, instruction, layout.token_x_mint_position).ok()?;
    let token_y_mint = instruction_account(tx, instruction, layout.token_y_mint_position).ok()?;
    let reserve_x = instruction_account(tx, instruction, layout.reserve_x_position).ok()?;
    let reserve_y = instruction_account(tx, instruction, layout.reserve_y_position).ok()?;
    let metadata = protocol_metadata(program_id);

    Some(LiquidityPoolRecord {
        pool_address: pool_address.to_string(),
        protocol_slug: metadata.protocol_slug,
        protocol_name: metadata.protocol_name,
        dex_id: METEORA_DLMM_DEX_ID.to_owned(),
        program_id: program_id.to_string(),
        program_label: metadata.program_label,
        token_x_mint: token_x_mint.to_string(),
        token_y_mint: token_y_mint.to_string(),
        reserve_x: Some(reserve_x.to_string()),
        reserve_y: Some(reserve_y.to_string()),
        source: format!("block:{}", layout.instruction_name),
        first_seen_slot: Some(slot as i64),
        first_seen_signature: Some(tx.signature.clone()),
        last_seen_slot: Some(slot as i64),
        last_seen_signature: Some(tx.signature.clone()),
        updated_at: block_time.unwrap_or_else(now_ts),
    })
}

async fn discover_meteora_dlmm_pools(
    state: SharedState,
    mint: Option<&str>,
) -> Result<Vec<LiquidityPoolRecord>> {
    let mut filters = Vec::new();
    if let Some(mint) = mint {
        filters.push(Some((METEORA_DLMM_TOKEN_X_MINT_OFFSET, mint.to_owned())));
        filters.push(Some((METEORA_DLMM_TOKEN_Y_MINT_OFFSET, mint.to_owned())));
    } else {
        filters.push(None);
    }

    let mut seen = HashSet::new();
    let mut pools = Vec::new();
    for mint_filter in filters {
        for account in get_program_accounts_meteora_dlmm(state.clone(), mint_filter).await? {
            if !seen.insert(account.pubkey.clone()) {
                continue;
            }
            let data = rpc_account_data_bytes(&account.account.data)
                .with_context(|| format!("decode Meteora DLMM account {}", account.pubkey))?;
            if let Some(pool) = parse_meteora_dlmm_lb_pair_account(&account.pubkey, &data) {
                pools.push(pool);
            }
        }
    }

    pools.sort_by(|left, right| left.pool_address.cmp(&right.pool_address));
    Ok(pools)
}

async fn get_program_accounts_meteora_dlmm(
    state: SharedState,
    mint_filter: Option<(usize, String)>,
) -> Result<Vec<RpcProgramAccount>> {
    let mut filters = vec![
        json!({ "dataSize": METEORA_DLMM_LB_PAIR_ACCOUNT_SIZE }),
        json!({
            "memcmp": {
                "offset": 0,
                "bytes": bs58::encode(METEORA_DLMM_LB_PAIR_DISCRIMINATOR).into_string()
            }
        }),
    ];
    if let Some((offset, mint)) = mint_filter {
        filters.push(json!({
            "memcmp": {
                "offset": offset,
                "bytes": mint
            }
        }));
    }

    let request = json!({
        "jsonrpc": "2.0",
        "id": "firewatch-seed-liquidity-pools",
        "method": "getProgramAccounts",
        "params": [
            METEORA_DLMM_PROGRAM_ID,
            {
                "encoding": "base64",
                "commitment": "finalized",
                "dataSlice": {
                    "offset": 0,
                    "length": METEORA_DLMM_LB_PAIR_DECODE_LEN
                },
                "filters": filters
            }
        ]
    });

    let response = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&request)
        .timeout(Duration::from_secs(60))
        .send()
        .await
        .context("fetch Meteora DLMM program accounts")?
        .error_for_status()
        .context("RPC getProgramAccounts returned an HTTP error")?
        .json::<RpcProgramAccountsResponse>()
        .await
        .context("decode Meteora DLMM getProgramAccounts response")?;

    if let Some(error) = response.error {
        bail!(
            "RPC getProgramAccounts failed with code {}: {}",
            error.code,
            error.message
        );
    }

    Ok(response.result.unwrap_or_default())
}

async fn get_program_account_count(
    state: SharedState,
    source: &PoolAccountSourceSpec,
) -> Result<(usize, usize)> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": "firewatch-dimension-liquidity-pools",
        "method": "getProgramAccounts",
        "params": [
            source.program_id,
            {
                "encoding": "base64",
                "commitment": "finalized",
                "dataSlice": {
                    "offset": 0,
                    "length": 0
                },
                "filters": source.filters
            }
        ]
    });

    let response_bytes = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&request)
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .with_context(|| format!("count {} accounts", source.label))?
        .error_for_status()
        .with_context(|| {
            format!(
                "RPC getProgramAccounts returned HTTP error for {}",
                source.label
            )
        })?
        .bytes()
        .await
        .with_context(|| format!("read {} account count response", source.label))?;
    let byte_len = response_bytes.len();
    let response = serde_json::from_slice::<RpcProgramAccountsCountResponse>(&response_bytes)
        .with_context(|| format!("decode {} account count response", source.label))?;

    if let Some(error) = response.error {
        bail!(
            "RPC getProgramAccounts failed with code {}: {}",
            error.code,
            error.message
        );
    }

    Ok((response.result.unwrap_or_default().len(), byte_len))
}

fn parse_meteora_dlmm_lb_pair_account(
    pool_address: &str,
    data: &[u8],
) -> Option<LiquidityPoolRecord> {
    if data.len() < METEORA_DLMM_LB_PAIR_DECODE_LEN {
        return None;
    }
    if data.get(..8)? != METEORA_DLMM_LB_PAIR_DISCRIMINATOR {
        return None;
    }

    let program_id = Pubkey::from_str(METEORA_DLMM_PROGRAM_ID).ok()?;
    let metadata = protocol_metadata(program_id);
    Some(LiquidityPoolRecord {
        pool_address: pool_address.to_owned(),
        protocol_slug: metadata.protocol_slug,
        protocol_name: metadata.protocol_name,
        dex_id: METEORA_DLMM_DEX_ID.to_owned(),
        program_id: METEORA_DLMM_PROGRAM_ID.to_owned(),
        program_label: metadata.program_label,
        token_x_mint: pubkey_at(data, METEORA_DLMM_TOKEN_X_MINT_OFFSET)?.to_string(),
        token_y_mint: pubkey_at(data, METEORA_DLMM_TOKEN_Y_MINT_OFFSET)?.to_string(),
        reserve_x: Some(pubkey_at(data, METEORA_DLMM_RESERVE_X_OFFSET)?.to_string()),
        reserve_y: Some(pubkey_at(data, METEORA_DLMM_RESERVE_Y_OFFSET)?.to_string()),
        source: "gpa:meteora_dlmm".to_owned(),
        first_seen_slot: None,
        first_seen_signature: None,
        last_seen_slot: None,
        last_seen_signature: None,
        updated_at: now_ts(),
    })
}

fn rpc_account_data_bytes(data: &RpcAccountData) -> Result<Vec<u8>> {
    let encoded = match data {
        RpcAccountData::Encoded(values) => values
            .first()
            .context("RPC account data is missing base64 payload")?,
        RpcAccountData::Raw(value) => value,
    };
    BASE64
        .decode(encoded)
        .context("RPC account data is not valid base64")
}

fn pubkey_at(data: &[u8], offset: usize) -> Option<Pubkey> {
    let bytes: [u8; 32] = data.get(offset..offset + 32)?.try_into().ok()?;
    Some(Pubkey::new_from_array(bytes))
}

fn meteora_initialize_layout(data: &[u8]) -> Option<MeteoraInitializeLayout> {
    let discriminator: [u8; 8] = data.get(..8)?.try_into().ok()?;
    match discriminator {
        INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR => Some(MeteoraInitializeLayout {
            instruction_name: "initialize_customizable_permissionless_lb_pair",
            pool_position: 0,
            token_x_mint_position: 2,
            token_y_mint_position: 3,
            reserve_x_position: 4,
            reserve_y_position: 5,
        }),
        INITIALIZE_CUSTOMIZABLE_PERMISSIONLESS_LB_PAIR2 => Some(MeteoraInitializeLayout {
            instruction_name: "initialize_customizable_permissionless_lb_pair2",
            pool_position: 0,
            token_x_mint_position: 2,
            token_y_mint_position: 3,
            reserve_x_position: 4,
            reserve_y_position: 5,
        }),
        INITIALIZE_LB_PAIR => Some(MeteoraInitializeLayout {
            instruction_name: "initialize_lb_pair",
            pool_position: 0,
            token_x_mint_position: 2,
            token_y_mint_position: 3,
            reserve_x_position: 4,
            reserve_y_position: 5,
        }),
        INITIALIZE_LB_PAIR2 => Some(MeteoraInitializeLayout {
            instruction_name: "initialize_lb_pair2",
            pool_position: 0,
            token_x_mint_position: 2,
            token_y_mint_position: 3,
            reserve_x_position: 4,
            reserve_y_position: 5,
        }),
        INITIALIZE_PERMISSION_LB_PAIR => Some(MeteoraInitializeLayout {
            instruction_name: "initialize_permission_lb_pair",
            pool_position: 1,
            token_x_mint_position: 3,
            token_y_mint_position: 4,
            reserve_x_position: 5,
            reserve_y_position: 6,
        }),
        _ => None,
    }
}

fn liquidity_pool_account_sources() -> Vec<PoolAccountSourceSpec> {
    vec![
        PoolAccountSourceSpec {
            label: "Meteora DLMM lbPair",
            protocol_slug: "meteora_dlmm",
            program_id: METEORA_DLMM_PROGRAM_ID,
            account_type: "lbPair",
            filters: vec![
                data_size_filter(METEORA_DLMM_LB_PAIR_ACCOUNT_SIZE),
                discriminator_filter(0, &METEORA_DLMM_LB_PAIR_DISCRIMINATOR),
            ],
        },
        PoolAccountSourceSpec {
            label: "Orca Whirlpools Whirlpool",
            protocol_slug: "orca_whirlpools",
            program_id: ORCA_WHIRLPOOLS_PROGRAM_ID,
            account_type: "Whirlpool",
            filters: vec![anchor_account_filter("Whirlpool")],
        },
        PoolAccountSourceSpec {
            label: "Meteora AMM V2 Pool",
            protocol_slug: "meteora",
            program_id: METEORA_AMM_V2_PROGRAM_ID,
            account_type: "Pool",
            filters: vec![anchor_account_filter("Pool")],
        },
        PoolAccountSourceSpec {
            label: "Raydium CLMM PoolState",
            protocol_slug: "raydium",
            program_id: RAYDIUM_CLMM_PROGRAM_ID,
            account_type: "PoolState",
            filters: vec![anchor_account_filter("PoolState")],
        },
        PoolAccountSourceSpec {
            label: "Raydium CPMM PoolState",
            protocol_slug: "raydium",
            program_id: RAYDIUM_CPMM_PROGRAM_ID,
            account_type: "PoolState",
            filters: vec![anchor_account_filter("PoolState")],
        },
        PoolAccountSourceSpec {
            label: "Raydium AMM v4 legacy pool",
            protocol_slug: "raydium",
            program_id: RAYDIUM_AMM_V4_PROGRAM_ID,
            account_type: "AmmInfo",
            filters: vec![data_size_filter(752)],
        },
    ]
}

fn data_size_filter(data_size: usize) -> Value {
    json!({ "dataSize": data_size })
}

fn anchor_account_filter(account_name: &str) -> Value {
    discriminator_filter(0, &anchor_account_discriminator(account_name))
}

fn discriminator_filter(offset: usize, discriminator: &[u8]) -> Value {
    json!({
        "memcmp": {
            "offset": offset,
            "bytes": bs58::encode(discriminator).into_string()
        }
    })
}

fn anchor_account_discriminator(account_name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("account:{account_name}"));
    hasher.finalize()[..8]
        .try_into()
        .expect("sha256 digest has at least 8 bytes")
}

fn protocol_metadata(program_id: Pubkey) -> ProtocolPoolMetadata {
    protocols::protocol_for_program(program_id)
        .map(|protocol| ProtocolPoolMetadata {
            protocol_slug: protocol.slug,
            protocol_name: protocol.name,
            program_label: protocol.program_label,
        })
        .unwrap_or_else(|| ProtocolPoolMetadata {
            protocol_slug: "unknown".to_owned(),
            protocol_name: "Unknown".to_owned(),
            program_label: "Unknown".to_owned(),
        })
}

fn unsupported_seed_programs() -> Vec<String> {
    [
        "Orca Whirlpools",
        "Raydium CLMM",
        "Raydium CPMM",
        "Raydium AMM v4",
        "Meteora AMM V2",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

struct ProtocolPoolMetadata {
    protocol_slug: String,
    protocol_name: String,
    program_label: String,
}

pub(crate) fn default_seed_mint_label(mint: Option<&str>) -> String {
    match mint {
        Some(mint) if mint == LOMBARD_LBTC_MINT => {
            format!("{LOMBARD_LBTC_SYMBOL} ({LOMBARD_LBTC_MINT})")
        }
        Some(mint) => mint.to_owned(),
        None => "all supported pools".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solana::{ResolvedInstruction, ResolvedTransaction};

    const POOL: &str = "2gdjgchz31VdaAxmERiAbTAJbzAtGBei7LAwAMV45P1M";
    const TOKEN_X: &str = "cbbtcs9CfBRH1xw4FS4QZ3D4QCLua4cQSNp1gu5G5pP";
    const TOKEN_Y: &str = LOMBARD_LBTC_MINT;
    const RESERVE_X: &str = "G7y6GWTR1tQvF5SMdw5f3NfptGYQBddBLp16XWkZKJ64";
    const RESERVE_Y: &str = "FT2UeAzUiq24opaxndnqJ6kgWwP1qZq22kR2dRDZanQL";

    #[test]
    fn decodes_meteora_dlmm_lb_pair_account() {
        let mut data = vec![0_u8; METEORA_DLMM_LB_PAIR_DECODE_LEN];
        data[..8].copy_from_slice(&METEORA_DLMM_LB_PAIR_DISCRIMINATOR);
        write_pubkey(&mut data, METEORA_DLMM_TOKEN_X_MINT_OFFSET, TOKEN_X);
        write_pubkey(&mut data, METEORA_DLMM_TOKEN_Y_MINT_OFFSET, TOKEN_Y);
        write_pubkey(&mut data, METEORA_DLMM_RESERVE_X_OFFSET, RESERVE_X);
        write_pubkey(&mut data, METEORA_DLMM_RESERVE_Y_OFFSET, RESERVE_Y);

        let pool = parse_meteora_dlmm_lb_pair_account(POOL, &data).expect("pool decoded");

        assert_eq!(pool.pool_address, POOL);
        assert_eq!(pool.protocol_slug, "meteora_dlmm");
        assert_eq!(pool.token_x_mint, TOKEN_X);
        assert_eq!(pool.token_y_mint, TOKEN_Y);
        assert_eq!(pool.reserve_x.as_deref(), Some(RESERVE_X));
        assert_eq!(pool.reserve_y.as_deref(), Some(RESERVE_Y));
    }

    #[test]
    fn observes_meteora_initialize_lb_pair2_instruction() {
        let program_id = Pubkey::from_str(METEORA_DLMM_PROGRAM_ID).unwrap();
        let pool = Pubkey::from_str(POOL).unwrap();
        let token_x = Pubkey::from_str(TOKEN_X).unwrap();
        let token_y = Pubkey::from_str(TOKEN_Y).unwrap();
        let reserve_x = Pubkey::from_str(RESERVE_X).unwrap();
        let reserve_y = Pubkey::from_str(RESERVE_Y).unwrap();
        let tx = ResolvedTransaction {
            signature: "test-signature".to_owned(),
            primary_signer: None,
            account_keys: vec![
                program_id,
                pool,
                Pubkey::new_unique(),
                token_x,
                token_y,
                reserve_x,
                reserve_y,
            ],
            top_level_instructions: Vec::new(),
            inner_instruction_sets: Vec::new(),
            log_messages: Vec::new(),
        };
        let instruction = ResolvedInstruction {
            program_id_index: 0,
            accounts: vec![1, 2, 3, 4, 5, 6],
            data: INITIALIZE_LB_PAIR2.to_vec(),
            stack_height: Some(1),
        };

        let observation = pool_observation_from_instruction(
            &tx,
            &instruction,
            421_651_686,
            Some(1_716_467_969),
            "0",
        )
        .expect("pool observed");

        assert_eq!(observation.pool_address, POOL);
        assert_eq!(observation.token_x_mint, TOKEN_X);
        assert_eq!(observation.token_y_mint, TOKEN_Y);
        assert_eq!(observation.reserve_x.as_deref(), Some(RESERVE_X));
        assert_eq!(observation.reserve_y.as_deref(), Some(RESERVE_Y));
        assert_eq!(observation.source, "block:initialize_lb_pair2");
        assert_eq!(observation.first_seen_slot, Some(421_651_686));
        assert_eq!(
            observation.first_seen_signature.as_deref(),
            Some("test-signature")
        );
    }

    #[test]
    fn observes_meteora_permission_pair_shifted_accounts() {
        let program_id = Pubkey::from_str(METEORA_DLMM_PROGRAM_ID).unwrap();
        let pool = Pubkey::from_str(POOL).unwrap();
        let token_x = Pubkey::from_str(TOKEN_X).unwrap();
        let token_y = Pubkey::from_str(TOKEN_Y).unwrap();
        let reserve_x = Pubkey::from_str(RESERVE_X).unwrap();
        let reserve_y = Pubkey::from_str(RESERVE_Y).unwrap();
        let tx = ResolvedTransaction {
            signature: "test-signature".to_owned(),
            primary_signer: None,
            account_keys: vec![
                program_id,
                Pubkey::new_unique(),
                pool,
                Pubkey::new_unique(),
                token_x,
                token_y,
                reserve_x,
                reserve_y,
            ],
            top_level_instructions: Vec::new(),
            inner_instruction_sets: Vec::new(),
            log_messages: Vec::new(),
        };
        let instruction = ResolvedInstruction {
            program_id_index: 0,
            accounts: vec![1, 2, 3, 4, 5, 6, 7],
            data: INITIALIZE_PERMISSION_LB_PAIR.to_vec(),
            stack_height: Some(1),
        };

        let observation =
            pool_observation_from_instruction(&tx, &instruction, 421_651_686, None, "0")
                .expect("pool observed");

        assert_eq!(observation.pool_address, POOL);
        assert_eq!(observation.token_x_mint, TOKEN_X);
        assert_eq!(observation.token_y_mint, TOKEN_Y);
        assert_eq!(observation.source, "block:initialize_permission_lb_pair");
    }

    fn write_pubkey(data: &mut [u8], offset: usize, pubkey: &str) {
        let pubkey = Pubkey::from_str(pubkey).unwrap();
        data[offset..offset + 32].copy_from_slice(&pubkey.to_bytes());
    }
}
