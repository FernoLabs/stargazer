//! Bounded, read-only HTTP queries over the compact archive index.
//!
//! The compact backfill deliberately does not create account-oriented
//! transaction-count indexes while it is writing hundreds of gigabytes. This
//! module therefore serves exact wallet/contract relationships immediately,
//! and only serves transaction totals and leaderboards when a separately
//! finalized `wallet_activity` snapshot is present and valid.

use anyhow::{anyhow, bail, Context, Result};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::Semaphore;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::governance::{load_governance_monitor, GovernanceMonitorResponse};
use crate::incidents::{HistoricalIncidentRules, WalletHistoricalFindings};
use crate::PUBLIC_NETWORK_PASSPHRASE;

const COMPACT_SCHEMA_VERSION: &str = "2";
const SERVING_SCHEMA_VERSION: &str = "1";
const DEFAULT_PAGE_SIZE: usize = 100;
const ABSOLUTE_MAX_PAGE_SIZE: usize = 500;
const MAX_PROTOCOL_ROOTS: usize = 10_000;
const MAX_EXAMPLE_WALLETS: usize = 20;
const MAX_LEADERBOARD_OFFSET: usize = 10_000;
const MAX_PROTOCOL_VERSION_ROWS: usize = 100_000;

#[derive(Debug, Clone)]
pub struct ReadApiConfig {
    pub index_db: PathBuf,
    pub serving_db: Option<PathBuf>,
    pub governance_dir: Option<PathBuf>,
    pub query_timeout: Duration,
    pub max_concurrent_queries: usize,
    pub max_page_size: usize,
}

impl ReadApiConfig {
    pub fn validate(&self) -> Result<()> {
        if self.query_timeout.is_zero() {
            bail!("query timeout must be greater than zero");
        }
        if self.max_concurrent_queries == 0 {
            bail!("max concurrent queries must be greater than zero");
        }
        if self.max_page_size == 0 {
            bail!("max page size must be greater than zero");
        }
        if self.max_page_size > ABSOLUTE_MAX_PAGE_SIZE {
            bail!("max page size must not exceed {ABSOLUTE_MAX_PAGE_SIZE}");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct ReadApiState(Arc<ReadApiInner>);

struct ReadApiInner {
    index_db: PathBuf,
    serving_db: PathBuf,
    governance_dir: Option<PathBuf>,
    historical_incidents: HistoricalIncidentRules,
    source: DatasetSource,
    coverage: LedgerCoverage,
    protocols_by_contract: HashMap<[u8; 32], Vec<ProtocolAttribution>>,
    query_timeout: Duration,
    max_page_size: usize,
    query_slots: Arc<Semaphore>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetSource {
    pub mode: &'static str,
    pub label: &'static str,
    pub indexed_from: String,
    pub as_of: String,
    pub network: &'static str,
    pub indexed_ledger_start: u32,
    pub indexed_ledger_end: u32,
    /// Exact number of transactions retained by the compact index because
    /// they interacted with at least one contract. This is not a count of all
    /// transactions in the covered Stellar ledgers.
    pub indexed_contract_transaction_count: u64,
    pub disclaimer: &'static str,
}

#[derive(Debug, Clone)]
struct LedgerCoverage {
    start: u32,
    end: u32,
    start_close_time: i64,
    end_close_time: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub wallet_contract_relationships: bool,
    pub protocol_attribution: &'static str,
    pub unattributed_contract_placeholders: bool,
    pub wallet_transaction_counts: bool,
    pub wallet_last_activity: bool,
    pub alerts: bool,
    pub historical_findings: bool,
    pub alert_delivery: bool,
    pub leaderboard: bool,
}

impl Capabilities {
    fn new(serving_available: bool, historical_findings: bool) -> Self {
        Self {
            wallet_contract_relationships: true,
            protocol_attribution: "seed-roots-only",
            unattributed_contract_placeholders: true,
            wallet_transaction_counts: serving_available,
            wallet_last_activity: serving_available,
            alerts: historical_findings,
            historical_findings,
            alert_delivery: false,
            leaderboard: serving_available,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub source: DatasetSource,
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletResponse {
    pub source: DatasetSource,
    pub query: WalletQueryIdentity,
    pub summary: WalletSummary,
    pub protocols: Vec<ProtocolSummary>,
    pub contracts: ContractPage,
    pub alerts: WalletHistoricalFindings,
    pub capabilities: Capabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletQueryIdentity {
    pub input_address: String,
    pub account_address: String,
    pub muxed_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletSummary {
    pub account_address: String,
    pub first_interaction_at: Option<String>,
    pub last_interaction_at: Option<String>,
    pub transaction_count: Option<u64>,
    pub successful_transaction_count: Option<u64>,
    /// One-based position under the transaction leaderboard's exact ordering.
    pub transaction_rank: Option<u64>,
    /// One-based position under the protocol leaderboard's exact ordering.
    pub protocol_rank: Option<u64>,
    /// Number of wallets in the finalized activity snapshot used for both ranks.
    pub ranked_wallet_count: Option<u64>,
    pub protocol_count: u64,
    pub contract_count: u64,
    pub unattributed_contract_count: u64,
    pub alert_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolAttribution {
    pub protocol_id: String,
    pub role: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolSummary {
    pub protocol_id: String,
    pub contract_count: u64,
    pub first_interaction_at: Option<String>,
    pub first_interaction_ledger: u32,
    pub transaction_count: Option<u64>,
    pub successful_transaction_count: Option<u64>,
    pub contracts: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractPage {
    pub items: Vec<WalletContract>,
    pub limit: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletContract {
    pub contract_address: String,
    pub first_interaction_ledger: u32,
    pub first_interaction_at: Option<String>,
    pub role_mask: u32,
    pub attribution_status: ContractAttributionStatus,
    pub placeholder_protocol_id: Option<String>,
    pub protocols: Vec<ProtocolAttribution>,
}

/// Whether a wallet-contract edge has a source-backed protocol attribution.
///
/// `Unattributed` is deliberately not an inferred protocol assignment. The
/// API exposes the contract's checksum-valid C-address as a deterministic UI
/// placeholder while keeping the source-backed protocol list empty.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContractAttributionStatus {
    VerifiedRegistry,
    Unattributed,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ContractAttributionFilter {
    #[default]
    All,
    Unattributed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredContractsResponse {
    pub source: DatasetSource,
    pub attribution: ContractAttributionFilter,
    pub items: Vec<DiscoveredContract>,
    pub limit: usize,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredContract {
    pub contract_address: String,
    pub attribution_status: ContractAttributionStatus,
    pub placeholder_protocol_id: Option<String>,
    pub protocols: Vec<ProtocolAttribution>,
    pub first_ledger: u32,
    pub first_seen_at: Option<String>,
    pub last_ledger: u32,
    pub last_seen_at: Option<String>,
    pub transaction_count: u64,
    pub successful_transaction_count: u64,
    pub current_executable: Option<CurrentContractExecutable>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleWalletsResponse {
    pub source: DatasetSource,
    pub selection: &'static str,
    pub wallets: Vec<ExampleWallet>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExampleWallet {
    pub account_address: String,
    pub first_interaction_at: Option<String>,
    pub first_interaction_ledger: u32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LeaderboardMetric {
    #[default]
    Transactions,
    Protocols,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardResponse {
    pub source: DatasetSource,
    pub metric: LeaderboardMetric,
    pub rows: Vec<LeaderboardRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardRow {
    pub rank: usize,
    pub account_address: String,
    pub transaction_count: u64,
    pub successful_transaction_count: u64,
    pub protocol_count: u64,
    pub contract_count: u64,
    pub last_interaction_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCatalogResponse {
    pub source: DatasetSource,
    pub protocols: Vec<ProtocolCatalogEntry>,
    pub alerts: Vec<ContractUpgradeAlert>,
    pub governance: GovernanceMonitorResponse,
    pub capabilities: ProtocolCatalogCapabilities,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCatalogCapabilities {
    pub executable_history: bool,
    pub upgrade_alerts: bool,
    pub multisig_monitoring: bool,
    pub multisig_reason: String,
}

impl ProtocolCatalogCapabilities {
    fn new(governance: &GovernanceMonitorResponse) -> Self {
        Self {
            executable_history: true,
            upgrade_alerts: true,
            multisig_monitoring: governance.available && governance.stale == Some(false),
            multisig_reason: governance.reason.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolCatalogEntry {
    pub protocol_id: String,
    pub contract_count: u64,
    pub version_observation_count: u64,
    pub upgrade_count: u64,
    pub contracts: Vec<ProtocolContractHistory>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolContractHistory {
    pub contract_address: String,
    pub roles: Vec<ProtocolRootRole>,
    pub usage: Option<ProtocolContractUsage>,
    pub current_executable: Option<CurrentContractExecutable>,
    pub versions: Vec<ContractExecutableVersion>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolRootRole {
    pub role: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtocolContractUsage {
    pub first_ledger: u32,
    pub first_seen_at: Option<String>,
    pub last_ledger: u32,
    pub last_seen_at: Option<String>,
    pub transaction_count: u64,
    pub successful_transaction_count: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentContractExecutable {
    pub ledger: u32,
    pub observed_at: Option<String>,
    pub executable_kind: String,
    pub wasm_hash: Option<String>,
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractExecutableVersion {
    pub ledger: u32,
    pub observed_at: Option<String>,
    pub apply_order: u32,
    pub transaction_hash: String,
    pub executable_kind: String,
    pub wasm_hash: Option<String>,
    pub source_kind: String,
    pub active_before_window: bool,
    pub exact_upgrade: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractUpgradeAlert {
    pub alert_id: String,
    pub kind: &'static str,
    pub severity: &'static str,
    pub protocol_id: String,
    pub contract_address: String,
    pub roles: Vec<String>,
    pub ledger: u32,
    pub observed_at: Option<String>,
    pub transaction_hash: String,
    pub previous_wasm_hash: Option<String>,
    pub new_wasm_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WalletParams {
    limit: Option<usize>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ContractsParams {
    limit: Option<usize>,
    cursor: Option<String>,
    #[serde(default)]
    attribution: ContractAttributionFilter,
}

#[derive(Debug, Deserialize)]
struct ExamplesParams {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LeaderboardParams {
    #[serde(default)]
    metric: LeaderboardMetric,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug)]
struct ParsedWallet {
    input_address: String,
    account_address: String,
    account_id: [u8; 32],
    muxed_id: Option<String>,
}

#[derive(Debug)]
struct ServingMeta {
    _generated_at: String,
}

enum ServingProbe {
    Available(Connection, ServingMeta),
    Unavailable(&'static str),
}

#[derive(Debug)]
enum WorkError {
    Internal(anyhow::Error),
    LeaderboardUnavailable {
        reason: &'static str,
        capabilities: Capabilities,
    },
}

impl From<anyhow::Error> for WorkError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}

impl From<rusqlite::Error> for WorkError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Internal(value.into())
    }
}

impl From<std::num::TryFromIntError> for WorkError {
    fn from(value: std::num::TryFromIntError) -> Self {
        Self::Internal(value.into())
    }
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest {
        code: &'static str,
        message: String,
    },
    QueryTimeout,
    Overloaded,
    LeaderboardUnavailable {
        reason: &'static str,
        capabilities: Capabilities,
    },
    Internal,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    error: ErrorBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    capabilities: Option<Capabilities>,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message, capabilities) = match self {
            Self::BadRequest { code, message } => (StatusCode::BAD_REQUEST, code, message, None),
            Self::QueryTimeout => (
                StatusCode::SERVICE_UNAVAILABLE,
                "query_timeout",
                "The read query exceeded its configured deadline.".to_owned(),
                None,
            ),
            Self::Overloaded => (
                StatusCode::SERVICE_UNAVAILABLE,
                "query_capacity_exhausted",
                "All read-query slots are busy; retry shortly.".to_owned(),
                None,
            ),
            Self::LeaderboardUnavailable {
                reason,
                capabilities,
            } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "leaderboard_unavailable",
                reason.to_owned(),
                Some(capabilities),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The read API could not complete this request.".to_owned(),
                None,
            ),
        };
        (
            status,
            Json(ErrorEnvelope {
                error: ErrorBody { code, message },
                capabilities,
            }),
        )
            .into_response()
    }
}

impl ReadApiState {
    pub fn open(config: ReadApiConfig) -> Result<Self> {
        config.validate()?;
        let deadline = Instant::now() + config.query_timeout;
        let connection = open_read_only(&config.index_db, deadline)
            .with_context(|| format!("opening compact index {}", config.index_db.display()))?;
        validate_compact_schema(&connection)?;
        let coverage = load_coverage(&connection)?;
        let indexed_contract_transaction_count =
            load_indexed_contract_transaction_count(&connection)?;
        let protocols_by_contract = load_protocol_attributions(&connection)?;
        let historical_incidents =
            HistoricalIncidentRules::load(&connection, coverage.start, coverage.end)?;
        let source = DatasetSource {
            mode: "live",
            label: "Stargazer bounded archive index",
            indexed_from: format_unix_time(coverage.start_close_time)?,
            as_of: format_unix_time(coverage.end_close_time)?,
            network: "Stellar Mainnet",
            indexed_ledger_start: coverage.start,
            indexed_ledger_end: coverage.end,
            indexed_contract_transaction_count,
            disclaimer: "Exact wallet-contract relationships and contract executable histories. Protocol attribution is limited to source-backed seed roots. Historical incident replay is separated from notification delivery, which is not configured.",
        };
        let serving_db = config.serving_db.unwrap_or_else(|| config.index_db.clone());
        Ok(Self(Arc::new(ReadApiInner {
            index_db: config.index_db,
            serving_db,
            governance_dir: config.governance_dir,
            historical_incidents,
            source,
            coverage,
            protocols_by_contract,
            query_timeout: config.query_timeout,
            max_page_size: config.max_page_size,
            query_slots: Arc::new(Semaphore::new(config.max_concurrent_queries)),
        })))
    }

    pub fn source(&self) -> &DatasetSource {
        &self.0.source
    }

    async fn run_query<T, F>(&self, operation: F) -> std::result::Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&ReadApiInner, Instant) -> std::result::Result<T, WorkError> + Send + 'static,
    {
        let permit = tokio::time::timeout(
            Duration::from_millis(250),
            self.0.query_slots.clone().acquire_owned(),
        )
        .await
        .map_err(|_| ApiError::Overloaded)?
        .map_err(|_| ApiError::Internal)?;
        let inner = self.0.clone();
        let timeout = inner.query_timeout;
        let deadline = Instant::now() + timeout;
        let task = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            operation(&inner, deadline)
        });
        match tokio::time::timeout(timeout + Duration::from_millis(250), task).await {
            Err(_) => Err(ApiError::QueryTimeout),
            Ok(Err(error)) => {
                tracing::error!(?error, "read-query worker failed");
                Err(ApiError::Internal)
            }
            Ok(Ok(Err(WorkError::LeaderboardUnavailable {
                reason,
                capabilities,
            }))) => Err(ApiError::LeaderboardUnavailable {
                reason,
                capabilities,
            }),
            Ok(Ok(Err(WorkError::Internal(error)))) => {
                if Instant::now() >= deadline || is_sqlite_interrupt(&error) {
                    tracing::warn!(?error, "read query timed out");
                    Err(ApiError::QueryTimeout)
                } else {
                    tracing::error!(?error, "read query failed");
                    Err(ApiError::Internal)
                }
            }
            Ok(Ok(Ok(value))) => Ok(value),
        }
    }

    async fn health(&self) -> std::result::Result<HealthResponse, ApiError> {
        self.run_query(|inner, deadline| {
            let serving_available =
                matches!(probe_serving(inner, deadline), ServingProbe::Available(..));
            Ok(HealthResponse {
                status: "ok",
                database: "ready",
                source: inner.source.clone(),
                capabilities: Capabilities::new(
                    serving_available,
                    inner.historical_incidents.available(),
                ),
            })
        })
        .await
    }

    async fn wallet(
        &self,
        address: String,
        limit: usize,
        cursor: Option<String>,
    ) -> std::result::Result<WalletResponse, ApiError> {
        let wallet = parse_wallet(&address)?;
        let cursor = cursor
            .as_deref()
            .map(parse_contract)
            .transpose()
            .map_err(|message| ApiError::BadRequest {
                code: "invalid_cursor",
                message,
            })?;
        let limit = limit.min(self.0.max_page_size).max(1);
        self.run_query(move |inner, deadline| {
            load_wallet(inner, deadline, wallet, limit, cursor).map_err(WorkError::from)
        })
        .await
    }

    async fn examples(
        &self,
        limit: usize,
    ) -> std::result::Result<ExampleWalletsResponse, ApiError> {
        let limit = limit.clamp(1, MAX_EXAMPLE_WALLETS);
        self.run_query(move |inner, deadline| {
            load_example_wallets(inner, deadline, limit).map_err(WorkError::from)
        })
        .await
    }

    async fn contracts(
        &self,
        attribution: ContractAttributionFilter,
        limit: usize,
        cursor: Option<String>,
    ) -> std::result::Result<DiscoveredContractsResponse, ApiError> {
        let cursor = cursor
            .as_deref()
            .map(parse_contract)
            .transpose()
            .map_err(|message| ApiError::BadRequest {
                code: "invalid_cursor",
                message,
            })?;
        let limit = limit.min(self.0.max_page_size).max(1);
        self.run_query(move |inner, deadline| {
            load_discovered_contracts(inner, deadline, attribution, limit, cursor)
                .map_err(WorkError::from)
        })
        .await
    }

    async fn leaderboard(
        &self,
        metric: LeaderboardMetric,
        limit: usize,
        offset: usize,
    ) -> std::result::Result<LeaderboardResponse, ApiError> {
        let limit = limit.min(self.0.max_page_size).max(1);
        if offset > MAX_LEADERBOARD_OFFSET {
            return Err(ApiError::BadRequest {
                code: "invalid_offset",
                message: format!("offset must not exceed {MAX_LEADERBOARD_OFFSET}"),
            });
        }
        self.run_query(move |inner, deadline| {
            load_leaderboard(inner, deadline, metric, limit, offset)
        })
        .await
    }

    async fn protocols(&self) -> std::result::Result<ProtocolCatalogResponse, ApiError> {
        self.run_query(|inner, deadline| {
            load_protocol_catalog(inner, deadline).map_err(WorkError::from)
        })
        .await
    }
}

pub fn router(state: ReadApiState, cors_origins: &[String]) -> Result<Router> {
    let cors = cors_layer(cors_origins)?;
    Ok(Router::new()
        .route("/health", get(health_handler))
        .route("/api/wallets/{wallet}", get(wallet_handler))
        .route("/api/contracts", get(contracts_handler))
        .route(
            "/api/contracts/unattributed",
            get(unattributed_contracts_handler),
        )
        .route("/api/examples/wallets", get(examples_handler))
        .route("/api/leaderboard", get(leaderboard_handler))
        .route("/api/protocols", get(protocols_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}

async fn health_handler(
    State(state): State<ReadApiState>,
) -> std::result::Result<Json<HealthResponse>, ApiError> {
    state.health().await.map(Json)
}

async fn wallet_handler(
    State(state): State<ReadApiState>,
    AxumPath(address): AxumPath<String>,
    Query(params): Query<WalletParams>,
) -> std::result::Result<Json<WalletResponse>, ApiError> {
    state
        .wallet(
            address,
            params.limit.unwrap_or(DEFAULT_PAGE_SIZE),
            params.cursor,
        )
        .await
        .map(Json)
}

async fn examples_handler(
    State(state): State<ReadApiState>,
    Query(params): Query<ExamplesParams>,
) -> std::result::Result<Json<ExampleWalletsResponse>, ApiError> {
    state.examples(params.limit.unwrap_or(5)).await.map(Json)
}

async fn contracts_handler(
    State(state): State<ReadApiState>,
    Query(params): Query<ContractsParams>,
) -> std::result::Result<Json<DiscoveredContractsResponse>, ApiError> {
    state
        .contracts(
            params.attribution,
            params.limit.unwrap_or(DEFAULT_PAGE_SIZE),
            params.cursor,
        )
        .await
        .map(Json)
}

async fn unattributed_contracts_handler(
    State(state): State<ReadApiState>,
    Query(params): Query<ContractsParams>,
) -> std::result::Result<Json<DiscoveredContractsResponse>, ApiError> {
    state
        .contracts(
            ContractAttributionFilter::Unattributed,
            params.limit.unwrap_or(DEFAULT_PAGE_SIZE),
            params.cursor,
        )
        .await
        .map(Json)
}

async fn leaderboard_handler(
    State(state): State<ReadApiState>,
    Query(params): Query<LeaderboardParams>,
) -> std::result::Result<Json<LeaderboardResponse>, ApiError> {
    state
        .leaderboard(
            params.metric,
            params.limit.unwrap_or(50),
            params.offset.unwrap_or(0),
        )
        .await
        .map(Json)
}

async fn protocols_handler(
    State(state): State<ReadApiState>,
) -> std::result::Result<Json<ProtocolCatalogResponse>, ApiError> {
    state.protocols().await.map(Json)
}

fn cors_layer(origins: &[String]) -> Result<CorsLayer> {
    let layer = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_headers([header::CONTENT_TYPE]);
    if origins.iter().any(|origin| origin == "*") {
        return Ok(layer.allow_origin(Any));
    }
    let values = origins
        .iter()
        .map(|origin| {
            origin
                .parse::<HeaderValue>()
                .with_context(|| format!("invalid CORS origin {origin}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(layer.allow_origin(AllowOrigin::list(values)))
}

fn load_wallet(
    inner: &ReadApiInner,
    deadline: Instant,
    wallet: ParsedWallet,
    limit: usize,
    cursor: Option<[u8; 32]>,
) -> Result<WalletResponse> {
    let core = open_read_only(&inner.index_db, deadline)?;
    let alerts = inner
        .historical_incidents
        .findings_for_wallet(&core, &wallet.account_id)?;
    let (contract_count, first_ledger) = wallet_relationship_summary(&core, &wallet.account_id)?;
    let protocols = wallet_protocols(&core, &wallet.account_id)?;
    let attributed_contract_count = protocols
        .iter()
        .flat_map(|protocol| protocol.contracts.iter())
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let unattributed_contract_count = contract_count
        .checked_sub(attributed_contract_count)
        .context("attributed contract count exceeds exact relationship count")?;
    let mut contracts = wallet_contract_page(
        &core,
        &wallet.account_id,
        cursor.as_ref(),
        limit,
        &inner.protocols_by_contract,
    )?;
    let first_interaction_at = first_ledger
        .map(|sequence| ledger_close_time(&core, sequence))
        .transpose()?
        .flatten()
        .map(format_unix_time)
        .transpose()?;

    let (mut serving_available, serving, activity) = match probe_serving(inner, deadline) {
        ServingProbe::Available(serving, _) => {
            let activity = wallet_activity(&serving, &wallet.account_id)?;
            (true, Some(serving), activity)
        }
        ServingProbe::Unavailable(_) => (false, None, None),
    };
    if serving_available
        && activity.as_ref().is_some_and(|activity| {
            activity.contract_count != contract_count
                || activity.protocol_count != protocols.len() as u64
                || Some(activity.first_ledger) != first_ledger
        })
    {
        tracing::warn!(
            account = %wallet.account_address,
            "wallet_activity row disagrees with compact relationships"
        );
        serving_available = false;
    }
    if serving_available && activity.is_none() && contract_count > 0 {
        tracing::warn!(
            account = %wallet.account_address,
            "wallet_activity snapshot is missing an indexed wallet"
        );
        serving_available = false;
    }
    let activity_rank = if serving_available {
        match (serving.as_ref(), activity.as_ref()) {
            (Some(serving), Some(activity)) => {
                Some(wallet_activity_rank(serving, &wallet.account_id, activity)?)
            }
            _ => None,
        }
    } else {
        None
    };
    let (transaction_count, successful_transaction_count, last_interaction_at) =
        if serving_available {
            match activity.as_ref() {
                Some(activity) => (
                    Some(activity.transaction_count),
                    Some(activity.successful_transaction_count),
                    ledger_close_time(&core, activity.last_ledger)?
                        .map(format_unix_time)
                        .transpose()?,
                ),
                None => (Some(0), Some(0), None),
            }
        } else {
            (None, None, None)
        };

    let has_more = contracts.len() > limit;
    if has_more {
        contracts.truncate(limit);
    }
    let next_cursor = has_more
        .then(|| contracts.last().map(|item| item.contract_address.clone()))
        .flatten();
    Ok(WalletResponse {
        source: inner.source.clone(),
        query: WalletQueryIdentity {
            input_address: wallet.input_address,
            account_address: wallet.account_address.clone(),
            muxed_id: wallet.muxed_id,
        },
        summary: WalletSummary {
            account_address: wallet.account_address,
            first_interaction_at,
            last_interaction_at,
            transaction_count,
            successful_transaction_count,
            transaction_rank: activity_rank.as_ref().map(|rank| rank.transaction_rank),
            protocol_rank: activity_rank.as_ref().map(|rank| rank.protocol_rank),
            ranked_wallet_count: activity_rank.map(|rank| rank.ranked_wallet_count),
            protocol_count: protocols.len() as u64,
            contract_count,
            unattributed_contract_count,
            alert_count: alerts.available.then_some(alerts.items.len() as u64),
        },
        protocols,
        contracts: ContractPage {
            items: contracts,
            limit,
            next_cursor,
        },
        alerts,
        capabilities: Capabilities::new(serving_available, inner.historical_incidents.available()),
    })
}

fn wallet_relationship_summary(
    connection: &Connection,
    account_id: &[u8; 32],
) -> Result<(u64, Option<u32>)> {
    let (count, first): (i64, Option<i64>) = connection.query_row(
        "SELECT COUNT(*),MIN(first_ledger)
         FROM wallet_contracts WHERE account_id=?1",
        [&account_id[..]],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((
        nonnegative_u64(count, "contract count")?,
        optional_u32(first)?,
    ))
}

fn wallet_contract_page(
    connection: &Connection,
    account_id: &[u8; 32],
    cursor: Option<&[u8; 32]>,
    limit: usize,
    protocols_by_contract: &HashMap<[u8; 32], Vec<ProtocolAttribution>>,
) -> Result<Vec<WalletContract>> {
    let sql_without_cursor = "SELECT wc.contract_id,wc.first_ledger,wc.role_mask,l.close_time
         FROM wallet_contracts wc
         LEFT JOIN compact_ledgers l ON l.sequence=wc.first_ledger
         WHERE wc.account_id=?1
         ORDER BY wc.contract_id LIMIT ?2";
    let sql_with_cursor = "SELECT wc.contract_id,wc.first_ledger,wc.role_mask,l.close_time
         FROM wallet_contracts wc
         LEFT JOIN compact_ledgers l ON l.sequence=wc.first_ledger
         WHERE wc.account_id=?1 AND wc.contract_id>?2
         ORDER BY wc.contract_id LIMIT ?3";
    let row_limit = i64::try_from(limit.saturating_add(1))?;
    let mut statement = connection.prepare(if cursor.is_some() {
        sql_with_cursor
    } else {
        sql_without_cursor
    })?;
    let mut rows = match cursor {
        Some(cursor) => statement.query(params![&account_id[..], &cursor[..], row_limit])?,
        None => statement.query(params![&account_id[..], row_limit])?,
    };
    let mut result = Vec::with_capacity(limit.saturating_add(1));
    while let Some(row) = rows.next()? {
        let contract_id = blob32(row.get(0)?, "wallet contract id")?;
        let contract_address = contract_strkey(contract_id);
        let first_ledger = checked_u32(row.get::<_, i64>(1)?, "first ledger")?;
        let role_mask = checked_u32(row.get::<_, i64>(2)?, "wallet role mask")?;
        let close_time: Option<i64> = row.get(3)?;
        let protocols = protocols_by_contract
            .get(&contract_id)
            .cloned()
            .unwrap_or_default();
        let (attribution_status, placeholder_protocol_id) = if protocols.is_empty() {
            (
                ContractAttributionStatus::Unattributed,
                Some(contract_address.clone()),
            )
        } else {
            (ContractAttributionStatus::VerifiedRegistry, None)
        };
        result.push(WalletContract {
            contract_address,
            first_interaction_ledger: first_ledger,
            first_interaction_at: close_time.map(format_unix_time).transpose()?,
            role_mask,
            attribution_status,
            placeholder_protocol_id,
            protocols,
        });
    }
    Ok(result)
}

fn load_discovered_contracts(
    inner: &ReadApiInner,
    deadline: Instant,
    attribution: ContractAttributionFilter,
    limit: usize,
    cursor: Option<[u8; 32]>,
) -> Result<DiscoveredContractsResponse> {
    let connection = open_read_only(&inner.index_db, deadline)?;
    let sql_without_cursor = "SELECT usage.contract_id,
                usage.first_ledger,first.close_time,
                usage.last_ledger,last.close_time,
                usage.transaction_count,usage.successful_transaction_count,
                current.ledger_sequence,current_ledger.close_time,
                current.executable_kind,current.wasm_hash,current.exact
         FROM contract_usage usage
         LEFT JOIN compact_ledgers first ON first.sequence=usage.first_ledger
         LEFT JOIN compact_ledgers last ON last.sequence=usage.last_ledger
         LEFT JOIN contract_current_executable current
              ON current.contract_id=usage.contract_id
         LEFT JOIN compact_ledgers current_ledger
              ON current_ledger.sequence=current.ledger_sequence
         ORDER BY usage.contract_id
         LIMIT ?1";
    let sql_with_cursor = "SELECT usage.contract_id,
                usage.first_ledger,first.close_time,
                usage.last_ledger,last.close_time,
                usage.transaction_count,usage.successful_transaction_count,
                current.ledger_sequence,current_ledger.close_time,
                current.executable_kind,current.wasm_hash,current.exact
         FROM contract_usage usage
         LEFT JOIN compact_ledgers first ON first.sequence=usage.first_ledger
         LEFT JOIN compact_ledgers last ON last.sequence=usage.last_ledger
         LEFT JOIN contract_current_executable current
              ON current.contract_id=usage.contract_id
         LEFT JOIN compact_ledgers current_ledger
              ON current_ledger.sequence=current.ledger_sequence
         WHERE usage.contract_id>?1
         ORDER BY usage.contract_id
         LIMIT ?2";

    // Source-backed roots are held in a separately bounded in-memory registry.
    // When filtering them out, reading at most one extra candidate per known
    // contract guarantees either `limit + 1` unattributed rows or end of data.
    let attribution_skip_budget = match attribution {
        ContractAttributionFilter::All => 0,
        ContractAttributionFilter::Unattributed => inner.protocols_by_contract.len(),
    };
    let candidate_limit = limit
        .checked_add(1)
        .and_then(|value| value.checked_add(attribution_skip_budget))
        .context("discovered-contract page limit overflow")?;
    let candidate_limit = i64::try_from(candidate_limit)?;
    let mut statement = connection.prepare(if cursor.is_some() {
        sql_with_cursor
    } else {
        sql_without_cursor
    })?;
    let mut rows = match cursor {
        Some(cursor) => statement.query(params![&cursor[..], candidate_limit])?,
        None => statement.query([candidate_limit])?,
    };
    let mut items = Vec::with_capacity(limit.saturating_add(1));
    while let Some(row) = rows.next()? {
        let contract_id = blob32(row.get(0)?, "discovered contract id")?;
        let protocols = inner
            .protocols_by_contract
            .get(&contract_id)
            .cloned()
            .unwrap_or_default();
        if attribution == ContractAttributionFilter::Unattributed && !protocols.is_empty() {
            continue;
        }
        let contract_address = contract_strkey(contract_id);
        let (attribution_status, placeholder_protocol_id) = if protocols.is_empty() {
            (
                ContractAttributionStatus::Unattributed,
                Some(contract_address.clone()),
            )
        } else {
            (ContractAttributionStatus::VerifiedRegistry, None)
        };
        let current_executable = match row.get::<_, Option<i64>>(7)? {
            Some(ledger) => {
                let executable_kind = row
                    .get::<_, Option<String>>(9)?
                    .context("current executable is missing its kind")?;
                let wasm_hash = row
                    .get::<_, Option<Vec<u8>>>(10)?
                    .context("current executable is missing its hash")?;
                let exact = row
                    .get::<_, Option<i64>>(11)?
                    .context("current executable is missing its exact flag")?;
                Some(CurrentContractExecutable {
                    ledger: checked_u32(ledger, "current executable ledger")?,
                    observed_at: row
                        .get::<_, Option<i64>>(8)?
                        .map(format_unix_time)
                        .transpose()?,
                    executable_kind,
                    wasm_hash: executable_hash_hex(wasm_hash, "current executable Wasm hash")?,
                    exact: sqlite_bool(exact, "current executable exact flag")?,
                })
            }
            None => None,
        };
        items.push(DiscoveredContract {
            contract_address,
            attribution_status,
            placeholder_protocol_id,
            protocols,
            first_ledger: checked_u32(row.get(1)?, "contract usage first ledger")?,
            first_seen_at: row
                .get::<_, Option<i64>>(2)?
                .map(format_unix_time)
                .transpose()?,
            last_ledger: checked_u32(row.get(3)?, "contract usage last ledger")?,
            last_seen_at: row
                .get::<_, Option<i64>>(4)?
                .map(format_unix_time)
                .transpose()?,
            transaction_count: nonnegative_u64(row.get(5)?, "contract usage transaction count")?,
            successful_transaction_count: nonnegative_u64(
                row.get(6)?,
                "contract usage successful transaction count",
            )?,
            current_executable,
        });
        if items.len() > limit {
            break;
        }
    }
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    let next_cursor = has_more
        .then(|| items.last().map(|item| item.contract_address.clone()))
        .flatten();
    Ok(DiscoveredContractsResponse {
        source: inner.source.clone(),
        attribution,
        items,
        limit,
        next_cursor,
    })
}

#[derive(Default)]
struct ProtocolRollup {
    first_ledger: Option<u32>,
    contracts: BTreeSet<[u8; 32]>,
}

fn wallet_protocols(
    connection: &Connection,
    account_id: &[u8; 32],
) -> Result<Vec<ProtocolSummary>> {
    // CROSS JOIN fixes protocol_contracts as the small outer relation. Each
    // lookup then seeks wallet_contracts by its (account_id, contract_id)
    // primary key instead of scanning the large relationship table by
    // contract_id.
    let mut statement = connection.prepare(
        "SELECT pc.protocol_id,pc.contract_id,wc.first_ledger
         FROM protocol_contracts pc
         CROSS JOIN wallet_contracts wc
         WHERE wc.account_id=?1 AND wc.contract_id=pc.contract_id
         ORDER BY pc.protocol_id,pc.contract_id
         LIMIT ?2",
    )?;
    let max_rows = i64::try_from(MAX_PROTOCOL_ROOTS.saturating_add(1))?;
    let mut rows = statement.query(params![&account_id[..], max_rows])?;
    let mut rollups = BTreeMap::<String, ProtocolRollup>::new();
    let mut row_count = 0_usize;
    while let Some(row) = rows.next()? {
        row_count += 1;
        if row_count > MAX_PROTOCOL_ROOTS {
            bail!("protocol root result exceeds safety limit");
        }
        let protocol_id: String = row.get(0)?;
        let contract_id = blob32(row.get(1)?, "protocol contract id")?;
        let first_ledger = checked_u32(row.get::<_, i64>(2)?, "protocol first ledger")?;
        let rollup = rollups.entry(protocol_id).or_default();
        rollup.first_ledger = Some(
            rollup
                .first_ledger
                .map_or(first_ledger, |current| current.min(first_ledger)),
        );
        rollup.contracts.insert(contract_id);
    }
    let mut result = Vec::with_capacity(rollups.len());
    for (protocol_id, rollup) in rollups {
        let first_ledger = rollup
            .first_ledger
            .context("protocol rollup has no first ledger")?;
        let first_interaction_at = ledger_close_time(connection, first_ledger)?
            .map(format_unix_time)
            .transpose()?;
        result.push(ProtocolSummary {
            protocol_id,
            contract_count: rollup.contracts.len() as u64,
            first_interaction_at,
            first_interaction_ledger: first_ledger,
            transaction_count: None,
            successful_transaction_count: None,
            contracts: rollup.contracts.into_iter().map(contract_strkey).collect(),
        });
    }
    Ok(result)
}

fn load_example_wallets(
    inner: &ReadApiInner,
    deadline: Instant,
    limit: usize,
) -> Result<ExampleWalletsResponse> {
    let connection = open_read_only(&inner.index_db, deadline)?;
    let mut statement = connection.prepare(
        "SELECT account_id,MIN(first_ledger)
         FROM wallet_contracts
         GROUP BY account_id
         ORDER BY account_id
         LIMIT ?1",
    )?;
    let mut rows = statement.query([i64::try_from(limit)?])?;
    let mut wallets = Vec::with_capacity(limit);
    while let Some(row) = rows.next()? {
        let account_id = blob32(row.get(0)?, "example account id")?;
        let first_ledger = checked_u32(row.get::<_, i64>(1)?, "example first ledger")?;
        let first_interaction_at = ledger_close_time(&connection, first_ledger)?
            .map(format_unix_time)
            .transpose()?;
        wallets.push(ExampleWallet {
            account_address: account_strkey(account_id),
            first_interaction_at,
            first_interaction_ledger: first_ledger,
        });
    }
    Ok(ExampleWalletsResponse {
        source: inner.source.clone(),
        selection:
            "Lexicographic sample from the exact wallet-contract relationship index; not a ranking.",
        wallets,
    })
}

#[derive(Debug)]
struct WalletActivity {
    transaction_count: u64,
    successful_transaction_count: u64,
    protocol_count: u64,
    contract_count: u64,
    first_ledger: u32,
    last_ledger: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct WalletActivityRank {
    transaction_rank: u64,
    protocol_rank: u64,
    ranked_wallet_count: u64,
}

fn wallet_activity(
    connection: &Connection,
    account_id: &[u8; 32],
) -> Result<Option<WalletActivity>> {
    connection
        .query_row(
            "SELECT transaction_count,successful_transaction_count,
                    protocol_count,contract_count,first_ledger,last_ledger
             FROM wallet_activity WHERE account_id=?1",
            [&account_id[..]],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .map(
            |(transactions, successful, protocols, contracts, first_ledger, last_ledger)| {
                Ok(WalletActivity {
                    transaction_count: nonnegative_u64(transactions, "wallet transaction count")?,
                    successful_transaction_count: nonnegative_u64(
                        successful,
                        "successful wallet transaction count",
                    )?,
                    protocol_count: nonnegative_u64(protocols, "wallet protocol count")?,
                    contract_count: nonnegative_u64(contracts, "wallet contract count")?,
                    first_ledger: checked_u32(first_ledger, "wallet first ledger")?,
                    last_ledger: checked_u32(last_ledger, "wallet last ledger")?,
                })
            },
        )
        .transpose()
}

/// Computes positions using the same total order as `load_leaderboard`.
///
/// The scalar range counts deliberately mirror the two published composite
/// indexes, avoiding a window-function scan or an unbounded leaderboard page.
fn wallet_activity_rank(
    connection: &Connection,
    account_id: &[u8; 32],
    activity: &WalletActivity,
) -> Result<WalletActivityRank> {
    let (transaction_predecessors, protocol_predecessors, ranked_wallet_count): (i64, i64, i64) =
        connection.query_row(
            "SELECT
             (SELECT COUNT(*) FROM wallet_activity
              WHERE transaction_count > ?1)
             +
             (SELECT COUNT(*) FROM wallet_activity
              WHERE transaction_count = ?1 AND account_id < ?3),
             (SELECT COUNT(*) FROM wallet_activity
              WHERE protocol_count > ?2)
             +
             (SELECT COUNT(*) FROM wallet_activity
              WHERE protocol_count = ?2 AND transaction_count > ?1)
             +
             (SELECT COUNT(*) FROM wallet_activity
              WHERE protocol_count = ?2 AND transaction_count = ?1 AND account_id < ?3),
             (SELECT COUNT(*) FROM wallet_activity)",
            params![
                i64::try_from(activity.transaction_count)?,
                i64::try_from(activity.protocol_count)?,
                &account_id[..]
            ],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

    Ok(WalletActivityRank {
        transaction_rank: nonnegative_u64(transaction_predecessors, "transaction predecessors")?
            .checked_add(1)
            .context("transaction rank overflow")?,
        protocol_rank: nonnegative_u64(protocol_predecessors, "protocol predecessors")?
            .checked_add(1)
            .context("protocol rank overflow")?,
        ranked_wallet_count: nonnegative_u64(ranked_wallet_count, "ranked wallet count")?,
    })
}

fn load_leaderboard(
    inner: &ReadApiInner,
    deadline: Instant,
    metric: LeaderboardMetric,
    limit: usize,
    offset: usize,
) -> std::result::Result<LeaderboardResponse, WorkError> {
    let (serving, _) = match probe_serving(inner, deadline) {
        ServingProbe::Available(connection, meta) => (connection, meta),
        ServingProbe::Unavailable(reason) => {
            return Err(WorkError::LeaderboardUnavailable {
                reason,
                capabilities: Capabilities::new(false, inner.historical_incidents.available()),
            })
        }
    };
    let core = open_read_only(&inner.index_db, deadline)?;
    let order = match metric {
        LeaderboardMetric::Transactions => "transaction_count DESC,account_id ASC",
        LeaderboardMetric::Protocols => "protocol_count DESC,transaction_count DESC,account_id ASC",
    };
    let sql = format!(
        "SELECT account_id,transaction_count,successful_transaction_count,
                protocol_count,contract_count,last_ledger
         FROM wallet_activity
         ORDER BY {order} LIMIT ?1 OFFSET ?2"
    );
    let mut statement = serving.prepare(&sql)?;
    let mut rows = statement.query(params![i64::try_from(limit)?, i64::try_from(offset)?])?;
    let mut result = Vec::with_capacity(limit);
    while let Some(row) = rows.next()? {
        let account_id = blob32(row.get(0)?, "leaderboard account id")?;
        let transaction_count = nonnegative_u64(row.get(1)?, "leaderboard transactions")?;
        let successful_transaction_count =
            nonnegative_u64(row.get(2)?, "leaderboard successful transactions")?;
        let protocol_count = nonnegative_u64(row.get(3)?, "leaderboard protocols")?;
        let contract_count = nonnegative_u64(row.get(4)?, "leaderboard contracts")?;
        let last_ledger = checked_u32(row.get(5)?, "leaderboard last ledger")?;
        let close_time = ledger_close_time(&core, last_ledger)?
            .with_context(|| format!("serving snapshot references missing ledger {last_ledger}"))?;
        result.push(LeaderboardRow {
            rank: offset + result.len() + 1,
            account_address: account_strkey(account_id),
            transaction_count,
            successful_transaction_count,
            protocol_count,
            contract_count,
            last_interaction_at: format_unix_time(close_time)?,
        });
    }
    Ok(LeaderboardResponse {
        source: inner.source.clone(),
        metric,
        rows: result,
    })
}

fn load_protocol_catalog(
    inner: &ReadApiInner,
    deadline: Instant,
) -> Result<ProtocolCatalogResponse> {
    let connection = open_read_only(&inner.index_db, deadline)?;
    let governance = load_governance_monitor(inner.governance_dir.as_deref())?;
    let mut registry = BTreeMap::<String, BTreeMap<[u8; 32], Vec<ProtocolRootRole>>>::new();
    for (contract_id, attributions) in &inner.protocols_by_contract {
        for attribution in attributions {
            registry
                .entry(attribution.protocol_id.clone())
                .or_default()
                .entry(*contract_id)
                .or_default()
                .push(ProtocolRootRole {
                    role: attribution.role.clone(),
                    source_url: attribution.source_url.clone(),
                });
        }
    }

    let mut protocols = Vec::with_capacity(registry.len());
    let mut alerts = Vec::<ContractUpgradeAlert>::new();
    let mut total_version_rows = 0_usize;
    for (protocol_id, roots) in registry {
        let mut contracts = Vec::with_capacity(roots.len());
        let mut version_observation_count = 0_u64;
        let mut upgrade_count = 0_u64;
        for (contract_id, mut roles) in roots {
            roles.sort_by(|left, right| {
                left.role
                    .cmp(&right.role)
                    .then_with(|| left.source_url.cmp(&right.source_url))
            });
            roles.dedup_by(|left, right| {
                left.role == right.role && left.source_url == right.source_url
            });
            let history = load_protocol_contract_history(&connection, contract_id)?;
            total_version_rows = total_version_rows.saturating_add(history.versions.len());
            if total_version_rows > MAX_PROTOCOL_VERSION_ROWS {
                bail!("protocol executable history exceeds {MAX_PROTOCOL_VERSION_ROWS} rows");
            }
            version_observation_count = version_observation_count
                .checked_add(history.versions.len() as u64)
                .context("protocol version count overflow")?;

            let contract_address = contract_strkey(contract_id);
            let role_names = roles
                .iter()
                .map(|role| role.role.clone())
                .collect::<Vec<_>>();
            let mut previous_wasm_hash = None;
            for version in &history.versions {
                if version.exact_upgrade {
                    upgrade_count = upgrade_count
                        .checked_add(1)
                        .context("protocol upgrade count overflow")?;
                    alerts.push(ContractUpgradeAlert {
                        alert_id: format!(
                            "contract-upgrade:{protocol_id}:{contract_address}:{}:{}",
                            version.ledger, version.transaction_hash
                        ),
                        kind: "contract_upgrade",
                        severity: "info",
                        protocol_id: protocol_id.clone(),
                        contract_address: contract_address.clone(),
                        roles: role_names.clone(),
                        ledger: version.ledger,
                        observed_at: version.observed_at.clone(),
                        transaction_hash: version.transaction_hash.clone(),
                        previous_wasm_hash: previous_wasm_hash.clone(),
                        new_wasm_hash: version.wasm_hash.clone(),
                    });
                }
                previous_wasm_hash = version.wasm_hash.clone();
            }
            contracts.push(ProtocolContractHistory {
                contract_address,
                roles,
                usage: history.usage,
                current_executable: history.current_executable,
                versions: history.versions,
            });
        }
        protocols.push(ProtocolCatalogEntry {
            protocol_id,
            contract_count: contracts.len() as u64,
            version_observation_count,
            upgrade_count,
            contracts,
        });
    }
    alerts.sort_by(|left, right| {
        right
            .ledger
            .cmp(&left.ledger)
            .then_with(|| right.transaction_hash.cmp(&left.transaction_hash))
            .then_with(|| left.protocol_id.cmp(&right.protocol_id))
            .then_with(|| left.contract_address.cmp(&right.contract_address))
    });
    Ok(ProtocolCatalogResponse {
        source: inner.source.clone(),
        protocols,
        alerts,
        capabilities: ProtocolCatalogCapabilities::new(&governance),
        governance,
    })
}

struct LoadedProtocolContractHistory {
    usage: Option<ProtocolContractUsage>,
    current_executable: Option<CurrentContractExecutable>,
    versions: Vec<ContractExecutableVersion>,
}

fn load_protocol_contract_history(
    connection: &Connection,
    contract_id: [u8; 32],
) -> Result<LoadedProtocolContractHistory> {
    let usage = connection
        .query_row(
            "SELECT u.first_ledger,first.close_time,u.last_ledger,last.close_time,
                    u.transaction_count,u.successful_transaction_count
             FROM contract_usage u
             LEFT JOIN compact_ledgers first ON first.sequence=u.first_ledger
             LEFT JOIN compact_ledgers last ON last.sequence=u.last_ledger
             WHERE u.contract_id=?1",
            [&contract_id[..]],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            },
        )
        .optional()?
        .map(
            |(first_ledger, first_at, last_ledger, last_at, transactions, successful)| -> Result<ProtocolContractUsage> {
                Ok(ProtocolContractUsage {
                    first_ledger: checked_u32(first_ledger, "contract usage first ledger")?,
                    first_seen_at: first_at.map(format_unix_time).transpose()?,
                    last_ledger: checked_u32(last_ledger, "contract usage last ledger")?,
                    last_seen_at: last_at.map(format_unix_time).transpose()?,
                    transaction_count: nonnegative_u64(
                        transactions,
                        "contract usage transaction count",
                    )?,
                    successful_transaction_count: nonnegative_u64(
                        successful,
                        "contract usage successful transaction count",
                    )?,
                })
            },
        )
        .transpose()?;

    let current_executable = connection
        .query_row(
            "SELECT current.ledger_sequence,ledger.close_time,current.executable_kind,
                    current.wasm_hash,current.exact
             FROM contract_current_executable current
             LEFT JOIN compact_ledgers ledger ON ledger.sequence=current.ledger_sequence
             WHERE current.contract_id=?1",
            [&contract_id[..]],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .map(|(ledger, observed_at, executable_kind, wasm_hash, exact)| -> Result<CurrentContractExecutable> {
            Ok(CurrentContractExecutable {
                ledger: checked_u32(ledger, "current executable ledger")?,
                observed_at: observed_at.map(format_unix_time).transpose()?,
                executable_kind,
                wasm_hash: executable_hash_hex(wasm_hash, "current executable Wasm hash")?,
                exact: sqlite_bool(exact, "current executable exact flag")?,
            })
        })
        .transpose()?;

    let mut statement = connection.prepare(
        "SELECT history.ledger_sequence,ledger.close_time,history.apply_order,
                history.tx_hash,history.executable_kind,history.wasm_hash,
                history.source_kind,history.active_before_window
         FROM contract_executable_history history
         LEFT JOIN compact_ledgers ledger ON ledger.sequence=history.ledger_sequence
         WHERE history.contract_id=?1
         ORDER BY history.ledger_sequence,history.apply_order,history.history_id
         LIMIT ?2",
    )?;
    let mut rows = statement.query(params![
        &contract_id[..],
        i64::try_from(MAX_PROTOCOL_VERSION_ROWS.saturating_add(1))?
    ])?;
    let mut versions = Vec::new();
    while let Some(row) = rows.next()? {
        if versions.len() >= MAX_PROTOCOL_VERSION_ROWS {
            bail!("one contract exceeds {MAX_PROTOCOL_VERSION_ROWS} executable history rows");
        }
        let source_kind: String = row.get(6)?;
        let active_before_window = sqlite_bool(
            row.get::<_, i64>(7)?,
            "executable active-before-window flag",
        )?;
        versions.push(ContractExecutableVersion {
            ledger: checked_u32(row.get(0)?, "executable history ledger")?,
            observed_at: row
                .get::<_, Option<i64>>(1)?
                .map(format_unix_time)
                .transpose()?,
            apply_order: checked_u32(row.get(2)?, "executable history apply order")?,
            transaction_hash: hex::encode(blob32(
                row.get(3)?,
                "executable history transaction hash",
            )?),
            executable_kind: row.get(4)?,
            wasm_hash: executable_hash_hex(row.get(5)?, "executable history Wasm hash")?,
            exact_upgrade: source_kind == "updated" && !active_before_window,
            source_kind,
            active_before_window,
        });
    }
    Ok(LoadedProtocolContractHistory {
        usage,
        current_executable,
        versions,
    })
}

fn probe_serving(inner: &ReadApiInner, deadline: Instant) -> ServingProbe {
    if !inner.serving_db.exists() {
        return ServingProbe::Unavailable(
            "The finalized wallet_activity serving snapshot is not available yet.",
        );
    }
    let connection = match open_read_only(&inner.serving_db, deadline) {
        Ok(connection) => connection,
        Err(error) => {
            tracing::warn!(?error, "serving snapshot could not be opened");
            return ServingProbe::Unavailable(
                "The finalized wallet_activity serving snapshot is not available yet.",
            );
        }
    };
    match validate_serving_snapshot(&connection, &inner.coverage) {
        Ok(meta) => ServingProbe::Available(connection, meta),
        Err(error) => {
            tracing::warn!(?error, "serving snapshot is not usable");
            ServingProbe::Unavailable(
                "The wallet_activity snapshot is missing, stale, or lacks its required ranking indexes.",
            )
        }
    }
}

fn validate_serving_snapshot(
    connection: &Connection,
    coverage: &LedgerCoverage,
) -> Result<ServingMeta> {
    let table_exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='wallet_activity'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !table_exists {
        bail!("wallet_activity table does not exist");
    }
    let schema_version = serving_meta(connection, "schema_version")?;
    if schema_version != SERVING_SCHEMA_VERSION {
        bail!("unsupported serving schema version {schema_version}");
    }
    let start: u32 = serving_meta(connection, "source_indexed_ledger_start")?
        .parse()
        .context("invalid serving start ledger")?;
    let end: u32 = serving_meta(connection, "source_indexed_ledger_end")?
        .parse()
        .context("invalid serving end ledger")?;
    if start != coverage.start || end != coverage.end {
        bail!(
            "serving snapshot covers {start}-{end}, compact index covers {}-{}",
            coverage.start,
            coverage.end
        );
    }
    validate_index(
        connection,
        "wallet_activity_by_transactions",
        &[("transaction_count", true), ("account_id", false)],
    )?;
    validate_index(
        connection,
        "wallet_activity_by_protocols",
        &[
            ("protocol_count", true),
            ("transaction_count", true),
            ("account_id", false),
        ],
    )?;
    Ok(ServingMeta {
        _generated_at: serving_meta(connection, "generated_at")?,
    })
}

fn serving_meta(connection: &Connection, key: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT value FROM serving_meta WHERE key=?1",
            [key],
            |row| row.get(0),
        )
        .optional()?
        .with_context(|| format!("missing serving metadata {key}"))
}

fn validate_index(connection: &Connection, name: &str, expected: &[(&str, bool)]) -> Result<()> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1",
            [name],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !exists {
        bail!("required index {name} does not exist");
    }
    let escaped = name.replace('"', "\"\"");
    let mut statement = connection.prepare(&format!("PRAGMA index_xinfo(\"{escaped}\")"))?;
    let mut rows = statement.query([])?;
    let mut actual = Vec::<(String, bool)>::new();
    while let Some(row) = rows.next()? {
        let key_column: bool = row.get(5)?;
        let column_name: Option<String> = row.get(2)?;
        if key_column {
            if let Some(column_name) = column_name {
                let descending: bool = row.get(3)?;
                actual.push((column_name, descending));
            }
        }
    }
    let expected = expected
        .iter()
        .map(|(name, descending)| ((*name).to_owned(), *descending))
        .collect::<Vec<_>>();
    if actual != expected {
        bail!("index {name} columns do not match serving contract");
    }
    Ok(())
}

fn validate_compact_schema(connection: &Connection) -> Result<()> {
    let version: String = connection
        .query_row(
            "SELECT value FROM compact_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .context("reading compact schema version")?;
    if version != COMPACT_SCHEMA_VERSION {
        bail!("unsupported compact schema version {version}");
    }
    let expected_network = hex::encode(Sha256::digest(PUBLIC_NETWORK_PASSPHRASE.as_bytes()));
    let network_id: String = connection
        .query_row(
            "SELECT value FROM compact_meta WHERE key='network_id'",
            [],
            |row| row.get(0),
        )
        .context("reading compact network id")?;
    if network_id != expected_network {
        bail!("compact index is not bound to Stellar public network");
    }
    for table in [
        "compact_ledgers",
        "contract_transactions",
        "wallet_contracts",
        "protocol_contracts",
        "contract_usage",
        "contract_executable_history",
        "contract_current_executable",
    ] {
        let exists = connection
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            bail!("compact index is missing required table {table}");
        }
    }
    Ok(())
}

fn load_coverage(connection: &Connection) -> Result<LedgerCoverage> {
    let first: Option<(i64, i64)> = connection
        .query_row(
            "SELECT sequence,close_time FROM compact_ledgers ORDER BY sequence ASC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let last: Option<(i64, i64)> = connection
        .query_row(
            "SELECT sequence,close_time FROM compact_ledgers ORDER BY sequence DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (start, start_close_time) = first.context("compact index contains no ledgers")?;
    let (end, end_close_time) = last.context("compact index contains no ledgers")?;
    Ok(LedgerCoverage {
        start: checked_u32(start, "coverage start")?,
        end: checked_u32(end, "coverage end")?,
        start_close_time,
        end_close_time,
    })
}

/// Read the high-water mark of the append-only transaction row IDs. The
/// compact writer never deletes rows and SQLite reuses rolled-back row IDs, so
/// the maximum integer primary key is the exact retained transaction count.
/// This is an indexed right-edge lookup rather than a full-table scan.
fn load_indexed_contract_transaction_count(connection: &Connection) -> Result<u64> {
    let count: i64 = connection
        .query_row(
            "SELECT COALESCE(MAX(tx_id),0) FROM contract_transactions",
            [],
            |row| row.get(0),
        )
        .context("reading indexed contract transaction count")?;
    nonnegative_u64(count, "indexed contract transaction count")
}

fn load_protocol_attributions(
    connection: &Connection,
) -> Result<HashMap<[u8; 32], Vec<ProtocolAttribution>>> {
    let mut statement = connection.prepare(
        "SELECT protocol_id,contract_id,role,source_url
         FROM protocol_contracts
         ORDER BY contract_id,protocol_id,role
         LIMIT ?1",
    )?;
    let max_rows = i64::try_from(MAX_PROTOCOL_ROOTS.saturating_add(1))?;
    let mut rows = statement.query([max_rows])?;
    let mut result = HashMap::<[u8; 32], Vec<ProtocolAttribution>>::new();
    let mut count = 0_usize;
    while let Some(row) = rows.next()? {
        count += 1;
        if count > MAX_PROTOCOL_ROOTS {
            bail!("protocol registry exceeds {MAX_PROTOCOL_ROOTS} rows");
        }
        let contract_id = blob32(row.get(1)?, "protocol contract id")?;
        result
            .entry(contract_id)
            .or_default()
            .push(ProtocolAttribution {
                protocol_id: row.get(0)?,
                role: row.get(2)?,
                source_url: row.get(3)?,
            });
    }
    Ok(result)
}

fn open_read_only(path: &Path, deadline: Instant) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_URI,
    )?;
    connection.busy_timeout(Duration::from_millis(250))?;
    connection.execute_batch(
        "PRAGMA query_only=ON;
         PRAGMA trusted_schema=OFF;
         PRAGMA temp_store=MEMORY;
         PRAGMA cache_size=-32768;
         PRAGMA mmap_size=268435456;",
    )?;
    connection.set_prepared_statement_cache_capacity(24);
    connection.progress_handler(10_000, Some(move || Instant::now() >= deadline));
    Ok(connection)
}

fn parse_wallet(input: &str) -> std::result::Result<ParsedWallet, ApiError> {
    let normalized = input.trim().to_ascii_uppercase();
    let account: stellar_xdr::MuxedAccount =
        normalized.parse().map_err(|_| ApiError::BadRequest {
            code: "invalid_wallet",
            message: "Enter a checksum-valid Stellar G- or M-address.".to_owned(),
        })?;
    let (account_id, account_address, muxed_id) = match account {
        stellar_xdr::MuxedAccount::Ed25519(value) => {
            let account_address = stellar_xdr::MuxedAccount::Ed25519(value.clone()).to_string();
            (value.0, account_address, None)
        }
        stellar_xdr::MuxedAccount::MuxedEd25519(value) => {
            let account_address =
                stellar_xdr::MuxedAccount::Ed25519(value.ed25519.clone()).to_string();
            (value.ed25519.0, account_address, Some(value.id.to_string()))
        }
    };
    Ok(ParsedWallet {
        input_address: normalized,
        account_address,
        account_id,
        muxed_id,
    })
}

fn parse_contract(input: &str) -> std::result::Result<[u8; 32], String> {
    let normalized = input.trim().to_ascii_uppercase();
    let contract: stellar_xdr::ContractId = normalized
        .parse()
        .map_err(|_| "cursor must be a checksum-valid Stellar C-address".to_owned())?;
    Ok(contract.0 .0)
}

fn account_strkey(account_id: [u8; 32]) -> String {
    stellar_xdr::MuxedAccount::Ed25519(stellar_xdr::Uint256(account_id)).to_string()
}

fn contract_strkey(contract_id: [u8; 32]) -> String {
    stellar_xdr::ContractId(stellar_xdr::Hash(contract_id)).to_string()
}

fn ledger_close_time(connection: &Connection, sequence: u32) -> Result<Option<i64>> {
    connection
        .query_row(
            "SELECT close_time FROM compact_ledgers WHERE sequence=?1",
            [sequence],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn format_unix_time(timestamp: i64) -> Result<String> {
    Ok(OffsetDateTime::from_unix_timestamp(timestamp)
        .context("invalid ledger close time")?
        .format(&Rfc3339)?)
}

fn blob32(value: Vec<u8>, label: &str) -> Result<[u8; 32]> {
    value
        .try_into()
        .map_err(|_| anyhow!("{label} is not 32 bytes"))
}

fn executable_hash_hex(value: Vec<u8>, label: &str) -> Result<Option<String>> {
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(hex::encode(blob32(value, label)?)))
}

fn sqlite_bool(value: i64, label: &str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => bail!("{label} must be 0 or 1, found {value}"),
    }
}

fn checked_u32(value: i64, label: &str) -> Result<u32> {
    value
        .try_into()
        .with_context(|| format!("{label} is outside the u32 range"))
}

fn optional_u32(value: Option<i64>) -> Result<Option<u32>> {
    value
        .map(|value| checked_u32(value, "ledger sequence"))
        .transpose()
}

fn nonnegative_u64(value: i64, label: &str) -> Result<u64> {
    value
        .try_into()
        .with_context(|| format!("{label} is negative"))
}

fn is_sqlite_interrupt(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<rusqlite::Error>()
            .is_some_and(|error| {
                matches!(
                    error,
                    rusqlite::Error::SqliteFailure(code, _)
                        if code.code == rusqlite::ErrorCode::OperationInterrupted
                )
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use rusqlite::params;
    use tempfile::TempDir;
    use tower::ServiceExt;

    struct Fixture {
        _directory: TempDir,
        compact_path: PathBuf,
        serving_path: PathBuf,
        account_id: [u8; 32],
        account: String,
        muxed: String,
        contract: String,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let compact_path = directory.path().join("compact.sqlite");
            let serving_path = directory.path().join("serving.sqlite");
            let account_id = [7_u8; 32];
            let account = account_strkey(account_id);
            let muxed =
                stellar_xdr::MuxedAccount::MuxedEd25519(stellar_xdr::MuxedAccountMed25519 {
                    id: 42,
                    ed25519: stellar_xdr::Uint256(account_id),
                })
                .to_string();
            let contract_id = [9_u8; 32];
            let contract = contract_strkey(contract_id);
            let network_id = hex::encode(Sha256::digest(PUBLIC_NETWORK_PASSPHRASE.as_bytes()));
            let connection = Connection::open(&compact_path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE compact_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
                     CREATE TABLE compact_ledgers(
                         sequence INTEGER PRIMARY KEY,
                         close_time INTEGER NOT NULL,
                         contract_transaction_count INTEGER NOT NULL
                     );
                     CREATE TABLE contract_transactions(
                         tx_id INTEGER PRIMARY KEY
                     );
                     CREATE TABLE wallet_contracts(
                         account_id BLOB NOT NULL,
                         contract_id BLOB NOT NULL,
                         first_ledger INTEGER NOT NULL,
                         role_mask INTEGER NOT NULL,
                         PRIMARY KEY(account_id,contract_id)
                     ) WITHOUT ROWID;
                     CREATE TABLE protocol_contracts(
                         protocol_id TEXT NOT NULL,
                         contract_id BLOB NOT NULL,
                         role TEXT NOT NULL,
                         source_url TEXT,
                         PRIMARY KEY(protocol_id,contract_id,role)
                     ) WITHOUT ROWID;
                     CREATE TABLE contract_usage(
                         contract_id BLOB PRIMARY KEY,
                         first_ledger INTEGER NOT NULL,
                         last_ledger INTEGER NOT NULL,
                         transaction_count INTEGER NOT NULL,
                         successful_transaction_count INTEGER NOT NULL
                     ) WITHOUT ROWID;
                     CREATE TABLE contract_executable_history(
                         history_id INTEGER PRIMARY KEY,
                         contract_id BLOB NOT NULL,
                         ledger_sequence INTEGER NOT NULL,
                         apply_order INTEGER NOT NULL,
                         tx_hash BLOB NOT NULL,
                         executable_kind TEXT NOT NULL,
                         wasm_hash BLOB NOT NULL,
                         source_kind TEXT NOT NULL,
                         active_before_window INTEGER NOT NULL
                     );
                     CREATE INDEX contract_executable_history_by_contract
                         ON contract_executable_history(
                             contract_id,ledger_sequence,apply_order,history_id
                         );
                     CREATE TABLE contract_current_executable(
                         contract_id BLOB PRIMARY KEY,
                         ledger_sequence INTEGER NOT NULL,
                         executable_kind TEXT NOT NULL,
                         wasm_hash BLOB NOT NULL,
                         exact INTEGER NOT NULL
                     ) WITHOUT ROWID;",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO compact_meta VALUES('schema_version',?1)",
                    [COMPACT_SCHEMA_VERSION],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO compact_meta VALUES('network_id',?1)",
                    [network_id],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO compact_ledgers VALUES
                         (100,1700000000,5),
                         (200,1700000500,7)",
                    [],
                )
                .unwrap();
            connection
                .execute_batch(
                    "INSERT INTO contract_transactions(tx_id) VALUES
                         (1),(2),(3),(4),(5),(6),(7),(8),(9),(10),(11),(12);",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO wallet_contracts VALUES(?1,?2,100,9)",
                    params![&account_id[..], &contract_id[..]],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO protocol_contracts VALUES('test-protocol',?1,'router','https://example.test/source')",
                    [&contract_id[..]],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO contract_usage VALUES(?1,100,200,12,11)",
                    [&contract_id[..]],
                )
                .unwrap();
            let baseline_wasm = [1_u8; 32];
            let updated_wasm = [2_u8; 32];
            let baseline_tx = [3_u8; 32];
            let update_tx = [4_u8; 32];
            connection
                .execute(
                    "INSERT INTO contract_executable_history VALUES(1,?1,100,1,?2,'wasm',?3,'observed_state',1)",
                    params![&contract_id[..], &baseline_tx[..], &baseline_wasm[..]],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO contract_executable_history VALUES(2,?1,200,2,?2,'wasm',?3,'updated',0)",
                    params![&contract_id[..], &update_tx[..], &updated_wasm[..]],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO contract_current_executable VALUES(?1,200,'wasm',?2,1)",
                    params![&contract_id[..], &updated_wasm[..]],
                )
                .unwrap();
            drop(connection);
            Self {
                _directory: directory,
                compact_path,
                serving_path,
                account_id,
                account,
                muxed,
                contract,
            }
        }

        fn state(&self) -> ReadApiState {
            ReadApiState::open(ReadApiConfig {
                index_db: self.compact_path.clone(),
                serving_db: Some(self.serving_path.clone()),
                governance_dir: None,
                query_timeout: Duration::from_secs(2),
                max_concurrent_queries: 2,
                max_page_size: 100,
            })
            .unwrap()
        }

        fn insert_unknown_contract(&self, contract_id: [u8; 32]) -> String {
            let connection = Connection::open(&self.compact_path).unwrap();
            connection
                .execute(
                    "INSERT INTO wallet_contracts VALUES(?1,?2,200,2)",
                    params![&self.account_id[..], &contract_id[..]],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO contract_usage VALUES(?1,200,200,4,3)",
                    [&contract_id[..]],
                )
                .unwrap();
            contract_strkey(contract_id)
        }

        fn create_serving_snapshot(&self) {
            let connection = Connection::open(&self.serving_path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE serving_meta(key TEXT PRIMARY KEY,value TEXT NOT NULL) WITHOUT ROWID;
                     CREATE TABLE wallet_activity(
                         account_id BLOB PRIMARY KEY,
                         transaction_count INTEGER NOT NULL,
                         successful_transaction_count INTEGER NOT NULL,
                         protocol_count INTEGER NOT NULL,
                         contract_count INTEGER NOT NULL,
                         first_ledger INTEGER NOT NULL,
                         last_ledger INTEGER NOT NULL
                     ) WITHOUT ROWID;
                     CREATE INDEX wallet_activity_by_transactions
                         ON wallet_activity(transaction_count DESC,account_id ASC);
                     CREATE INDEX wallet_activity_by_protocols
                         ON wallet_activity(protocol_count DESC,transaction_count DESC,account_id ASC);
                     INSERT INTO serving_meta VALUES
                         ('schema_version','1'),
                         ('source_indexed_ledger_start','100'),
                         ('source_indexed_ledger_end','200'),
                         ('generated_at','2026-07-27T00:00:00Z');",
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO wallet_activity VALUES(?1,12,11,1,1,100,200)",
                    [&self.account_id[..]],
                )
                .unwrap();
        }
    }

    #[tokio::test]
    async fn health_reports_exact_indexed_contract_transaction_count() {
        let fixture = Fixture::new();
        let state = fixture.state();
        let response = state.health().await.unwrap();

        assert_eq!(response.source.indexed_contract_transaction_count, 12);
        assert_eq!(response.source.indexed_from, "2023-11-14T22:13:20Z");
        assert_eq!(response.source.as_of, "2023-11-14T22:21:40Z");

        let app = router(state, &[]).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            value["source"]["indexedContractTransactionCount"],
            serde_json::json!(12)
        );
        assert_eq!(
            value["source"]["indexedFrom"],
            serde_json::json!("2023-11-14T22:13:20Z")
        );
    }

    #[tokio::test]
    async fn wallet_normalizes_muxed_address_and_returns_exact_relationships() {
        let fixture = Fixture::new();
        let response = fixture
            .state()
            .wallet(fixture.muxed.clone(), 25, None)
            .await
            .unwrap();
        assert_eq!(response.query.account_address, fixture.account);
        assert_eq!(response.query.muxed_id.as_deref(), Some("42"));
        assert_eq!(response.summary.contract_count, 1);
        assert_eq!(response.summary.protocol_count, 1);
        assert_eq!(response.summary.transaction_count, None);
        assert_eq!(response.summary.transaction_rank, None);
        assert_eq!(response.summary.protocol_rank, None);
        assert_eq!(response.summary.ranked_wallet_count, None);
        assert_eq!(
            response.contracts.items[0].contract_address,
            fixture.contract
        );
        assert_eq!(response.protocols[0].protocol_id, "test-protocol");
        assert!(!response.alerts.available);
        assert!(!response.capabilities.leaderboard);
    }

    #[tokio::test]
    async fn wallet_preserves_unattributed_contracts_as_address_placeholders() {
        let fixture = Fixture::new();
        let unknown_contract = fixture.insert_unknown_contract([10_u8; 32]);
        let response = fixture
            .state()
            .wallet(fixture.account.clone(), 25, None)
            .await
            .unwrap();

        assert_eq!(response.summary.protocol_count, 1);
        assert_eq!(response.summary.contract_count, 2);
        assert_eq!(response.summary.unattributed_contract_count, 1);
        assert!(response.capabilities.unattributed_contract_placeholders);

        let known = response
            .contracts
            .items
            .iter()
            .find(|item| item.contract_address == fixture.contract)
            .unwrap();
        assert_eq!(
            known.attribution_status,
            ContractAttributionStatus::VerifiedRegistry
        );
        assert_eq!(known.placeholder_protocol_id, None);
        assert_eq!(known.protocols.len(), 1);

        let unknown = response
            .contracts
            .items
            .iter()
            .find(|item| item.contract_address == unknown_contract)
            .unwrap();
        assert_eq!(
            unknown.attribution_status,
            ContractAttributionStatus::Unattributed
        );
        assert_eq!(
            unknown.placeholder_protocol_id.as_deref(),
            Some(unknown_contract.as_str())
        );
        assert!(unknown.protocols.is_empty());
    }

    #[tokio::test]
    async fn discovered_contracts_are_keyset_paginated_and_filterable() {
        let fixture = Fixture::new();
        let unknown_contract = fixture.insert_unknown_contract([10_u8; 32]);
        let state = fixture.state();

        let first = state
            .contracts(ContractAttributionFilter::All, 1, None)
            .await
            .unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.items[0].contract_address, fixture.contract);
        assert_eq!(
            first.items[0].attribution_status,
            ContractAttributionStatus::VerifiedRegistry
        );
        assert_eq!(
            first.next_cursor.as_deref(),
            Some(fixture.contract.as_str())
        );

        let second = state
            .contracts(ContractAttributionFilter::All, 1, first.next_cursor.clone())
            .await
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0].contract_address, unknown_contract);
        assert_eq!(second.next_cursor, None);

        let unattributed = state
            .contracts(ContractAttributionFilter::Unattributed, 25, None)
            .await
            .unwrap();
        assert_eq!(unattributed.items.len(), 1);
        assert_eq!(unattributed.items[0].contract_address, unknown_contract);
        assert_eq!(
            unattributed.items[0].placeholder_protocol_id.as_deref(),
            Some(unknown_contract.as_str())
        );
        assert_eq!(unattributed.items[0].first_ledger, 200);
        assert_eq!(unattributed.items[0].last_ledger, 200);
        assert_eq!(unattributed.items[0].transaction_count, 4);
        assert_eq!(unattributed.items[0].successful_transaction_count, 3);
        assert!(unattributed.items[0].current_executable.is_none());
    }

    #[tokio::test]
    async fn unattributed_contract_endpoint_serializes_explicit_placeholder_semantics() {
        let fixture = Fixture::new();
        let unknown_contract = fixture.insert_unknown_contract([10_u8; 32]);
        let app = router(fixture.state(), &[]).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/contracts/unattributed?limit=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["attribution"], "unattributed");
        assert_eq!(value["items"][0]["contractAddress"], unknown_contract);
        assert_eq!(value["items"][0]["attributionStatus"], "unattributed");
        assert_eq!(
            value["items"][0]["placeholderProtocolId"],
            value["items"][0]["contractAddress"]
        );
        assert_eq!(value["items"][0]["protocols"], serde_json::json!([]));
    }

    #[test]
    fn discovered_contract_cursor_uses_the_contract_usage_primary_key() {
        let fixture = Fixture::new();
        let connection = Connection::open(&fixture.compact_path).unwrap();
        let mut statement = connection
            .prepare(
                "EXPLAIN QUERY PLAN
                 SELECT contract_id FROM contract_usage
                 WHERE contract_id>?1
                 ORDER BY contract_id LIMIT ?2",
            )
            .unwrap();
        let details = statement
            .query_map(params![&[0_u8; 32][..], 101_i64], |row| {
                row.get::<_, String>(3)
            })
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            details
                .iter()
                .any(|detail| detail.contains("PRIMARY KEY (contract_id>?)")),
            "unexpected query plan: {details:?}"
        );
        assert!(
            details.iter().all(|detail| !detail.contains("TEMP B-TREE")),
            "keyset page must not sort into a temporary table: {details:?}"
        );
    }

    #[tokio::test]
    async fn serving_snapshot_is_discovered_without_api_restart() {
        let fixture = Fixture::new();
        let state = fixture.state();
        let unavailable = state
            .leaderboard(LeaderboardMetric::Transactions, 10, 0)
            .await
            .unwrap_err();
        assert!(matches!(
            unavailable,
            ApiError::LeaderboardUnavailable { .. }
        ));

        fixture.create_serving_snapshot();
        let response = state
            .leaderboard(LeaderboardMetric::Transactions, 10, 0)
            .await
            .unwrap();
        assert_eq!(response.rows.len(), 1);
        assert_eq!(response.rows[0].account_address, fixture.account);
        assert_eq!(response.rows[0].transaction_count, 12);
    }

    #[tokio::test]
    async fn wallet_summary_positions_match_both_leaderboard_orders() {
        let fixture = Fixture::new();
        fixture.create_serving_snapshot();
        let connection = Connection::open(&fixture.serving_path).unwrap();
        for (account_id, transactions, protocols) in [
            ([1_u8; 32], 20, 0),
            ([2_u8; 32], 12, 1),
            ([8_u8; 32], 12, 1),
            ([3_u8; 32], 5, 2),
            ([4_u8; 32], 30, 1),
            ([0_u8; 32], 12, 0),
        ] {
            connection
                .execute(
                    "INSERT INTO wallet_activity VALUES(?1,?2,?2,?3,0,100,200)",
                    params![&account_id[..], transactions, protocols],
                )
                .unwrap();
        }
        drop(connection);

        let state = fixture.state();
        let wallet = state
            .wallet(fixture.account.clone(), 25, None)
            .await
            .unwrap();
        assert_eq!(wallet.summary.transaction_rank, Some(5));
        assert_eq!(wallet.summary.protocol_rank, Some(4));
        assert_eq!(wallet.summary.ranked_wallet_count, Some(7));

        let transactions = state
            .leaderboard(LeaderboardMetric::Transactions, 10, 0)
            .await
            .unwrap();
        assert_eq!(transactions.rows[4].account_address, fixture.account);
        assert_eq!(transactions.rows[4].rank, 5);
        let protocols = state
            .leaderboard(LeaderboardMetric::Protocols, 10, 0)
            .await
            .unwrap();
        assert_eq!(protocols.rows[3].account_address, fixture.account);
        assert_eq!(protocols.rows[3].rank, 4);

        let app = router(state, &[]).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/wallets/{}", fixture.account))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["summary"]["transactionRank"], 5);
        assert_eq!(value["summary"]["protocolRank"], 4);
        assert_eq!(value["summary"]["rankedWalletCount"], 7);
    }

    #[tokio::test]
    async fn incomplete_serving_snapshot_does_not_invent_a_zero_count() {
        let fixture = Fixture::new();
        fixture.create_serving_snapshot();
        let connection = Connection::open(&fixture.serving_path).unwrap();
        connection
            .execute("DELETE FROM wallet_activity", [])
            .unwrap();
        drop(connection);

        let response = fixture
            .state()
            .wallet(fixture.account.clone(), 25, None)
            .await
            .unwrap();
        assert_eq!(response.summary.contract_count, 1);
        assert_eq!(response.summary.transaction_count, None);
        assert_eq!(response.summary.transaction_rank, None);
        assert_eq!(response.summary.protocol_rank, None);
        assert_eq!(response.summary.ranked_wallet_count, None);
        assert!(!response.capabilities.wallet_transaction_counts);
        assert!(!response.capabilities.leaderboard);
    }

    #[tokio::test]
    async fn unavailable_leaderboard_has_stable_http_error_code() {
        let fixture = Fixture::new();
        let app = router(fixture.state(), &[]).unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/leaderboard?metric=protocols")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["error"]["code"], "leaderboard_unavailable");
        assert_eq!(value["capabilities"]["leaderboard"], false);
    }

    #[tokio::test]
    async fn examples_are_real_rows_and_explicitly_not_a_ranking() {
        let fixture = Fixture::new();
        let response = fixture.state().examples(5).await.unwrap();
        assert_eq!(response.wallets.len(), 1);
        assert_eq!(response.wallets[0].account_address, fixture.account);
        assert!(response.selection.contains("not a ranking"));
    }

    #[tokio::test]
    async fn protocol_catalog_turns_only_exact_updates_into_alerts() {
        let fixture = Fixture::new();
        let response = fixture.state().protocols().await.unwrap();
        assert_eq!(response.protocols.len(), 1);
        let protocol = &response.protocols[0];
        assert_eq!(protocol.protocol_id, "test-protocol");
        assert_eq!(protocol.contract_count, 1);
        assert_eq!(protocol.version_observation_count, 2);
        assert_eq!(protocol.upgrade_count, 1);
        assert_eq!(protocol.contracts[0].contract_address, fixture.contract);
        assert_eq!(protocol.contracts[0].versions.len(), 2);
        assert!(!protocol.contracts[0].versions[0].exact_upgrade);
        assert!(protocol.contracts[0].versions[1].exact_upgrade);
        assert_eq!(response.alerts.len(), 1);
        assert_eq!(response.alerts[0].kind, "contract_upgrade");
        assert_eq!(response.alerts[0].ledger, 200);
        assert_eq!(
            response.alerts[0].previous_wasm_hash.as_deref(),
            Some(hex::encode([1_u8; 32]).as_str())
        );
        assert_eq!(
            response.alerts[0].new_wasm_hash.as_deref(),
            Some(hex::encode([2_u8; 32]).as_str())
        );
        assert!(!response.capabilities.multisig_monitoring);
    }
}
