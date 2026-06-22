use crate::{
    context::SharedState,
    delivery,
    liquidity_pools::{
        LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED, LiquidityPoolLatestStateRecord,
        LiquidityPoolSnapshotRecord,
    },
    models::{ChainEvent, TelegramEventScope, event_kind, now_ts, target_type},
};
use anyhow::{Context, Result, bail};
use futures::{StreamExt, stream};
use serde::Deserialize;
use serde_json::{Value, json};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::Duration,
};
use tracing::{debug, warn};

pub(crate) const LOMBARD_LBTC_MINT: &str = "LBTCgU4b3wsFKsPwBn1rRZDx5DoFutM6RPiEt1TPDsY";
pub(crate) const LOMBARD_LBTC_SYMBOL: &str = "LBTC";

const DEXSCREENER_TOKEN_PAIRS_URL: &str = "https://api.dexscreener.com/token-pairs/v1/solana";
const RPC_SIGNATURE_PAGE_LIMIT: usize = 1_000;
const DEFAULT_TRANSACTION_CONCURRENCY: usize = 8;
const CRITICAL_POOL_WATCHED_VALUE_SHARE: f64 = 0.05;
const CRITICAL_RESERVE_THRESHOLD_RATIO: f64 = 0.5;

#[derive(Clone, Debug)]
pub(crate) struct TokenLiquidityScanOptions {
    pub(crate) mint: String,
    pub(crate) symbol: String,
    pub(crate) lookback_days: i64,
    pub(crate) min_token_reserve: Option<f64>,
    pub(crate) min_token_reserve_usd: Option<f64>,
    pub(crate) max_signature_pages: usize,
    pub(crate) wallet_addresses: Vec<String>,
    pub(crate) include_wallet_subscriptions: bool,
    pub(crate) emit_events: bool,
    pub(crate) enqueue_deliveries: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TokenLiquidityScanReport {
    pub(crate) token_mint: String,
    pub(crate) token_symbol: String,
    pub(crate) since_ts: i64,
    pub(crate) pools: Vec<TokenPoolLiquidityReport>,
    pub(crate) wallet_checks: Vec<TokenWalletHolding>,
    pub(crate) inserted_event_count: usize,
    pub(crate) delivered_event_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct TokenPoolLiquidityReport {
    pub(crate) dex_id: String,
    pub(crate) pair_address: String,
    pub(crate) pair_url: Option<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) base_token: MarketToken,
    pub(crate) quote_token: MarketToken,
    pub(crate) current_liquidity_usd: Option<f64>,
    pub(crate) current_watched_reserve: Option<f64>,
    pub(crate) current_watched_reserve_usd: Option<f64>,
    pub(crate) latest_snapshot: Option<PoolLiquiditySnapshot>,
    pub(crate) first_low_snapshot: Option<PoolLiquiditySnapshot>,
    pub(crate) min_snapshot: Option<PoolLiquiditySnapshot>,
    pub(crate) low_transition: Option<PoolLiquidityTransition>,
    pub(crate) max_drop_transition: Option<PoolLiquidityTransition>,
    pub(crate) buy_pressure: Option<PoolBuyPressureReport>,
    pub(crate) signature_count: usize,
    pub(crate) successful_signature_count: usize,
    pub(crate) fetched_transaction_count: usize,
    pub(crate) failed_transaction_count: usize,
    pub(crate) snapshot_count: usize,
}

impl TokenPoolLiquidityReport {
    pub(crate) fn watched_token_is_base(&self, mint: &str) -> bool {
        self.base_token.address == mint
    }

    pub(crate) fn watched_token(&self, mint: &str) -> &MarketToken {
        if self.watched_token_is_base(mint) {
            &self.base_token
        } else {
            &self.quote_token
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MarketToken {
    pub(crate) address: String,
    pub(crate) symbol: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PoolLiquiditySnapshot {
    pub(crate) signature: String,
    pub(crate) slot: i64,
    pub(crate) block_time: Option<i64>,
    pub(crate) base_reserve: Option<f64>,
    pub(crate) quote_reserve: Option<f64>,
    pub(crate) watched_reserve: f64,
    pub(crate) watched_reserve_usd: Option<f64>,
    pub(crate) estimated_liquidity_usd: Option<f64>,
    pub(crate) base_vault: Option<String>,
    pub(crate) quote_vault: Option<String>,
    pub(crate) signers: Vec<String>,
    pub(crate) token_owner_deltas: Vec<TokenOwnerDelta>,
}

#[derive(Clone, Debug)]
pub(crate) struct PoolLiquidityTransition {
    pub(crate) previous: PoolLiquiditySnapshot,
    pub(crate) current: PoolLiquiditySnapshot,
    pub(crate) reserve_delta: f64,
    pub(crate) reserve_delta_ratio: Option<f64>,
    pub(crate) reserve_usd_delta: Option<f64>,
    pub(crate) liquidity_usd_delta: Option<f64>,
    pub(crate) counter_reserve_delta: Option<f64>,
    pub(crate) inferred_counter_per_watched: Option<f64>,
    pub(crate) watched_token_buyers: Vec<TokenOwnerDelta>,
    pub(crate) counter_token_payers: Vec<TokenOwnerDelta>,
}

#[derive(Clone, Debug)]
pub(crate) struct TokenOwnerDelta {
    pub(crate) owner: String,
    pub(crate) mint: String,
    pub(crate) amount_delta: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct PoolBuyPressureReport {
    pub(crate) trade_count: usize,
    pub(crate) unique_buyer_count: usize,
    pub(crate) watched_token_out: f64,
    pub(crate) counter_token_in: f64,
    pub(crate) average_counter_per_watched: Option<f64>,
    pub(crate) max_counter_per_watched: Option<f64>,
    pub(crate) top_buyers: Vec<PoolBuyPressureBuyer>,
}

#[derive(Clone, Debug)]
pub(crate) struct PoolBuyPressureBuyer {
    pub(crate) owner: String,
    pub(crate) trade_count: usize,
    pub(crate) watched_token_received: f64,
    pub(crate) counter_token_paid: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct TokenWalletHolding {
    pub(crate) wallet_address: String,
    pub(crate) token_amount: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DexScreenerPair {
    chain_id: String,
    dex_id: String,
    url: Option<String>,
    pair_address: String,
    labels: Option<Vec<String>>,
    base_token: DexScreenerToken,
    quote_token: DexScreenerToken,
    price_native: Option<String>,
    price_usd: Option<String>,
    liquidity: Option<DexScreenerLiquidity>,
}

#[derive(Clone, Debug, Deserialize)]
struct DexScreenerToken {
    address: String,
    name: String,
    symbol: String,
}

#[derive(Clone, Debug, Deserialize)]
struct DexScreenerLiquidity {
    usd: Option<f64>,
    base: Option<f64>,
    quote: Option<f64>,
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
    err: Option<Value>,
    block_time: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RpcTransactionResponse {
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcTokenAccountsResponse {
    result: Option<RpcTokenAccountsResult>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcTokenAccountsResult {
    value: Vec<RpcTokenAccount>,
}

#[derive(Debug, Deserialize)]
struct RpcTokenAccount {
    account: RpcTokenAccountData,
}

#[derive(Debug, Deserialize)]
struct RpcTokenAccountData {
    data: Value,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Clone, Debug)]
struct TokenPoolMarket {
    dex_id: String,
    pair_address: String,
    pair_url: Option<String>,
    labels: Vec<String>,
    base_token: MarketToken,
    quote_token: MarketToken,
    price_native: Option<f64>,
    base_price_usd: Option<f64>,
    current_liquidity_usd: Option<f64>,
    current_base_reserve: Option<f64>,
    current_quote_reserve: Option<f64>,
}

impl TokenPoolMarket {
    fn watched_reserve(&self, mint: &str) -> Option<f64> {
        if self.base_token.address == mint {
            self.current_base_reserve
        } else if self.quote_token.address == mint {
            self.current_quote_reserve
        } else {
            None
        }
    }

    fn watched_price_usd(&self, mint: &str) -> Option<f64> {
        if self.base_token.address == mint {
            self.base_price_usd
        } else if self.quote_token.address == mint {
            self.quote_price_usd()
        } else {
            None
        }
    }

    fn quote_price_usd(&self) -> Option<f64> {
        let base_price_usd = self.base_price_usd?;
        let price_native = self.price_native?;
        if price_native <= 0.0 {
            return None;
        }
        Some(base_price_usd / price_native)
    }
}

pub(crate) async fn scan_token_liquidity(
    state: SharedState,
    options: TokenLiquidityScanOptions,
) -> Result<TokenLiquidityScanReport> {
    let mint = Pubkey::from_str(&options.mint)
        .with_context(|| format!("invalid token mint {}", options.mint))?
        .to_string();
    if options.lookback_days <= 0 {
        bail!("token liquidity lookback days must be greater than zero");
    }
    if options.max_signature_pages == 0 {
        bail!("token liquidity max signature pages must be greater than zero");
    }

    let since_ts = now_ts().saturating_sub(options.lookback_days.saturating_mul(86_400));
    let markets = discover_solana_token_markets(&state, &mint).await?;
    let wallet_addresses = token_liquidity_wallet_addresses(&state, &options).await?;
    let wallet_checks = find_wallet_token_holdings(&state, &mint, &wallet_addresses).await?;

    let mut report = TokenLiquidityScanReport {
        token_mint: mint.clone(),
        token_symbol: options.symbol.clone(),
        since_ts,
        wallet_checks,
        ..Default::default()
    };

    for market in markets {
        let pool_report =
            scan_pool_liquidity_history(&state, &market, &mint, since_ts, &options).await?;
        report.pools.push(pool_report);
    }

    if options.emit_events {
        let mut inserted = 0usize;
        let mut delivered = 0usize;
        for holding in &report.wallet_checks {
            for pool in &report.pools {
                let Some(latest_snapshot) = pool.latest_snapshot.as_ref() else {
                    continue;
                };
                if !snapshot_is_low(latest_snapshot, &options) {
                    continue;
                }
                let trigger_snapshot = pool
                    .low_transition
                    .as_ref()
                    .map(|transition| &transition.current)
                    .or(pool.first_low_snapshot.as_ref())
                    .unwrap_or(latest_snapshot);
                let event = build_low_liquidity_event(
                    &options,
                    &report,
                    pool,
                    trigger_snapshot,
                    latest_snapshot,
                    holding,
                );
                if state.db.insert_event(&event).await? {
                    inserted += 1;
                    if options.enqueue_deliveries {
                        delivery::enqueue_telegram_deliveries(state.clone(), &event).await?;
                        delivered += 1;
                    }
                }
            }
        }
        report.inserted_event_count = inserted;
        report.delivered_event_count = delivered;
    }

    Ok(report)
}

async fn discover_solana_token_markets(
    state: &SharedState,
    mint: &str,
) -> Result<Vec<TokenPoolMarket>> {
    let url = format!("{DEXSCREENER_TOKEN_PAIRS_URL}/{mint}");
    let pairs = state
        .http_client
        .get(&url)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .context("fetch DexScreener token markets")?
        .error_for_status()
        .context("DexScreener token markets endpoint returned an HTTP error")?
        .json::<Vec<DexScreenerPair>>()
        .await
        .context("decode DexScreener token markets")?;

    let mut seen = HashSet::new();
    let markets = pairs
        .into_iter()
        .filter(|pair| pair.chain_id == "solana")
        .filter(|pair| pair.base_token.address == mint || pair.quote_token.address == mint)
        .filter(|pair| seen.insert(pair.pair_address.clone()))
        .map(|pair| TokenPoolMarket {
            dex_id: pair.dex_id,
            pair_address: pair.pair_address,
            pair_url: pair.url,
            labels: pair.labels.unwrap_or_default(),
            base_token: MarketToken {
                address: pair.base_token.address,
                symbol: pair.base_token.symbol,
                name: pair.base_token.name,
            },
            quote_token: MarketToken {
                address: pair.quote_token.address,
                symbol: pair.quote_token.symbol,
                name: pair.quote_token.name,
            },
            price_native: pair
                .price_native
                .as_deref()
                .and_then(|value| value.parse::<f64>().ok()),
            base_price_usd: pair
                .price_usd
                .as_deref()
                .and_then(|value| value.parse::<f64>().ok()),
            current_liquidity_usd: pair.liquidity.as_ref().and_then(|value| value.usd),
            current_base_reserve: pair.liquidity.as_ref().and_then(|value| value.base),
            current_quote_reserve: pair.liquidity.as_ref().and_then(|value| value.quote),
        })
        .collect::<Vec<_>>();

    Ok(markets)
}

async fn token_liquidity_wallet_addresses(
    state: &SharedState,
    options: &TokenLiquidityScanOptions,
) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut wallets = Vec::new();

    for wallet in &options.wallet_addresses {
        let wallet = Pubkey::from_str(wallet)
            .with_context(|| format!("invalid wallet address {wallet}"))?
            .to_string();
        if seen.insert(wallet.clone()) {
            wallets.push(wallet);
        }
    }

    if options.include_wallet_subscriptions {
        for subscription in state.db.list_telegram_subscriptions().await? {
            if subscription.status != "active"
                || subscription.scope != TelegramEventScope::WalletActivity
            {
                continue;
            }
            let Some(target_address) = subscription.target_address.as_deref() else {
                continue;
            };
            let Ok(wallet) = Pubkey::from_str(target_address).map(|value| value.to_string()) else {
                continue;
            };
            if seen.insert(wallet.clone()) {
                wallets.push(wallet);
            }
        }
    }

    Ok(wallets)
}

async fn find_wallet_token_holdings(
    state: &SharedState,
    mint: &str,
    wallet_addresses: &[String],
) -> Result<Vec<TokenWalletHolding>> {
    let mut holdings = Vec::new();

    for wallet in wallet_addresses {
        let amount = wallet_token_amount(state, wallet, mint).await?;
        if amount > 0.0 {
            holdings.push(TokenWalletHolding {
                wallet_address: wallet.clone(),
                token_amount: amount,
            });
        }
    }

    Ok(holdings)
}

async fn wallet_token_amount(state: &SharedState, wallet: &str, mint: &str) -> Result<f64> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": format!("wallet-token-holding:{wallet}:{mint}"),
        "method": "getTokenAccountsByOwner",
        "params": [
            wallet,
            {
                "mint": mint
            },
            {
                "commitment": "finalized",
                "encoding": "jsonParsed"
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
        .with_context(|| format!("fetch token accounts for wallet {wallet}"))?
        .error_for_status()
        .with_context(|| format!("RPC getTokenAccountsByOwner HTTP error for wallet {wallet}"))?
        .json::<RpcTokenAccountsResponse>()
        .await
        .with_context(|| format!("decode token accounts for wallet {wallet}"))?;

    if let Some(error) = response.error {
        bail!(
            "RPC getTokenAccountsByOwner returned error {} for {}: {}",
            error.code,
            wallet,
            error.message
        );
    }

    let amount = response
        .result
        .map(|result| {
            result
                .value
                .iter()
                .filter_map(|account| {
                    account
                        .account
                        .data
                        .pointer("/parsed/info/tokenAmount")
                        .and_then(token_amount_value)
                })
                .sum::<f64>()
        })
        .unwrap_or(0.0);

    Ok(amount)
}

async fn scan_pool_liquidity_history(
    state: &SharedState,
    market: &TokenPoolMarket,
    mint: &str,
    since_ts: i64,
    options: &TokenLiquidityScanOptions,
) -> Result<TokenPoolLiquidityReport> {
    let signatures = fetch_pool_signatures(state, &market.pair_address, since_ts, options).await?;
    let successful_signatures = signatures
        .iter()
        .filter(|item| item.err.is_none())
        .filter(|item| {
            item.block_time
                .is_none_or(|block_time| block_time >= since_ts)
        })
        .cloned()
        .collect::<Vec<_>>();

    let snapshots =
        stream::iter(successful_signatures.clone())
            .map(|signature| {
                let state = state.clone();
                let market = market.clone();
                let mint = mint.to_owned();
                async move {
                    fetch_pool_snapshot_for_signature(&state, &market, &mint, &signature).await
                }
            })
            .buffer_unordered(DEFAULT_TRANSACTION_CONCURRENCY)
            .collect::<Vec<_>>()
            .await;

    let fetched_transaction_count = snapshots
        .iter()
        .filter(|result| matches!(result, Ok(Some(_))))
        .count();
    let failed_transaction_count = snapshots.iter().filter(|result| result.is_err()).count();
    for result in snapshots.iter().filter_map(|result| result.as_ref().err()) {
        warn!(
            pool = %market.pair_address,
            error = ?result,
            "failed to fetch token liquidity transaction snapshot",
        );
    }

    let mut snapshots = snapshots
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .collect::<Vec<_>>();
    snapshots.sort_by_key(|snapshot| (snapshot.slot, snapshot.signature.clone()));
    persist_pool_liquidity_snapshots(state, market, &snapshots)
        .await
        .with_context(|| {
            format!(
                "persist liquidity snapshots for pool {}",
                market.pair_address
            )
        })?;

    let latest_snapshot = snapshots.last().cloned();
    let first_low_snapshot = snapshots
        .iter()
        .find(|snapshot| snapshot_is_low(snapshot, options))
        .cloned();
    let min_snapshot = snapshots
        .iter()
        .min_by(|left, right| {
            left.watched_reserve
                .partial_cmp(&right.watched_reserve)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let watched_token_is_base = market.base_token.address == mint;
    let watched_mint = if watched_token_is_base {
        market.base_token.address.as_str()
    } else {
        market.quote_token.address.as_str()
    };
    let counter_mint = if watched_token_is_base {
        market.quote_token.address.as_str()
    } else {
        market.base_token.address.as_str()
    };
    let transitions = snapshots
        .windows(2)
        .map(|pair| {
            pool_liquidity_transition(
                &pair[0],
                &pair[1],
                watched_token_is_base,
                watched_mint,
                counter_mint,
            )
        })
        .collect::<Vec<_>>();
    let low_transition = transitions
        .iter()
        .find(|transition| {
            !snapshot_is_low(&transition.previous, options)
                && snapshot_is_low(&transition.current, options)
        })
        .cloned();
    let max_drop_transition = transitions
        .iter()
        .filter(|transition| transition.reserve_delta < 0.0)
        .min_by(|left, right| {
            left.reserve_delta
                .partial_cmp(&right.reserve_delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let buy_pressure = pool_buy_pressure_report(&transitions);

    let current_watched_reserve = latest_snapshot
        .as_ref()
        .map(|snapshot| snapshot.watched_reserve)
        .or_else(|| market.watched_reserve(mint));
    let current_watched_reserve_usd = latest_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.watched_reserve_usd)
        .or_else(|| Some(current_watched_reserve? * market.watched_price_usd(mint)?));

    Ok(TokenPoolLiquidityReport {
        dex_id: market.dex_id.clone(),
        pair_address: market.pair_address.clone(),
        pair_url: market.pair_url.clone(),
        labels: market.labels.clone(),
        base_token: market.base_token.clone(),
        quote_token: market.quote_token.clone(),
        current_liquidity_usd: latest_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.estimated_liquidity_usd)
            .or(market.current_liquidity_usd),
        current_watched_reserve,
        current_watched_reserve_usd,
        latest_snapshot,
        first_low_snapshot,
        min_snapshot,
        low_transition,
        max_drop_transition,
        buy_pressure,
        signature_count: signatures.len(),
        successful_signature_count: successful_signatures.len(),
        fetched_transaction_count,
        failed_transaction_count,
        snapshot_count: snapshots.len(),
    })
}

async fn fetch_pool_signatures(
    state: &SharedState,
    pair_address: &str,
    since_ts: i64,
    options: &TokenLiquidityScanOptions,
) -> Result<Vec<RpcSignatureInfo>> {
    let mut signatures = Vec::new();
    let mut before_signature = None::<String>;

    for page_index in 0..options.max_signature_pages {
        let mut config = json!({
            "commitment": "finalized",
            "limit": RPC_SIGNATURE_PAGE_LIMIT
        });
        if let Some(before_signature) = before_signature.as_deref() {
            config["before"] = json!(before_signature);
        }

        let request = json!({
            "jsonrpc": "2.0",
            "id": format!("token-liquidity-signatures:{pair_address}:{page_index}"),
            "method": "getSignaturesForAddress",
            "params": [
                pair_address,
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
            .with_context(|| format!("fetch signatures for pool {pair_address}"))?
            .error_for_status()
            .with_context(|| format!("RPC getSignaturesForAddress HTTP error for {pair_address}"))?
            .json::<RpcSignaturesResponse>()
            .await
            .with_context(|| format!("decode signatures for pool {pair_address}"))?;

        if let Some(error) = response.error {
            bail!(
                "RPC getSignaturesForAddress returned error {} for {}: {}",
                error.code,
                pair_address,
                error.message
            );
        }

        let page = response.result.unwrap_or_default();
        if page.is_empty() {
            break;
        }

        let reached_lookback = page
            .last()
            .and_then(|item| item.block_time)
            .is_some_and(|block_time| block_time < since_ts);
        before_signature = page.last().map(|item| item.signature.clone());
        signatures.extend(page);
        if reached_lookback {
            break;
        }
    }

    Ok(signatures)
}

async fn fetch_pool_snapshot_for_signature(
    state: &SharedState,
    market: &TokenPoolMarket,
    mint: &str,
    signature: &RpcSignatureInfo,
) -> Result<Option<PoolLiquiditySnapshot>> {
    let request = json!({
        "jsonrpc": "2.0",
        "id": format!("token-liquidity-transaction:{}", signature.signature),
        "method": "getTransaction",
        "params": [
            signature.signature,
            {
                "encoding": "jsonParsed",
                "commitment": "finalized",
                "maxSupportedTransactionVersion": 0
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
        .with_context(|| format!("fetch transaction {}", signature.signature))?
        .error_for_status()
        .with_context(|| format!("RPC getTransaction HTTP error for {}", signature.signature))?
        .json::<RpcTransactionResponse>()
        .await
        .with_context(|| format!("decode transaction {}", signature.signature))?;

    if let Some(error) = response.error {
        bail!(
            "RPC getTransaction returned error {} for {}: {}",
            error.code,
            signature.signature,
            error.message
        );
    }

    let Some(transaction) = response.result else {
        debug!(signature = %signature.signature, "transaction not available for liquidity scan");
        return Ok(None);
    };

    if !transaction
        .pointer("/meta/err")
        .is_some_and(serde_json::Value::is_null)
    {
        return Ok(None);
    }

    Ok(pool_snapshot_from_transaction(
        market,
        mint,
        &signature.signature,
        &transaction,
    ))
}

fn pool_snapshot_from_transaction(
    market: &TokenPoolMarket,
    mint: &str,
    signature: &str,
    transaction: &Value,
) -> Option<PoolLiquiditySnapshot> {
    let slot = transaction.get("slot")?.as_i64()?;
    let block_time = transaction.get("blockTime").and_then(Value::as_i64);
    let account_keys = transaction_account_keys(transaction);
    let signers = transaction_signers(transaction);

    let mut balances = HashMap::<String, PoolOwnedTokenBalance>::new();
    for balance in transaction
        .pointer("/meta/postTokenBalances")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if balance.get("owner").and_then(Value::as_str) != Some(market.pair_address.as_str()) {
            continue;
        }
        let Some(mint) = balance.get("mint").and_then(Value::as_str) else {
            continue;
        };
        if mint != market.base_token.address && mint != market.quote_token.address {
            continue;
        }
        let Some(amount) = balance
            .pointer("/uiTokenAmount")
            .and_then(token_amount_value)
        else {
            continue;
        };
        let account_index = balance.get("accountIndex").and_then(Value::as_u64);
        let vault = account_index
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| account_keys.get(index).cloned());
        let entry = balances
            .entry(mint.to_owned())
            .or_insert_with(|| PoolOwnedTokenBalance {
                amount,
                vault: vault.clone(),
            });
        if amount > entry.amount {
            entry.amount = amount;
            entry.vault = vault;
        }
    }

    let base = balances.get(&market.base_token.address);
    let quote = balances.get(&market.quote_token.address);
    let base_reserve = base.map(|balance| balance.amount);
    let quote_reserve = quote.map(|balance| balance.amount);
    let watched_reserve = if mint == market.base_token.address {
        base_reserve?
    } else if mint == market.quote_token.address {
        quote_reserve?
    } else {
        return None;
    };
    let watched_reserve_usd = Some(watched_reserve * market.watched_price_usd(mint)?);
    let estimated_liquidity_usd = match (base_reserve, quote_reserve) {
        (Some(base_reserve), Some(quote_reserve)) => {
            let base_price = market.base_price_usd?;
            let quote_price = market.quote_price_usd()?;
            Some(base_reserve.mul_add(base_price, quote_reserve * quote_price))
        }
        _ => None,
    };

    Some(PoolLiquiditySnapshot {
        signature: signature.to_owned(),
        slot,
        block_time,
        base_reserve,
        quote_reserve,
        watched_reserve,
        watched_reserve_usd,
        estimated_liquidity_usd,
        base_vault: base.and_then(|balance| balance.vault.clone()),
        quote_vault: quote.and_then(|balance| balance.vault.clone()),
        signers,
        token_owner_deltas: transaction_token_owner_deltas(
            transaction,
            &market.base_token.address,
            &market.quote_token.address,
            &market.pair_address,
        ),
    })
}

async fn persist_pool_liquidity_snapshots(
    state: &SharedState,
    market: &TokenPoolMarket,
    snapshots: &[PoolLiquiditySnapshot],
) -> Result<()> {
    let now = now_ts();
    for snapshot in snapshots {
        let record = liquidity_pool_snapshot_record(market, snapshot, now);
        state
            .db
            .upsert_liquidity_pool_snapshot(&record)
            .await
            .context("upsert liquidity pool snapshot")?;
    }

    if let Some(latest_snapshot) = snapshots.last() {
        let latest_state = liquidity_pool_latest_state_record(market, latest_snapshot, now);
        state
            .db
            .upsert_liquidity_pool_latest_state(&latest_state)
            .await
            .context("upsert liquidity pool latest state")?;
    }

    Ok(())
}

fn liquidity_pool_snapshot_record(
    market: &TokenPoolMarket,
    snapshot: &PoolLiquiditySnapshot,
    now: i64,
) -> LiquidityPoolSnapshotRecord {
    LiquidityPoolSnapshotRecord {
        id: format!("{}:{}", market.pair_address, snapshot.signature),
        pool_address: market.pair_address.clone(),
        dex_id: market.dex_id.clone(),
        base_token_mint: market.base_token.address.clone(),
        quote_token_mint: market.quote_token.address.clone(),
        signature: snapshot.signature.clone(),
        slot: snapshot.slot,
        block_time: snapshot.block_time,
        base_reserve: snapshot.base_reserve,
        quote_reserve: snapshot.quote_reserve,
        base_vault: snapshot.base_vault.clone(),
        quote_vault: snapshot.quote_vault.clone(),
        estimated_liquidity_usd: snapshot.estimated_liquidity_usd,
        retention_class: LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED.to_owned(),
        observed_at: snapshot.block_time.unwrap_or(now),
    }
}

fn liquidity_pool_latest_state_record(
    market: &TokenPoolMarket,
    snapshot: &PoolLiquiditySnapshot,
    now: i64,
) -> LiquidityPoolLatestStateRecord {
    LiquidityPoolLatestStateRecord {
        pool_address: market.pair_address.clone(),
        dex_id: market.dex_id.clone(),
        base_token_mint: market.base_token.address.clone(),
        quote_token_mint: market.quote_token.address.clone(),
        base_reserve: snapshot.base_reserve,
        quote_reserve: snapshot.quote_reserve,
        base_vault: snapshot.base_vault.clone(),
        quote_vault: snapshot.quote_vault.clone(),
        estimated_liquidity_usd: snapshot.estimated_liquidity_usd,
        latest_signature: snapshot.signature.clone(),
        latest_slot: snapshot.slot,
        latest_block_time: snapshot.block_time,
        updated_at: now,
    }
}

#[derive(Clone, Debug)]
struct PoolOwnedTokenBalance {
    amount: f64,
    vault: Option<String>,
}

fn transaction_account_keys(transaction: &Value) -> Vec<String> {
    transaction
        .pointer("/transaction/message/accountKeys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| {
            account
                .as_str()
                .or_else(|| account.get("pubkey").and_then(Value::as_str))
        })
        .map(str::to_owned)
        .collect()
}

fn transaction_signers(transaction: &Value) -> Vec<String> {
    transaction
        .pointer("/transaction/message/accountKeys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|account| account.get("signer").and_then(Value::as_bool) == Some(true))
        .filter_map(|account| account.get("pubkey").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn transaction_token_owner_deltas(
    transaction: &Value,
    base_mint: &str,
    quote_mint: &str,
    pool_owner: &str,
) -> Vec<TokenOwnerDelta> {
    let pre = transaction_token_owner_amounts(
        transaction,
        "/meta/preTokenBalances",
        base_mint,
        quote_mint,
    );
    let post = transaction_token_owner_amounts(
        transaction,
        "/meta/postTokenBalances",
        base_mint,
        quote_mint,
    );
    let mut keys = pre.keys().cloned().collect::<HashSet<_>>();
    keys.extend(post.keys().cloned());

    let mut deltas = keys
        .into_iter()
        .filter_map(|(owner, mint)| {
            if owner == pool_owner {
                return None;
            }
            let amount_delta = post
                .get(&(owner.clone(), mint.clone()))
                .copied()
                .unwrap_or_default()
                - pre
                    .get(&(owner.clone(), mint.clone()))
                    .copied()
                    .unwrap_or_default();
            if amount_delta.abs() <= 0.000000001 {
                return None;
            }
            Some(TokenOwnerDelta {
                owner,
                mint,
                amount_delta,
            })
        })
        .collect::<Vec<_>>();
    deltas.sort_by(|left, right| {
        left.owner
            .cmp(&right.owner)
            .then_with(|| left.mint.cmp(&right.mint))
    });
    deltas
}

fn transaction_token_owner_amounts(
    transaction: &Value,
    pointer: &str,
    base_mint: &str,
    quote_mint: &str,
) -> HashMap<(String, String), f64> {
    let mut amounts = HashMap::new();
    for balance in transaction
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(owner) = balance.get("owner").and_then(Value::as_str) else {
            continue;
        };
        let Some(mint) = balance.get("mint").and_then(Value::as_str) else {
            continue;
        };
        if mint != base_mint && mint != quote_mint {
            continue;
        }
        let Some(amount) = balance
            .pointer("/uiTokenAmount")
            .and_then(token_amount_value)
        else {
            continue;
        };
        *amounts
            .entry((owner.to_owned(), mint.to_owned()))
            .or_insert(0.0) += amount;
    }
    amounts
}

fn token_amount_value(value: &Value) -> Option<f64> {
    value
        .get("uiAmountString")
        .and_then(Value::as_str)
        .or_else(|| value.get("amount").and_then(Value::as_str))
        .and_then(|amount| {
            if value.get("uiAmountString").is_some() {
                amount.parse::<f64>().ok()
            } else {
                let decimals = value.get("decimals").and_then(Value::as_u64)?;
                let raw = amount.parse::<f64>().ok()?;
                let scale = 10_f64.powi(i32::try_from(decimals).ok()?);
                Some(raw / scale)
            }
        })
}

fn snapshot_is_low(snapshot: &PoolLiquiditySnapshot, options: &TokenLiquidityScanOptions) -> bool {
    options
        .min_token_reserve
        .is_some_and(|threshold| snapshot.watched_reserve < threshold)
        || options.min_token_reserve_usd.is_some_and(|threshold| {
            snapshot
                .watched_reserve_usd
                .is_some_and(|value| value < threshold)
        })
}

fn pool_liquidity_transition(
    previous: &PoolLiquiditySnapshot,
    current: &PoolLiquiditySnapshot,
    watched_token_is_base: bool,
    watched_mint: &str,
    counter_mint: &str,
) -> PoolLiquidityTransition {
    let reserve_delta = current.watched_reserve - previous.watched_reserve;
    let base_delta = match (previous.base_reserve, current.base_reserve) {
        (Some(previous), Some(current)) => Some(current - previous),
        _ => None,
    };
    let quote_delta = match (previous.quote_reserve, current.quote_reserve) {
        (Some(previous), Some(current)) => Some(current - previous),
        _ => None,
    };
    let counter_reserve_delta = if watched_token_is_base {
        quote_delta
    } else {
        base_delta
    };
    let inferred_counter_per_watched = match (reserve_delta, counter_reserve_delta) {
        (watched_delta, Some(counter_delta)) if watched_delta < 0.0 && counter_delta > 0.0 => {
            Some(counter_delta / watched_delta.abs())
        }
        (watched_delta, Some(counter_delta)) if watched_delta > 0.0 && counter_delta < 0.0 => {
            Some(counter_delta.abs() / watched_delta)
        }
        _ => None,
    };
    let reserve_delta_ratio = if previous.watched_reserve.abs() > f64::EPSILON {
        Some(reserve_delta / previous.watched_reserve)
    } else {
        None
    };
    let reserve_usd_delta = match (previous.watched_reserve_usd, current.watched_reserve_usd) {
        (Some(previous), Some(current)) => Some(current - previous),
        _ => None,
    };
    let liquidity_usd_delta = match (
        previous.estimated_liquidity_usd,
        current.estimated_liquidity_usd,
    ) {
        (Some(previous), Some(current)) => Some(current - previous),
        _ => None,
    };
    let watched_token_buyers = current
        .token_owner_deltas
        .iter()
        .filter(|delta| delta.mint == watched_mint && delta.amount_delta > 0.0)
        .cloned()
        .collect::<Vec<_>>();
    let counter_token_payers = current
        .token_owner_deltas
        .iter()
        .filter(|delta| delta.mint == counter_mint && delta.amount_delta < 0.0)
        .cloned()
        .collect::<Vec<_>>();

    PoolLiquidityTransition {
        previous: previous.clone(),
        current: current.clone(),
        reserve_delta,
        reserve_delta_ratio,
        reserve_usd_delta,
        liquidity_usd_delta,
        counter_reserve_delta,
        inferred_counter_per_watched,
        watched_token_buyers,
        counter_token_payers,
    }
}

fn pool_buy_pressure_report(
    transitions: &[PoolLiquidityTransition],
) -> Option<PoolBuyPressureReport> {
    let mut trade_count = 0usize;
    let mut watched_token_out = 0.0;
    let mut counter_token_in = 0.0;
    let mut max_counter_per_watched = None::<f64>;
    let mut buyers = HashMap::<String, PoolBuyPressureBuyerAccumulator>::new();

    for transition in transitions {
        let Some(counter_delta) = transition.counter_reserve_delta else {
            continue;
        };
        if transition.reserve_delta >= 0.0 || counter_delta <= 0.0 {
            continue;
        }

        trade_count += 1;
        let watched_out = transition.reserve_delta.abs();
        watched_token_out += watched_out;
        counter_token_in += counter_delta;
        if let Some(price) = transition.inferred_counter_per_watched {
            max_counter_per_watched = Some(
                max_counter_per_watched
                    .map(|current| current.max(price))
                    .unwrap_or(price),
            );
        }

        let buyer_owners = if transition.watched_token_buyers.is_empty() {
            transition
                .current
                .signers
                .iter()
                .map(|owner| (owner.clone(), watched_out, counter_delta))
                .collect::<Vec<_>>()
        } else {
            let observed_watched_total = transition
                .watched_token_buyers
                .iter()
                .map(|buyer| buyer.amount_delta)
                .sum::<f64>();
            transition
                .watched_token_buyers
                .iter()
                .map(|buyer| {
                    let counter_paid = transition
                        .counter_token_payers
                        .iter()
                        .filter(|payer| payer.owner == buyer.owner)
                        .map(|payer| payer.amount_delta.abs())
                        .sum::<f64>();
                    (
                        buyer.owner.clone(),
                        buyer.amount_delta,
                        if counter_paid > 0.0 {
                            counter_paid
                        } else if observed_watched_total > 0.0 {
                            counter_delta * (buyer.amount_delta / observed_watched_total)
                        } else {
                            counter_delta
                        },
                    )
                })
                .collect::<Vec<_>>()
        };

        let split_count = buyer_owners.len().max(1) as f64;
        for (owner, watched_received, counter_paid) in buyer_owners {
            let entry =
                buyers
                    .entry(owner.clone())
                    .or_insert_with(|| PoolBuyPressureBuyerAccumulator {
                        owner,
                        ..Default::default()
                    });
            entry.trade_count += 1;
            entry.watched_token_received += if watched_received > 0.0 {
                watched_received
            } else {
                watched_out / split_count
            };
            entry.counter_token_paid += if counter_paid > 0.0 {
                counter_paid
            } else {
                counter_delta / split_count
            };
        }
    }

    if trade_count == 0 || watched_token_out <= 0.0 {
        return None;
    }

    let unique_buyer_count = buyers.len();
    let mut top_buyers = buyers
        .into_values()
        .map(|buyer| PoolBuyPressureBuyer {
            owner: buyer.owner,
            trade_count: buyer.trade_count,
            watched_token_received: buyer.watched_token_received,
            counter_token_paid: buyer.counter_token_paid,
        })
        .collect::<Vec<_>>();
    top_buyers.sort_by(|left, right| {
        right
            .watched_token_received
            .partial_cmp(&left.watched_token_received)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    top_buyers.truncate(5);

    Some(PoolBuyPressureReport {
        trade_count,
        unique_buyer_count,
        watched_token_out,
        counter_token_in,
        average_counter_per_watched: Some(counter_token_in / watched_token_out),
        max_counter_per_watched,
        top_buyers,
    })
}

#[derive(Default)]
struct PoolBuyPressureBuyerAccumulator {
    owner: String,
    trade_count: usize,
    watched_token_received: f64,
    counter_token_paid: f64,
}

fn critical_pool_state_reason(
    options: &TokenLiquidityScanOptions,
    trigger_snapshot: &PoolLiquiditySnapshot,
    latest_snapshot: &PoolLiquiditySnapshot,
    watched_symbol: &str,
) -> Option<String> {
    critical_snapshot_state_reason(options, latest_snapshot, watched_symbol).or_else(|| {
        if latest_snapshot.signature == trigger_snapshot.signature
            && latest_snapshot.slot == trigger_snapshot.slot
        {
            None
        } else {
            critical_snapshot_state_reason(options, trigger_snapshot, watched_symbol)
        }
    })
}

fn critical_snapshot_state_reason(
    options: &TokenLiquidityScanOptions,
    snapshot: &PoolLiquiditySnapshot,
    watched_symbol: &str,
) -> Option<String> {
    if let (Some(watched_reserve_usd), Some(estimated_liquidity_usd)) = (
        snapshot.watched_reserve_usd,
        snapshot.estimated_liquidity_usd,
    ) {
        if estimated_liquidity_usd > 0.0 {
            let watched_share = watched_reserve_usd / estimated_liquidity_usd;
            if watched_share <= CRITICAL_POOL_WATCHED_VALUE_SHARE {
                return Some(format!(
                    "critical pool imbalance: {} side is {:.2}% of estimated pool value",
                    watched_symbol,
                    watched_share * 100.0
                ));
            }
        }
    }

    if let Some(threshold) = options.min_token_reserve {
        if threshold > 0.0
            && snapshot.watched_reserve <= threshold * CRITICAL_RESERVE_THRESHOLD_RATIO
        {
            return Some(format!(
                "critical {} reserve: {} {} remains",
                watched_symbol,
                format_amount(snapshot.watched_reserve),
                watched_symbol
            ));
        }
    }

    if let (Some(reserve_usd), Some(threshold_usd)) =
        (snapshot.watched_reserve_usd, options.min_token_reserve_usd)
    {
        if threshold_usd > 0.0 && reserve_usd <= threshold_usd * CRITICAL_RESERVE_THRESHOLD_RATIO {
            return Some(format!(
                "critical {} reserve value: {} remains",
                watched_symbol,
                format_usd(reserve_usd)
            ));
        }
    }

    None
}

fn build_low_liquidity_event(
    options: &TokenLiquidityScanOptions,
    report: &TokenLiquidityScanReport,
    pool: &TokenPoolLiquidityReport,
    trigger_snapshot: &PoolLiquiditySnapshot,
    latest_snapshot: &PoolLiquiditySnapshot,
    holding: &TokenWalletHolding,
) -> ChainEvent {
    let watched_token = pool.watched_token(&report.token_mint);
    let reserve = format_amount(trigger_snapshot.watched_reserve);
    let reserve_usd = trigger_snapshot
        .watched_reserve_usd
        .map(format_usd)
        .unwrap_or_else(|| "unknown USD value".to_owned());
    let trigger_transition = pool.low_transition.as_ref().filter(|transition| {
        transition.current.signature == trigger_snapshot.signature
            && transition.current.slot == trigger_snapshot.slot
    });
    let critical_reason = critical_pool_state_reason(
        options,
        trigger_snapshot,
        latest_snapshot,
        &watched_token.symbol,
    );
    let severity = if critical_reason.is_some() {
        "critical"
    } else {
        "high"
    };
    let summary = if let Some(transition) = trigger_transition {
        let mut value = format!(
            "{} liquidity crossed low threshold on {} pool {}: {} {} available ({}) after a {} {} drop from previous snapshot",
            report.token_symbol,
            pool.dex_id,
            compact_address(&pool.pair_address),
            reserve,
            watched_token.symbol,
            reserve_usd,
            format_amount(transition.reserve_delta.abs()),
            watched_token.symbol
        );
        if let Some(reason) = critical_reason.as_deref() {
            value.push_str("; ");
            value.push_str(reason);
        }
        value
    } else {
        let mut value = format!(
            "{} liquidity is low on {} pool {}: {} {} available ({})",
            report.token_symbol,
            pool.dex_id,
            compact_address(&pool.pair_address),
            reserve,
            watched_token.symbol,
            reserve_usd
        );
        if let Some(reason) = critical_reason.as_deref() {
            value.push_str("; ");
            value.push_str(reason);
        }
        value
    };
    let threshold_key = token_liquidity_threshold_key(options);
    let mut tags = vec![
        "target:wallet".to_owned(),
        "watch:defi".to_owned(),
        "wallet:activity".to_owned(),
        "signal:wallet".to_owned(),
        "signal:low_liquidity".to_owned(),
        "sector:defi".to_owned(),
        format!("severity:{severity}"),
        format!("impact:{severity}"),
        format!("token:{}", report.token_symbol.to_ascii_lowercase()),
        format!("mint:{}", report.token_mint),
        format!("dex:{}", pool.dex_id.to_ascii_lowercase()),
    ];
    if critical_reason.is_some() {
        tags.push("signal:pool_imbalance".to_owned());
        tags.push("signal:critical_liquidity".to_owned());
    }

    ChainEvent {
        id: format!(
            "{}:{}:{}:{}:{}:{}",
            trigger_snapshot.signature,
            event_kind::TOKEN_LIQUIDITY_LOW,
            holding.wallet_address,
            report.token_mint,
            pool.pair_address,
            threshold_key
        ),
        signature: trigger_snapshot.signature.clone(),
        instruction_path: "liquidity".to_owned(),
        kind: event_kind::TOKEN_LIQUIDITY_LOW.to_owned(),
        target_type: target_type::WALLET.to_owned(),
        target_address: holding.wallet_address.clone(),
        actor: None,
        slot: trigger_snapshot.slot,
        block_time: trigger_snapshot.block_time,
        indexed_at: now_ts(),
        summary,
        tags,
        payload: json!({
            "classification": "token_liquidity_low",
            "severity": severity,
            "critical_reason": critical_reason,
            "wallet": holding.wallet_address,
            "wallet_token_balance": holding.token_amount,
            "token_symbol": report.token_symbol,
            "token_mint": report.token_mint,
            "token_name": watched_token.name,
            "dex_id": pool.dex_id,
            "pool_address": pool.pair_address,
            "pool_url": pool.pair_url,
            "pool_labels": pool.labels,
            "base_token": {
                "address": pool.base_token.address,
                "symbol": pool.base_token.symbol,
                "name": pool.base_token.name,
                "reserve": trigger_snapshot.base_reserve,
                "vault": trigger_snapshot.base_vault,
            },
            "quote_token": {
                "address": pool.quote_token.address,
                "symbol": pool.quote_token.symbol,
                "name": pool.quote_token.name,
                "reserve": trigger_snapshot.quote_reserve,
                "vault": trigger_snapshot.quote_vault,
            },
            "watched_token_reserve": trigger_snapshot.watched_reserve,
            "watched_token_reserve_usd": trigger_snapshot.watched_reserve_usd,
            "watched_token_reserve_delta": trigger_transition.map(|transition| transition.reserve_delta),
            "watched_token_reserve_delta_ratio": trigger_transition.and_then(|transition| transition.reserve_delta_ratio),
            "watched_token_reserve_usd_delta": trigger_transition.and_then(|transition| transition.reserve_usd_delta),
            "estimated_liquidity_usd": trigger_snapshot.estimated_liquidity_usd,
            "estimated_liquidity_usd_delta": trigger_transition.and_then(|transition| transition.liquidity_usd_delta),
            "current_liquidity_usd": pool.current_liquidity_usd,
            "latest_watched_token_reserve": latest_snapshot.watched_reserve,
            "latest_watched_token_reserve_usd": latest_snapshot.watched_reserve_usd,
            "latest_estimated_liquidity_usd": latest_snapshot.estimated_liquidity_usd,
            "min_token_reserve": options.min_token_reserve,
            "min_token_reserve_usd": options.min_token_reserve_usd,
            "threshold_key": threshold_key,
            "trigger": snapshot_payload(trigger_snapshot),
            "latest": snapshot_payload(latest_snapshot),
            "low_since": pool.first_low_snapshot.as_ref().map(snapshot_payload),
            "low_transition": pool.low_transition.as_ref().map(transition_payload),
            "min_observed": pool.min_snapshot.as_ref().map(snapshot_payload),
            "max_observed_drop": pool.max_drop_transition.as_ref().map(transition_payload),
            "buy_pressure": pool.buy_pressure.as_ref().map(buy_pressure_payload),
            "lookback_since_ts": report.since_ts,
            "history_method": "getSignaturesForAddress/getTransaction",
            "market_source": "dexscreener",
        }),
        delivery_attempts: 0,
        delivered_at: None,
        last_error: None,
        next_retry_at: now_ts(),
    }
}

fn token_liquidity_threshold_key(options: &TokenLiquidityScanOptions) -> String {
    let reserve = options
        .min_token_reserve
        .map(|value| format!("{value:.12}"))
        .unwrap_or_else(|| "none".to_owned());
    let reserve_usd = options
        .min_token_reserve_usd
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "none".to_owned());
    format!("reserve={reserve}:reserve_usd={reserve_usd}")
}

fn snapshot_payload(snapshot: &PoolLiquiditySnapshot) -> Value {
    json!({
        "signature": snapshot.signature,
        "slot": snapshot.slot,
        "block_time": snapshot.block_time,
        "watched_token_reserve": snapshot.watched_reserve,
        "watched_token_reserve_usd": snapshot.watched_reserve_usd,
        "estimated_liquidity_usd": snapshot.estimated_liquidity_usd,
        "signers": &snapshot.signers,
    })
}

fn transition_payload(transition: &PoolLiquidityTransition) -> Value {
    json!({
        "previous": snapshot_payload(&transition.previous),
        "current": snapshot_payload(&transition.current),
        "reserve_delta": transition.reserve_delta,
        "reserve_delta_ratio": transition.reserve_delta_ratio,
        "reserve_usd_delta": transition.reserve_usd_delta,
        "liquidity_usd_delta": transition.liquidity_usd_delta,
        "counter_reserve_delta": transition.counter_reserve_delta,
        "inferred_counter_per_watched": transition.inferred_counter_per_watched,
        "watched_token_buyers": transition
            .watched_token_buyers
            .iter()
            .map(token_owner_delta_payload)
            .collect::<Vec<_>>(),
        "counter_token_payers": transition
            .counter_token_payers
            .iter()
            .map(token_owner_delta_payload)
            .collect::<Vec<_>>(),
    })
}

fn token_owner_delta_payload(delta: &TokenOwnerDelta) -> Value {
    json!({
        "owner": &delta.owner,
        "mint": &delta.mint,
        "amount_delta": delta.amount_delta,
    })
}

fn buy_pressure_payload(report: &PoolBuyPressureReport) -> Value {
    json!({
        "trade_count": report.trade_count,
        "unique_buyer_count": report.unique_buyer_count,
        "watched_token_out": report.watched_token_out,
        "counter_token_in": report.counter_token_in,
        "average_counter_per_watched": report.average_counter_per_watched,
        "max_counter_per_watched": report.max_counter_per_watched,
        "top_buyers": report
            .top_buyers
            .iter()
            .map(|buyer| {
                json!({
                    "owner": &buyer.owner,
                    "trade_count": buyer.trade_count,
                    "watched_token_received": buyer.watched_token_received,
                    "counter_token_paid": buyer.counter_token_paid,
                })
            })
            .collect::<Vec<_>>(),
    })
}

pub(crate) fn format_amount(value: f64) -> String {
    if value >= 1_000.0 {
        format!("{value:.2}")
    } else if value >= 1.0 {
        format!("{value:.6}")
    } else {
        format!("{value:.8}")
    }
}

pub(crate) fn format_usd(value: f64) -> String {
    if value >= 1_000_000.0 {
        format!("${:.2}M", value / 1_000_000.0)
    } else if value >= 1_000.0 {
        format!("${:.1}K", value / 1_000.0)
    } else {
        format!("${value:.2}")
    }
}

fn compact_address(address: &str) -> String {
    if address.len() <= 12 {
        return address.to_owned();
    }
    format!("{}...{}", &address[..6], &address[address.len() - 6..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_market() -> TokenPoolMarket {
        TokenPoolMarket {
            dex_id: "meteora".to_owned(),
            pair_address: "Pool111111111111111111111111111111111111111".to_owned(),
            pair_url: None,
            labels: vec!["DLMM".to_owned()],
            base_token: MarketToken {
                address: LOMBARD_LBTC_MINT.to_owned(),
                symbol: "LBTC".to_owned(),
                name: "Lombard Staked BTC".to_owned(),
            },
            quote_token: MarketToken {
                address: "cbbtc111111111111111111111111111111111111".to_owned(),
                symbol: "cbBTC".to_owned(),
                name: "Coinbase Wrapped BTC".to_owned(),
            },
            price_native: Some(1.01),
            base_price_usd: Some(65_000.0),
            current_liquidity_usd: None,
            current_base_reserve: None,
            current_quote_reserve: None,
        }
    }

    fn sample_options() -> TokenLiquidityScanOptions {
        TokenLiquidityScanOptions {
            mint: LOMBARD_LBTC_MINT.to_owned(),
            symbol: LOMBARD_LBTC_SYMBOL.to_owned(),
            lookback_days: 30,
            min_token_reserve: Some(0.1),
            min_token_reserve_usd: Some(10_000.0),
            max_signature_pages: 1,
            wallet_addresses: Vec::new(),
            include_wallet_subscriptions: false,
            emit_events: false,
            enqueue_deliveries: false,
        }
    }

    fn sample_snapshot(signature: &str, slot: i64, watched_reserve: f64) -> PoolLiquiditySnapshot {
        PoolLiquiditySnapshot {
            signature: signature.to_owned(),
            slot,
            block_time: Some(1_700_000_000 + slot),
            base_reserve: Some(watched_reserve),
            quote_reserve: Some(45.0 - watched_reserve),
            watched_reserve,
            watched_reserve_usd: Some(watched_reserve * 65_000.0),
            estimated_liquidity_usd: Some(2_900_000.0),
            base_vault: Some("baseVault".to_owned()),
            quote_vault: Some("quoteVault".to_owned()),
            signers: vec!["buyer".to_owned()],
            token_owner_deltas: vec![
                TokenOwnerDelta {
                    owner: "buyer".to_owned(),
                    mint: LOMBARD_LBTC_MINT.to_owned(),
                    amount_delta: watched_reserve,
                },
                TokenOwnerDelta {
                    owner: "buyer".to_owned(),
                    mint: "cbbtc111111111111111111111111111111111111".to_owned(),
                    amount_delta: -watched_reserve,
                },
            ],
        }
    }

    #[test]
    fn parses_pool_owned_post_token_balances() {
        let market = sample_market();
        let transaction = json!({
            "slot": 427897279,
            "blockTime": 1782027490,
            "meta": {
                "err": null,
                "postTokenBalances": [
                    {
                        "accountIndex": 2,
                        "mint": LOMBARD_LBTC_MINT,
                        "owner": "SomeoneElse11111111111111111111111111111111",
                        "uiTokenAmount": { "uiAmountString": "10", "decimals": 8, "amount": "1000000000" }
                    },
                    {
                        "accountIndex": 3,
                        "mint": LOMBARD_LBTC_MINT,
                        "owner": market.pair_address,
                        "uiTokenAmount": { "uiAmountString": "0.04230277", "decimals": 8, "amount": "4230277" }
                    },
                    {
                        "accountIndex": 4,
                        "mint": market.quote_token.address,
                        "owner": market.pair_address,
                        "uiTokenAmount": { "uiAmountString": "45.08757795", "decimals": 8, "amount": "4508757795" }
                    }
                ]
            },
            "transaction": {
                "message": {
                    "accountKeys": [
                        {"pubkey": "payer"},
                        {"pubkey": market.pair_address},
                        {"pubkey": "userVault"},
                        {"pubkey": "baseVault"},
                        {"pubkey": "quoteVault"}
                    ]
                }
            }
        });

        let snapshot = pool_snapshot_from_transaction(
            &market,
            LOMBARD_LBTC_MINT,
            "signature111",
            &transaction,
        )
        .expect("snapshot");

        assert_eq!(snapshot.slot, 427897279);
        assert_eq!(snapshot.watched_reserve, 0.04230277);
        assert_eq!(snapshot.base_vault.as_deref(), Some("baseVault"));
        assert_eq!(snapshot.quote_vault.as_deref(), Some("quoteVault"));
        assert!(snapshot.signers.is_empty());
        assert!(snapshot.watched_reserve_usd.unwrap() > 2_700.0);
        assert!(snapshot.estimated_liquidity_usd.unwrap() > 2_900_000.0);
    }

    #[test]
    fn low_snapshot_can_use_token_or_usd_threshold() {
        let snapshot = sample_snapshot("sig", 1, 0.04);
        let options = sample_options();

        assert!(snapshot_is_low(&snapshot, &options));
    }

    #[test]
    fn buy_pressure_summarizes_repeated_watched_token_buys() {
        let market = sample_market();
        let mut previous = sample_snapshot("previous", 10, 10.0);
        previous.base_reserve = Some(10.0);
        previous.quote_reserve = Some(10.0);
        previous.token_owner_deltas.clear();

        let mut current = sample_snapshot("current", 11, 8.0);
        current.base_reserve = Some(8.0);
        current.quote_reserve = Some(12.01);
        current.token_owner_deltas = vec![
            TokenOwnerDelta {
                owner: "buyer111".to_owned(),
                mint: LOMBARD_LBTC_MINT.to_owned(),
                amount_delta: 2.0,
            },
            TokenOwnerDelta {
                owner: "buyer111".to_owned(),
                mint: market.quote_token.address.clone(),
                amount_delta: -2.01,
            },
        ];

        let transition = pool_liquidity_transition(
            &previous,
            &current,
            true,
            LOMBARD_LBTC_MINT,
            &market.quote_token.address,
        );
        let pressure = pool_buy_pressure_report(&[transition]).expect("buy pressure");

        assert_eq!(pressure.trade_count, 1);
        assert_eq!(pressure.unique_buyer_count, 1);
        assert!((pressure.watched_token_out - 2.0).abs() < 0.000001);
        assert!((pressure.counter_token_in - 2.01).abs() < 0.000001);
        assert_eq!(pressure.top_buyers[0].owner, "buyer111");
        assert!((pressure.average_counter_per_watched.unwrap() - 1.005).abs() < 0.000001);
    }

    #[test]
    fn low_liquidity_event_uses_threshold_crossing_snapshot() {
        let market = sample_market();
        let previous = sample_snapshot("previous", 10, 0.5);
        let trigger = sample_snapshot("trigger", 11, 0.079);
        let latest = sample_snapshot("latest", 20, 0.042);
        let transition = pool_liquidity_transition(
            &previous,
            &trigger,
            true,
            LOMBARD_LBTC_MINT,
            &market.quote_token.address,
        );
        let options = sample_options();
        let report = TokenLiquidityScanReport {
            token_mint: LOMBARD_LBTC_MINT.to_owned(),
            token_symbol: LOMBARD_LBTC_SYMBOL.to_owned(),
            since_ts: 1_699_999_000,
            pools: Vec::new(),
            wallet_checks: Vec::new(),
            inserted_event_count: 0,
            delivered_event_count: 0,
        };
        let pool = TokenPoolLiquidityReport {
            dex_id: market.dex_id,
            pair_address: market.pair_address,
            pair_url: market.pair_url,
            labels: market.labels,
            base_token: market.base_token,
            quote_token: market.quote_token,
            current_liquidity_usd: latest.estimated_liquidity_usd,
            current_watched_reserve: Some(latest.watched_reserve),
            current_watched_reserve_usd: latest.watched_reserve_usd,
            latest_snapshot: Some(latest.clone()),
            first_low_snapshot: Some(trigger.clone()),
            min_snapshot: Some(latest.clone()),
            low_transition: Some(transition.clone()),
            max_drop_transition: Some(transition),
            buy_pressure: None,
            signature_count: 3,
            successful_signature_count: 3,
            fetched_transaction_count: 3,
            failed_transaction_count: 0,
            snapshot_count: 3,
        };
        let holding = TokenWalletHolding {
            wallet_address: "Wallet1111111111111111111111111111111111111".to_owned(),
            token_amount: 1.25,
        };

        let event =
            build_low_liquidity_event(&options, &report, &pool, &trigger, &latest, &holding);

        assert_eq!(event.signature, "trigger");
        assert_eq!(event.slot, trigger.slot);
        assert!(event.id.starts_with("trigger:token_liquidity_low:"));
        assert_eq!(
            event
                .payload
                .pointer("/latest/signature")
                .and_then(Value::as_str),
            Some("latest")
        );
        assert_eq!(
            event
                .payload
                .pointer("/low_transition/previous/signature")
                .and_then(Value::as_str),
            Some("previous")
        );
        assert!(
            event
                .payload
                .get("watched_token_reserve_delta")
                .and_then(Value::as_f64)
                .is_some_and(|value| value < 0.0)
        );
    }

    #[test]
    fn low_liquidity_event_escalates_pool_imbalance_to_critical() {
        let market = sample_market();
        let previous = sample_snapshot("previous", 10, 2.5);
        let trigger = sample_snapshot("trigger", 11, 0.079);
        let latest = sample_snapshot("latest", 20, 0.042);
        let transition = pool_liquidity_transition(
            &previous,
            &trigger,
            true,
            LOMBARD_LBTC_MINT,
            &market.quote_token.address,
        );
        let options = sample_options();
        let report = TokenLiquidityScanReport {
            token_mint: LOMBARD_LBTC_MINT.to_owned(),
            token_symbol: LOMBARD_LBTC_SYMBOL.to_owned(),
            since_ts: 1_699_999_000,
            pools: Vec::new(),
            wallet_checks: Vec::new(),
            inserted_event_count: 0,
            delivered_event_count: 0,
        };
        let pool = TokenPoolLiquidityReport {
            dex_id: market.dex_id,
            pair_address: market.pair_address,
            pair_url: market.pair_url,
            labels: market.labels,
            base_token: market.base_token,
            quote_token: market.quote_token,
            current_liquidity_usd: latest.estimated_liquidity_usd,
            current_watched_reserve: Some(latest.watched_reserve),
            current_watched_reserve_usd: latest.watched_reserve_usd,
            latest_snapshot: Some(latest.clone()),
            first_low_snapshot: Some(trigger.clone()),
            min_snapshot: Some(latest.clone()),
            low_transition: Some(transition.clone()),
            max_drop_transition: Some(transition),
            buy_pressure: None,
            signature_count: 3,
            successful_signature_count: 3,
            fetched_transaction_count: 3,
            failed_transaction_count: 0,
            snapshot_count: 3,
        };
        let holding = TokenWalletHolding {
            wallet_address: "Wallet1111111111111111111111111111111111111".to_owned(),
            token_amount: 1.25,
        };

        let event =
            build_low_liquidity_event(&options, &report, &pool, &trigger, &latest, &holding);

        assert!(event.tags.iter().any(|tag| tag == "severity:critical"));
        assert!(event.tags.iter().any(|tag| tag == "signal:pool_imbalance"));
        assert_eq!(
            event.payload.get("severity").and_then(Value::as_str),
            Some("critical")
        );
        assert!(
            event
                .payload
                .get("critical_reason")
                .and_then(Value::as_str)
                .is_some_and(|value| value.contains("critical pool imbalance"))
        );
    }
}
