use crate::blob_store::BlobStore;
use crate::program_bytes::{
    ProgramByteAssembly, ProgramDataChunk, assemble_program_bytes, instruction_order_key,
    sha256_hex,
};
use crate::program_disassembly::{
    DISASSEMBLY_FORMAT, KnownProgramInstruction, disassemble_program_bytes,
};
use crate::protocols;
use crate::{
    anchor_idl,
    liquidity_pools::{
        LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED, LiquidityPoolLatestStateRecord,
        LiquidityPoolRecord, LiquidityPoolSnapshotRecord,
    },
    models::{
        ChainEvent, EventScopeCounts, EventsSummary, FirewatchUser, MetricAlertPreset,
        MetricAlertRule, PaymentLink, PendingTelegramDelivery, ProgramDataWrite,
        ProgramDisassemblyRecord, ProgramIdlDataWrite, ProgramIdlVersionRecord, ProgramRiskSummary,
        ProgramSignerStat, ProgramVersionCatalogSummary, ProgramVersionLatestSummary,
        ProgramVersionRecord, ProgramVersionSummary, PromoCode, ProtocolActivitySummary,
        ProtocolMetricObservation, TelegramConfigSession, TelegramEventScope, TelegramSubscription,
        UserEntitlement, event_kind, now_ts,
    },
};
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use flate2::read::{GzDecoder, ZlibDecoder};
use sqlx::{
    QueryBuilder, Row, Sqlite, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{collections::HashMap, io::Read, str::FromStr, time::Duration};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    blob_store: BlobStore,
}

#[derive(Clone, Debug)]
pub struct ProgramActivityState {
    pub program_id: String,
    pub protocol_slug: Option<String>,
    pub protocol_name: Option<String>,
    pub program_label: Option<String>,
    pub last_signature: String,
    pub last_instruction_path: String,
    pub last_slot: i64,
    pub last_block_time: Option<i64>,
    pub updated_at: i64,
}

#[derive(Clone, Copy)]
pub struct EventListFilters<'a> {
    pub protocol_slug: Option<&'a str>,
    pub related_multisigs: &'a [String],
    pub target_address: Option<&'a str>,
    pub scope: Option<&'a str>,
    pub search: Option<&'a str>,
    pub severity: Option<&'a str>,
    pub known_defi_only: bool,
    pub tag: Option<&'a str>,
    pub from_ts: Option<i64>,
    pub before_ts: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct AddressLabelRecord {
    pub label: String,
    pub kind: String,
    pub reference_address: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AddressProtocolLink {
    pub protocol_slug: String,
    pub evidence: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SnsDomainRecord {
    pub name_account: String,
    pub domain_name: String,
    pub owner_address: Option<String>,
    pub class_address: Option<String>,
    pub parent_name_account: Option<String>,
    pub root_domain: bool,
    pub last_seen_slot: Option<i64>,
    pub last_event_kind: Option<String>,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct SnsPrimaryDomainRecord {
    pub owner_address: String,
    pub name_account: String,
    pub domain_name: String,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramVersionByteSide {
    Before,
    After,
}

#[derive(Clone, Debug)]
struct ProgramVersionOrder {
    slot: i64,
    instruction_order: String,
}

#[derive(Clone, Debug)]
struct CompleteProgramVersionBytes {
    id: String,
    program_sha256: String,
    program_bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct ProgramVersionMetadata {
    id: String,
}

#[derive(Clone, Debug)]
struct IdlVersionMetadata {
    id: String,
    idl_len: Option<i64>,
    idl_sha256: Option<String>,
}

#[derive(Clone, Debug)]
struct ProgramVersionLookup {
    slot: i64,
    instruction_order: String,
}

#[derive(Clone, Debug)]
struct CompleteIdlVersionBytes {
    idl_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct BlobMigrationReport {
    pub program_versions: u64,
    pub program_data_writes: u64,
    pub program_idl_version_bytes: u64,
    pub program_idl_version_texts: u64,
    pub disassemblies: u64,
    pub bytes_moved: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DatabaseStorageReport {
    pub database_path: Option<String>,
    pub page_size: i64,
    pub page_count: i64,
    pub freelist_count: i64,
    pub total_bytes: i64,
    pub free_bytes: i64,
    pub used_bytes: i64,
    pub tables: Vec<DatabaseTableStorageStat>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct DatabaseTableStorageStat {
    pub name: String,
    pub bytes: i64,
    pub pages: i64,
}

#[derive(Clone, Debug, Default)]
pub struct BlockTimeUpdateReport {
    pub events: u64,
    pub program_versions: u64,
    pub program_idl_versions: u64,
    pub program_data_writes: u64,
    pub program_idl_writes: u64,
    pub protocol_metric_observations: u64,
}

impl BlockTimeUpdateReport {
    pub fn total(&self) -> u64 {
        self.events
            + self.program_versions
            + self.program_idl_versions
            + self.program_data_writes
            + self.program_idl_writes
            + self.protocol_metric_observations
    }

    pub fn merge(&mut self, other: &Self) {
        self.events += other.events;
        self.program_versions += other.program_versions;
        self.program_idl_versions += other.program_idl_versions;
        self.program_data_writes += other.program_data_writes;
        self.program_idl_writes += other.program_idl_writes;
        self.protocol_metric_observations += other.protocol_metric_observations;
    }
}

impl Database {
    pub async fn connect(database_url: &str, blob_store: BlobStore) -> Result<Self> {
        let connect_options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(connect_options)
            .await?;

        Ok(Self { pool, blob_store })
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                kind TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_address TEXT NOT NULL,
                actor TEXT,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                indexed_at INTEGER NOT NULL,
                summary TEXT NOT NULL,
                tags_json TEXT NOT NULL DEFAULT '[]',
                payload_json TEXT NOT NULL,
                delivery_attempts INTEGER NOT NULL DEFAULT 0,
                delivered_at INTEGER,
                last_error TEXT,
                next_retry_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        self.ensure_column("events", "tags_json", "TEXT NOT NULL DEFAULT '[]'")
            .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_events_slot
            ON events (slot DESC, indexed_at DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_events_target_slot
            ON events (target_address, slot DESC, indexed_at DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_data_writes (
                id TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                instruction_order TEXT NOT NULL,
                loader TEXT NOT NULL,
                write_account TEXT NOT NULL,
                authority TEXT,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                indexed_at INTEGER NOT NULL,
                offset INTEGER NOT NULL,
                data_len INTEGER NOT NULL,
                bytes_sha256 TEXT NOT NULL,
                bytes BLOB NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_data_writes_account_order
            ON program_data_writes (write_account, slot ASC, instruction_order ASC, id ASC);
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.ensure_column("program_data_writes", "bytes_blob_key", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_versions (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL UNIQUE,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                instruction_order TEXT NOT NULL,
                kind TEXT NOT NULL,
                program_address TEXT NOT NULL,
                loader TEXT NOT NULL,
                source_account TEXT NOT NULL,
                authority TEXT,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                indexed_at INTEGER NOT NULL,
                complete INTEGER NOT NULL DEFAULT 0,
                chunk_count INTEGER NOT NULL DEFAULT 0,
                program_len INTEGER,
                program_sha256 TEXT,
                program_bytes BLOB,
                missing_ranges_json TEXT NOT NULL DEFAULT '[]',
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_versions_program_order
            ON program_versions (program_address, slot DESC, instruction_order DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_versions_source_order
            ON program_versions (source_account, slot DESC, instruction_order DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.ensure_column("program_versions", "program_bytes_blob_key", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_idl_writes (
                id TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                instruction_order TEXT NOT NULL,
                origin TEXT NOT NULL,
                metadata_account TEXT NOT NULL,
                source_account TEXT NOT NULL,
                program_address TEXT NOT NULL,
                authority TEXT,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                indexed_at INTEGER NOT NULL,
                offset INTEGER,
                data_len INTEGER NOT NULL,
                bytes_sha256 TEXT NOT NULL,
                bytes BLOB NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_idl_writes_source_order
            ON program_idl_writes (source_account, slot ASC, instruction_order ASC, id ASC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_idl_writes_program_order
            ON program_idl_writes (program_address, slot DESC, instruction_order DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.ensure_column("program_idl_writes", "bytes_blob_key", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_idl_versions (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL UNIQUE,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                instruction_order TEXT NOT NULL,
                kind TEXT NOT NULL,
                origin TEXT NOT NULL,
                program_address TEXT NOT NULL,
                metadata_account TEXT NOT NULL,
                source_account TEXT NOT NULL,
                authority TEXT,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                indexed_at INTEGER NOT NULL,
                complete INTEGER NOT NULL DEFAULT 0,
                chunk_count INTEGER NOT NULL DEFAULT 0,
                idl_len INTEGER,
                idl_sha256 TEXT,
                idl_bytes BLOB,
                idl_text TEXT,
                encoding TEXT,
                compression TEXT,
                format TEXT,
                data_source TEXT,
                missing_ranges_json TEXT NOT NULL DEFAULT '[]',
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_idl_versions_program_order
            ON program_idl_versions (program_address, slot DESC, instruction_order DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_idl_versions_source_order
            ON program_idl_versions (source_account, slot DESC, instruction_order DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.ensure_column("program_idl_versions", "idl_bytes_blob_key", "TEXT")
            .await?;
        self.ensure_column("program_idl_versions", "idl_text_blob_key", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_version_disassemblies (
                version_id TEXT PRIMARY KEY,
                program_address TEXT NOT NULL,
                program_sha256 TEXT NOT NULL,
                format TEXT NOT NULL,
                instruction_count INTEGER NOT NULL,
                entrypoint_instruction INTEGER,
                asm_sha256 TEXT NOT NULL,
                asm_text TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_version_disassemblies_program
            ON program_version_disassemblies (program_address, version_id);
            "#,
        )
        .execute(&self.pool)
        .await?;
        self.ensure_column("program_version_disassemblies", "asm_text_blob_key", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_version_diffs (
                version_id TEXT PRIMARY KEY,
                program_address TEXT NOT NULL,
                previous_version_id TEXT NOT NULL,
                before_program_sha256 TEXT NOT NULL,
                after_program_sha256 TEXT NOT NULL,
                format TEXT NOT NULL,
                diff_sha256 TEXT NOT NULL,
                diff_text TEXT NOT NULL DEFAULT '',
                diff_text_blob_key TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_version_diffs_program
            ON program_version_diffs (program_address, version_id);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_reviews (
                event_id TEXT PRIMARY KEY,
                reviewed_at INTEGER NOT NULL,
                reviewer TEXT,
                outcome TEXT NOT NULL DEFAULT 'reviewed',
                note TEXT
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_event_reviews_reviewed_at
            ON event_reviews (reviewed_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS telegram_subscriptions (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                user_id TEXT,
                chat_id TEXT NOT NULL,
                thread_id INTEGER,
                scope TEXT NOT NULL,
                target_address TEXT,
                required_tags_json TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'active',
                payment_link_id TEXT,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        self.ensure_column(
            "telegram_subscriptions",
            "required_tags_json",
            "TEXT NOT NULL DEFAULT '[]'",
        )
        .await?;
        self.ensure_column("telegram_subscriptions", "user_id", "TEXT")
            .await?;
        self.ensure_column(
            "telegram_subscriptions",
            "status",
            "TEXT NOT NULL DEFAULT 'active'",
        )
        .await?;
        self.ensure_column("telegram_subscriptions", "payment_link_id", "TEXT")
            .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_telegram_subscriptions_scope_target
            ON telegram_subscriptions (scope, target_address, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_telegram_subscriptions_status_user
            ON telegram_subscriptions (status, user_id, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS telegram_deliveries (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                subscription_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                delivery_attempts INTEGER NOT NULL DEFAULT 0,
                delivered_at INTEGER,
                last_error TEXT,
                next_retry_at INTEGER NOT NULL,
                UNIQUE(event_id, subscription_id)
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_telegram_deliveries_pending
            ON telegram_deliveries (delivered_at, next_retry_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS protocol_metric_observations (
                id TEXT PRIMARY KEY,
                signature TEXT NOT NULL,
                instruction_path TEXT NOT NULL,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                protocol_slug TEXT NOT NULL,
                protocol_name TEXT NOT NULL,
                program_id TEXT NOT NULL,
                program_label TEXT NOT NULL,
                instruction_name TEXT NOT NULL,
                metric_key TEXT NOT NULL,
                metric_label TEXT NOT NULL,
                metric_value REAL NOT NULL,
                subject_address TEXT,
                subject_kind TEXT,
                subject_label TEXT,
                payload_json TEXT NOT NULL DEFAULT '{}',
                observed_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_protocol_metric_observations_lookup
            ON protocol_metric_observations (
                protocol_slug,
                instruction_name,
                metric_key,
                subject_address,
                slot DESC,
                observed_at DESC
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS metric_alert_rules (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                protocol_slug TEXT NOT NULL,
                protocol_name TEXT NOT NULL,
                instruction_name TEXT NOT NULL,
                metric_key TEXT NOT NULL,
                metric_label TEXT NOT NULL,
                subject_address TEXT,
                subject_kind TEXT,
                subject_label TEXT,
                min_value REAL,
                max_value REAL,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_metric_alert_rules_lookup
            ON metric_alert_rules (protocol_slug, instruction_name, metric_key, subject_address, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS program_activity_state (
                program_id TEXT PRIMARY KEY,
                protocol_slug TEXT,
                protocol_name TEXT,
                program_label TEXT,
                last_signature TEXT NOT NULL,
                last_instruction_path TEXT NOT NULL,
                last_slot INTEGER NOT NULL,
                last_block_time INTEGER,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS liquidity_pools (
                pool_address TEXT PRIMARY KEY,
                protocol_slug TEXT NOT NULL,
                protocol_name TEXT NOT NULL,
                dex_id TEXT NOT NULL,
                program_id TEXT NOT NULL,
                program_label TEXT NOT NULL,
                token_x_mint TEXT NOT NULL,
                token_y_mint TEXT NOT NULL,
                reserve_x TEXT,
                reserve_y TEXT,
                source TEXT NOT NULL,
                first_seen_slot INTEGER,
                first_seen_signature TEXT,
                last_seen_slot INTEGER,
                last_seen_signature TEXT,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pools_token_x
            ON liquidity_pools (token_x_mint, protocol_slug, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pools_token_y
            ON liquidity_pools (token_y_mint, protocol_slug, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pools_program
            ON liquidity_pools (program_id, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS liquidity_pool_latest_state (
                pool_address TEXT PRIMARY KEY,
                dex_id TEXT NOT NULL,
                base_token_mint TEXT NOT NULL,
                quote_token_mint TEXT NOT NULL,
                base_reserve REAL,
                quote_reserve REAL,
                base_vault TEXT,
                quote_vault TEXT,
                estimated_liquidity_usd REAL,
                latest_signature TEXT NOT NULL,
                latest_slot INTEGER NOT NULL,
                latest_block_time INTEGER,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_latest_state_pair
            ON liquidity_pool_latest_state (base_token_mint, quote_token_mint, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_latest_state_updated
            ON liquidity_pool_latest_state (updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS liquidity_pool_snapshots (
                id TEXT PRIMARY KEY,
                pool_address TEXT NOT NULL,
                dex_id TEXT NOT NULL,
                base_token_mint TEXT NOT NULL,
                quote_token_mint TEXT NOT NULL,
                signature TEXT NOT NULL,
                slot INTEGER NOT NULL,
                block_time INTEGER,
                base_reserve REAL,
                quote_reserve REAL,
                base_vault TEXT,
                quote_vault TEXT,
                estimated_liquidity_usd REAL,
                retention_class TEXT NOT NULL DEFAULT 'default',
                observed_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_pool_slot
            ON liquidity_pool_snapshots (pool_address, slot DESC, id DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_retention
            ON liquidity_pool_snapshots (retention_class, block_time, observed_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_base_time
            ON liquidity_pool_snapshots (base_token_mint, block_time DESC, slot DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_quote_time
            ON liquidity_pool_snapshots (quote_token_mint, block_time DESC, slot DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_program_activity_state_updated
            ON program_activity_state (updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS promo_codes (
                id TEXT PRIMARY KEY,
                code TEXT NOT NULL UNIQUE,
                label TEXT NOT NULL,
                audience TEXT NOT NULL,
                plan_id TEXT NOT NULL,
                duration_months INTEGER NOT NULL,
                max_redemptions INTEGER,
                redeemed_count INTEGER NOT NULL DEFAULT 0,
                starts_at INTEGER,
                expires_at INTEGER,
                active INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS firewatch_users (
                id TEXT PRIMARY KEY,
                telegram_user_id INTEGER,
                telegram_username TEXT,
                label TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_firewatch_users_telegram_user_id
            ON firewatch_users (telegram_user_id)
            WHERE telegram_user_id IS NOT NULL;
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_firewatch_users_telegram_username
            ON firewatch_users (telegram_username);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_entitlements (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                plan_id TEXT NOT NULL,
                source TEXT NOT NULL,
                status TEXT NOT NULL,
                starts_at INTEGER NOT NULL,
                expires_at INTEGER,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_user_entitlements_user_status
            ON user_entitlements (user_id, status, plan_id, expires_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS payment_links (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                plan_id TEXT NOT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                public_url TEXT NOT NULL,
                solana_pay_url TEXT NOT NULL,
                reference TEXT NOT NULL UNIQUE,
                recipient TEXT NOT NULL,
                amount TEXT NOT NULL,
                spl_token TEXT,
                label TEXT NOT NULL,
                message TEXT NOT NULL,
                memo TEXT NOT NULL,
                status TEXT NOT NULL,
                expires_at INTEGER NOT NULL,
                paid_at INTEGER,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_payment_links_user_created
            ON payment_links (user_id, created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS telegram_config_sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                chat_id TEXT NOT NULL,
                chat_title TEXT,
                chat_type TEXT,
                token_hash TEXT NOT NULL UNIQUE,
                expires_at INTEGER NOT NULL,
                used_at INTEGER,
                created_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_telegram_config_sessions_token
            ON telegram_config_sessions (token_hash, expires_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_promo_codes_created_at
            ON promo_codes (created_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS app_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS address_labels (
                address TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                source TEXT NOT NULL,
                kind TEXT NOT NULL,
                reference_address TEXT,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sns_domains (
                name_account TEXT PRIMARY KEY,
                domain_name TEXT NOT NULL,
                owner_address TEXT,
                class_address TEXT,
                parent_name_account TEXT,
                root_domain INTEGER NOT NULL DEFAULT 0,
                last_seen_slot INTEGER,
                last_event_kind TEXT,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_sns_domains_domain_name
            ON sns_domains (domain_name);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sns_domains_owner_address
            ON sns_domains (owner_address, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sns_wallet_primary_domains (
                owner_address TEXT PRIMARY KEY,
                name_account TEXT NOT NULL,
                domain_name TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_sns_wallet_primary_domains_name_account
            ON sns_wallet_primary_domains (name_account, updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_address_labels_updated_at
            ON address_labels (updated_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_address_label(
        &self,
        address: &str,
        label: &str,
        source: &str,
        kind: &str,
        reference_address: Option<&str>,
        updated_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO address_labels (
                address,
                label,
                source,
                kind,
                reference_address,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(address) DO UPDATE SET
                label = excluded.label,
                source = excluded.source,
                kind = excluded.kind,
                reference_address = excluded.reference_address,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(address)
        .bind(label)
        .bind(source)
        .bind(kind)
        .bind(reference_address)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn address_label(&self, address: &str) -> Result<Option<AddressLabelRecord>> {
        let row = sqlx::query(
            r#"
            SELECT address, label, source, kind, reference_address, updated_at
            FROM address_labels
            WHERE address = ?;
            "#,
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(AddressLabelRecord {
                label: row.try_get("label")?,
                kind: row.try_get("kind")?,
                reference_address: row.try_get("reference_address")?,
            })
        })
        .transpose()
    }

    pub async fn address_labels(
        &self,
        addresses: &[String],
    ) -> Result<HashMap<String, AddressLabelRecord>> {
        if addresses.is_empty() {
            return Ok(HashMap::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT address, label, source, kind, reference_address, updated_at
            FROM address_labels
            WHERE address IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for address in addresses {
            separated.push_bind(address);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut labels = HashMap::with_capacity(rows.len());
        for row in rows {
            labels.insert(
                row.try_get("address")?,
                AddressLabelRecord {
                    label: row.try_get("label")?,
                    kind: row.try_get("kind")?,
                    reference_address: row.try_get("reference_address")?,
                },
            );
        }

        Ok(labels)
    }

    pub async fn delete_address_label_if_kind(&self, address: &str, kind: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM address_labels
            WHERE address = ? AND kind = ?;
            "#,
        )
        .bind(address)
        .bind(kind)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_sns_domain(
        &self,
        name_account: &str,
        domain_name: &str,
        owner_address: Option<&str>,
        class_address: Option<&str>,
        parent_name_account: Option<&str>,
        root_domain: bool,
        last_seen_slot: Option<i64>,
        last_event_kind: Option<&str>,
        updated_at: i64,
        deleted_at: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sns_domains (
                name_account,
                domain_name,
                owner_address,
                class_address,
                parent_name_account,
                root_domain,
                last_seen_slot,
                last_event_kind,
                updated_at,
                deleted_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(name_account) DO UPDATE SET
                domain_name = excluded.domain_name,
                owner_address = excluded.owner_address,
                class_address = excluded.class_address,
                parent_name_account = excluded.parent_name_account,
                root_domain = excluded.root_domain,
                last_seen_slot = COALESCE(excluded.last_seen_slot, sns_domains.last_seen_slot),
                last_event_kind = excluded.last_event_kind,
                updated_at = excluded.updated_at,
                deleted_at = excluded.deleted_at;
            "#,
        )
        .bind(name_account)
        .bind(domain_name)
        .bind(owner_address)
        .bind(class_address)
        .bind(parent_name_account)
        .bind(if root_domain { 1 } else { 0 })
        .bind(last_seen_slot)
        .bind(last_event_kind)
        .bind(updated_at)
        .bind(deleted_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_sns_domain_deleted(
        &self,
        name_account: &str,
        last_seen_slot: Option<i64>,
        updated_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sns_domains
            SET
                last_seen_slot = COALESCE(?, last_seen_slot),
                last_event_kind = 'name_service_domain_deleted',
                updated_at = ?,
                deleted_at = ?
            WHERE name_account = ?;
            "#,
        )
        .bind(last_seen_slot)
        .bind(updated_at)
        .bind(updated_at)
        .bind(name_account)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn sns_domain(&self, name_account: &str) -> Result<Option<SnsDomainRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                name_account,
                domain_name,
                owner_address,
                class_address,
                parent_name_account,
                root_domain,
                last_seen_slot,
                last_event_kind,
                updated_at,
                deleted_at
            FROM sns_domains
            WHERE name_account = ?;
            "#,
        )
        .bind(name_account)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(SnsDomainRecord {
                name_account: row.try_get("name_account")?,
                domain_name: row.try_get("domain_name")?,
                owner_address: row.try_get("owner_address")?,
                class_address: row.try_get("class_address")?,
                parent_name_account: row.try_get("parent_name_account")?,
                root_domain: row.try_get::<i64, _>("root_domain")? != 0,
                last_seen_slot: row.try_get("last_seen_slot")?,
                last_event_kind: row.try_get("last_event_kind")?,
                updated_at: row.try_get("updated_at")?,
                deleted_at: row.try_get("deleted_at")?,
            })
        })
        .transpose()
    }

    pub async fn sns_domains_for_name_accounts(
        &self,
        name_accounts: &[String],
    ) -> Result<HashMap<String, SnsDomainRecord>> {
        if name_accounts.is_empty() {
            return Ok(HashMap::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                name_account,
                domain_name,
                owner_address,
                class_address,
                parent_name_account,
                root_domain,
                last_seen_slot,
                last_event_kind,
                updated_at,
                deleted_at
            FROM sns_domains
            WHERE name_account IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for name_account in name_accounts {
            separated.push_bind(name_account);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut domains = HashMap::with_capacity(rows.len());
        for row in rows {
            let name_account = row.try_get::<String, _>("name_account")?;
            domains.insert(
                name_account.clone(),
                SnsDomainRecord {
                    name_account,
                    domain_name: row.try_get("domain_name")?,
                    owner_address: row.try_get("owner_address")?,
                    class_address: row.try_get("class_address")?,
                    parent_name_account: row.try_get("parent_name_account")?,
                    root_domain: row.try_get::<i64, _>("root_domain")? != 0,
                    last_seen_slot: row.try_get("last_seen_slot")?,
                    last_event_kind: row.try_get("last_event_kind")?,
                    updated_at: row.try_get("updated_at")?,
                    deleted_at: row.try_get("deleted_at")?,
                },
            );
        }

        Ok(domains)
    }

    pub async fn upsert_sns_primary_domain(
        &self,
        owner_address: &str,
        name_account: &str,
        domain_name: &str,
        updated_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sns_wallet_primary_domains (
                owner_address,
                name_account,
                domain_name,
                updated_at
            )
            VALUES (?, ?, ?, ?)
            ON CONFLICT(owner_address) DO UPDATE SET
                name_account = excluded.name_account,
                domain_name = excluded.domain_name,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(owner_address)
        .bind(name_account)
        .bind(domain_name)
        .bind(updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_sns_primary_domain(&self, owner_address: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sns_wallet_primary_domains
            WHERE owner_address = ?;
            "#,
        )
        .bind(owner_address)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn sns_primary_domain(
        &self,
        owner_address: &str,
    ) -> Result<Option<SnsPrimaryDomainRecord>> {
        let row = sqlx::query(
            r#"
            SELECT owner_address, name_account, domain_name, updated_at
            FROM sns_wallet_primary_domains
            WHERE owner_address = ?;
            "#,
        )
        .bind(owner_address)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(SnsPrimaryDomainRecord {
                owner_address: row.try_get("owner_address")?,
                name_account: row.try_get("name_account")?,
                domain_name: row.try_get("domain_name")?,
                updated_at: row.try_get("updated_at")?,
            })
        })
        .transpose()
    }

    pub async fn sns_primary_domains_for_owners(
        &self,
        owner_addresses: &[String],
    ) -> Result<HashMap<String, SnsPrimaryDomainRecord>> {
        if owner_addresses.is_empty() {
            return Ok(HashMap::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT owner_address, name_account, domain_name, updated_at
            FROM sns_wallet_primary_domains
            WHERE owner_address IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for owner_address in owner_addresses {
            separated.push_bind(owner_address);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut domains = HashMap::with_capacity(rows.len());
        for row in rows {
            let owner_address = row.try_get::<String, _>("owner_address")?;
            domains.insert(
                owner_address.clone(),
                SnsPrimaryDomainRecord {
                    owner_address,
                    name_account: row.try_get("name_account")?,
                    domain_name: row.try_get("domain_name")?,
                    updated_at: row.try_get("updated_at")?,
                },
            );
        }

        Ok(domains)
    }

    pub async fn recent_unlabeled_addresses(&self, limit: i64) -> Result<Vec<String>> {
        let scan_limit = (limit.max(1) * 250).clamp(1_000, 25_000);
        let rows = sqlx::query(
            r#"
            WITH recent_events AS (
                SELECT target_address, actor, payload_json, slot
                FROM events
                ORDER BY slot DESC, indexed_at DESC, id DESC
                LIMIT ?
            ),
            raw_candidates(address, slot) AS (
                SELECT target_address, slot FROM recent_events
                UNION ALL
                SELECT actor, slot FROM recent_events WHERE actor IS NOT NULL AND actor <> ''
                UNION ALL
                SELECT json_extract(payload_json, '$.member'), slot FROM recent_events
                  WHERE json_extract(payload_json, '$.member') IS NOT NULL
                    AND json_extract(payload_json, '$.member') <> ''
                UNION ALL
                SELECT json_extract(payload_json, '$.multisig'), slot FROM recent_events
                  WHERE json_extract(payload_json, '$.multisig') IS NOT NULL
                    AND json_extract(payload_json, '$.multisig') <> ''
                UNION ALL
                SELECT json_extract(payload_json, '$.owner'), slot FROM recent_events
                  WHERE json_extract(payload_json, '$.owner') IS NOT NULL
                    AND json_extract(payload_json, '$.owner') <> ''
                UNION ALL
                SELECT json_extract(payload_json, '$.new_owner'), slot FROM recent_events
                  WHERE json_extract(payload_json, '$.new_owner') IS NOT NULL
                    AND json_extract(payload_json, '$.new_owner') <> ''
                UNION ALL
                SELECT json_extract(payload_json, '$.refund_target'), slot FROM recent_events
                  WHERE json_extract(payload_json, '$.refund_target') IS NOT NULL
                    AND json_extract(payload_json, '$.refund_target') <> ''
            ),
            candidates AS (
                SELECT address, MAX(slot) AS latest_slot
                FROM raw_candidates
                WHERE address IS NOT NULL AND address <> ''
                GROUP BY address
            )
            SELECT candidates.address
            FROM candidates
            LEFT JOIN address_labels ON address_labels.address = candidates.address
            WHERE address_labels.address IS NULL
            ORDER BY candidates.latest_slot DESC
            LIMIT ?;
            "#,
        )
        .bind(scan_limit)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<Option<String>, _>("address").ok().flatten())
            .collect())
    }

    pub async fn insert_event(&self, event: &ChainEvent) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO events (
                id,
                signature,
                instruction_path,
                kind,
                target_type,
                target_address,
                actor,
                slot,
                block_time,
                indexed_at,
                summary,
                tags_json,
                payload_json,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO NOTHING;
            "#,
        )
        .bind(&event.id)
        .bind(&event.signature)
        .bind(&event.instruction_path)
        .bind(&event.kind)
        .bind(&event.target_type)
        .bind(&event.target_address)
        .bind(&event.actor)
        .bind(event.slot)
        .bind(event.block_time)
        .bind(event.indexed_at)
        .bind(&event.summary)
        .bind(serde_json::to_string(&event.tags)?)
        .bind(serde_json::to_string(&event.payload)?)
        .bind(event.delivery_attempts)
        .bind(event.delivered_at)
        .bind(&event.last_error)
        .bind(event.next_retry_at)
        .execute(&self.pool)
        .await?;

        let inserted = result.rows_affected() == 1;
        if !inserted && event.block_time.is_some() {
            sqlx::query(
                r#"
                UPDATE events
                SET block_time = ?
                WHERE id = ?
                  AND block_time IS NULL;
                "#,
            )
            .bind(event.block_time)
            .bind(&event.id)
            .execute(&self.pool)
            .await?;
        }

        Ok(inserted)
    }

    pub async fn insert_program_data_write(&self, write: &ProgramDataWrite) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO program_data_writes (
                id,
                signature,
                instruction_path,
                instruction_order,
                loader,
                write_account,
                authority,
                slot,
                block_time,
                indexed_at,
                offset,
                data_len,
                bytes_sha256,
                bytes
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO NOTHING;
            "#,
        )
        .bind(&write.id)
        .bind(&write.signature)
        .bind(&write.instruction_path)
        .bind(&write.instruction_order)
        .bind(&write.loader)
        .bind(&write.write_account)
        .bind(&write.authority)
        .bind(write.slot)
        .bind(write.block_time)
        .bind(write.indexed_at)
        .bind(write.offset)
        .bind(write.bytes.len() as i64)
        .bind(&write.bytes_sha256)
        .bind(&write.bytes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn insert_program_idl_data_write(&self, write: &ProgramIdlDataWrite) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO program_idl_writes (
                id,
                signature,
                instruction_path,
                instruction_order,
                origin,
                metadata_account,
                source_account,
                program_address,
                authority,
                slot,
                block_time,
                indexed_at,
                offset,
                data_len,
                bytes_sha256,
                bytes
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO NOTHING;
            "#,
        )
        .bind(&write.id)
        .bind(&write.signature)
        .bind(&write.instruction_path)
        .bind(&write.instruction_order)
        .bind(&write.origin)
        .bind(&write.metadata_account)
        .bind(&write.source_account)
        .bind(&write.program_address)
        .bind(&write.authority)
        .bind(write.slot)
        .bind(write.block_time)
        .bind(write.indexed_at)
        .bind(write.offset)
        .bind(write.bytes.len() as i64)
        .bind(&write.bytes_sha256)
        .bind(&write.bytes)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn upsert_program_versions_for_write_account(
        &self,
        write_account: &str,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                kind,
                target_type,
                target_address,
                actor,
                slot,
                block_time,
                indexed_at,
                summary,
                tags_json,
                payload_json,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            FROM events
            WHERE target_type = 'program'
              AND kind IN ('program_deployed', 'program_upgraded')
              AND (
                    json_extract(payload_json, '$.buffer_account') = ?
                 OR (
                        target_address = ?
                    AND json_extract(payload_json, '$.loader') IS NOT NULL
                    AND json_extract(payload_json, '$.loader') <> 'upgradeable'
                 )
              )
            ORDER BY slot ASC, instruction_path ASC, id ASC;
            "#,
        )
        .bind(write_account)
        .bind(write_account)
        .fetch_all(&self.pool)
        .await?;

        let mut updated = 0;
        for row in rows {
            let event = row_to_event(row)?;
            if self
                .upsert_program_version_for_event(&event)
                .await?
                .is_some()
            {
                updated += 1;
            }
        }

        Ok(updated)
    }

    pub async fn upsert_program_version_for_event(
        &self,
        event: &ChainEvent,
    ) -> Result<Option<ProgramVersionSummary>> {
        let Some(source) = ProgramVersionEventSource::from_event(event) else {
            return Ok(None);
        };

        let previous_source_order = self
            .previous_program_version_order_for_source(
                &source.source_account,
                source.slot,
                &source.instruction_order,
            )
            .await?;
        let base_bytes = if source.uses_direct_account_writes {
            self.previous_complete_program_bytes(
                &source.program_address,
                source.slot,
                &source.instruction_order,
            )
            .await?
        } else {
            None
        };
        let existing_bytes = self
            .complete_program_version_bytes_by_id(&source.program_address, &source.id)
            .await?;
        let assembly = match existing_bytes {
            Some(existing) => ProgramByteAssembly {
                program_len: Some(existing.program_bytes.len() as i64),
                program_sha256: Some(existing.program_sha256),
                bytes: Some(existing.program_bytes),
                complete: true,
                chunk_count: 0,
                missing_ranges: Vec::new(),
            },
            _ => {
                let chunks = self
                    .program_data_chunks_for_source(
                        &source.source_account,
                        previous_source_order.as_ref(),
                        source.slot,
                        &source.instruction_order,
                    )
                    .await?;
                assemble_program_bytes(base_bytes, chunks)
            }
        };
        let missing_ranges_json = serde_json::to_string(&assembly.missing_ranges)?;
        let program_bytes_blob_key = match (
            assembly.bytes.as_deref(),
            assembly.program_sha256.as_deref(),
        ) {
            (Some(bytes), Some(program_sha256)) => {
                self.put_program_version_blob(
                    &source.program_address,
                    source.slot,
                    &source.id,
                    program_sha256,
                    bytes,
                )
                .await?
            }
            _ => None,
        };
        let program_bytes_inline: Option<&[u8]> = if program_bytes_blob_key.is_some() {
            None
        } else {
            assembly.bytes.as_deref()
        };
        let now = now_ts();

        sqlx::query(
            r#"
            INSERT INTO program_versions (
                id,
                event_id,
                signature,
                instruction_path,
                instruction_order,
                kind,
                program_address,
                loader,
                source_account,
                authority,
                slot,
                block_time,
                indexed_at,
                complete,
                chunk_count,
                program_len,
                program_sha256,
                program_bytes,
                program_bytes_blob_key,
                missing_ranges_json,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                signature = excluded.signature,
                instruction_path = excluded.instruction_path,
                instruction_order = excluded.instruction_order,
                kind = excluded.kind,
                program_address = excluded.program_address,
                loader = excluded.loader,
                source_account = excluded.source_account,
                authority = excluded.authority,
                slot = excluded.slot,
                block_time = COALESCE(excluded.block_time, program_versions.block_time),
                indexed_at = excluded.indexed_at,
                complete = excluded.complete,
                chunk_count = excluded.chunk_count,
                program_len = excluded.program_len,
                program_sha256 = excluded.program_sha256,
                program_bytes = excluded.program_bytes,
                program_bytes_blob_key = excluded.program_bytes_blob_key,
                missing_ranges_json = excluded.missing_ranges_json,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(&source.id)
        .bind(&source.event_id)
        .bind(&source.signature)
        .bind(&source.instruction_path)
        .bind(&source.instruction_order)
        .bind(&source.kind)
        .bind(&source.program_address)
        .bind(&source.loader)
        .bind(&source.source_account)
        .bind(&source.authority)
        .bind(source.slot)
        .bind(source.block_time)
        .bind(source.indexed_at)
        .bind(if assembly.complete { 1_i64 } else { 0_i64 })
        .bind(assembly.chunk_count as i64)
        .bind(assembly.program_len)
        .bind(&assembly.program_sha256)
        .bind(program_bytes_inline)
        .bind(program_bytes_blob_key.as_deref())
        .bind(&missing_ranges_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        if assembly.complete
            && let (Some(bytes), Some(program_sha256)) = (
                assembly.bytes.as_deref(),
                assembly.program_sha256.as_deref(),
            )
            && let Err(error) = self
                .upsert_program_disassembly_for_bytes(
                    &source.id,
                    &source.program_address,
                    program_sha256,
                    bytes,
                )
                .await
        {
            tracing::warn!(
                version_id = %source.id,
                program_address = %source.program_address,
                error = %error,
                "failed to disassemble program version",
            );
        }

        if assembly.complete {
            if let Err(error) = self
                .upsert_program_text_diff_for_version(&source.program_address, &source.id)
                .await
            {
                tracing::warn!(
                    version_id = %source.id,
                    program_address = %source.program_address,
                    error = %error,
                    "failed to store program version text diff",
                );
            }

            if let Err(error) = self
                .prune_program_data_write_bytes_for_source(
                    &source.source_account,
                    previous_source_order.as_ref(),
                    source.slot,
                    &source.instruction_order,
                )
                .await
            {
                tracing::warn!(
                    version_id = %source.id,
                    source_account = %source.source_account,
                    error = %error,
                    "failed to prune raw program data write bytes after version assembly",
                );
            }
        }

        let summary = ProgramVersionSummary {
            id: source.id,
            event_id: source.event_id,
            program_address: source.program_address,
            loader: source.loader,
            source_account: source.source_account,
            slot: source.slot,
            instruction_path: source.instruction_path,
            complete: assembly.complete,
            chunk_count: assembly.chunk_count as i64,
            program_len: assembly.program_len,
            program_sha256: assembly.program_sha256,
            missing_ranges: assembly.missing_ranges,
        };
        self.update_event_program_version_payload(&summary).await?;

        Ok(Some(summary))
    }

    pub async fn upsert_program_idl_versions_for_source_account(
        &self,
        source_account: &str,
    ) -> Result<usize> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                kind,
                target_type,
                target_address,
                actor,
                slot,
                block_time,
                indexed_at,
                summary,
                tags_json,
                payload_json,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            FROM events
            WHERE target_type = 'program'
              AND kind IN (
                    'program_idl_initialized',
                    'program_idl_updated',
                    'program_idl_authority_changed',
                    'program_idl_immutable',
                    'program_idl_resized',
                    'program_idl_closed'
              )
              AND (
                    json_extract(payload_json, '$.idl_source_account') = ?
                 OR json_extract(payload_json, '$.metadata_account') = ?
              )
            ORDER BY slot ASC, instruction_path ASC, id ASC;
            "#,
        )
        .bind(source_account)
        .bind(source_account)
        .fetch_all(&self.pool)
        .await?;

        let mut updated = 0;
        for row in rows {
            let event = row_to_event(row)?;
            if self
                .upsert_program_idl_version_for_event(&event)
                .await?
                .is_some()
            {
                updated += 1;
            }
        }

        Ok(updated)
    }

    pub async fn upsert_anchor_legacy_direct_idl_version_for_write(
        &self,
        write: &ProgramIdlDataWrite,
    ) -> Result<Option<ProgramIdlVersionRecord>> {
        if write.origin != anchor_idl::LEGACY_ANCHOR_IDL_ORIGIN
            || write.source_account != write.metadata_account
        {
            return Ok(None);
        }

        let chunks = self
            .program_idl_chunks_for_source(
                &write.source_account,
                None,
                write.slot,
                &write.instruction_order,
            )
            .await?;
        let assembly = assemble_program_bytes(None, chunks);
        let Some(bytes) = assembly.bytes.as_deref() else {
            return Ok(None);
        };
        let Some(idl_text) = decode_idl_text(bytes, Some("utf8"), Some("zlib"), Some("json"))
            .ok()
            .flatten()
        else {
            return Ok(None);
        };

        if serde_json::from_str::<serde_json::Value>(&idl_text).is_err() {
            return Ok(None);
        }

        let previous = self
            .previous_idl_version_metadata(
                &write.program_address,
                write.slot,
                &write.instruction_order,
            )
            .await?;
        if previous
            .as_ref()
            .and_then(|record| record.idl_sha256.as_deref())
            == assembly.program_sha256.as_deref()
        {
            return Ok(None);
        }

        let kind = if previous.is_some() {
            event_kind::PROGRAM_IDL_UPDATED
        } else {
            event_kind::PROGRAM_IDL_INITIALIZED
        };
        let version_id = format!("{}:idl_version", write.id);
        let source = ProgramIdlVersionEventSource {
            id: version_id.clone(),
            event_id: write.id.clone(),
            signature: write.signature.clone(),
            instruction_path: write.instruction_path.clone(),
            instruction_order: write.instruction_order.clone(),
            kind: kind.to_owned(),
            origin: write.origin.clone(),
            program_address: write.program_address.clone(),
            metadata_account: write.metadata_account.clone(),
            source_account: write.source_account.clone(),
            authority: write.authority.clone(),
            slot: write.slot,
            block_time: write.block_time,
            indexed_at: write.indexed_at,
            encoding: Some("utf8".to_owned()),
            compression: Some("zlib".to_owned()),
            format: Some("json".to_owned()),
            data_source: Some("anchor_idl_account".to_owned()),
            reset_data: true,
            preserves_previous_data: false,
            closed: false,
        };

        self.upsert_program_idl_version_from_source(source, assembly, Some(idl_text))
            .await?;
        self.program_idl_version_by_id(&write.program_address, &version_id)
            .await
    }

    pub async fn upsert_latest_anchor_legacy_direct_idl_version_for_source_account(
        &self,
        source_account: &str,
    ) -> Result<Option<ProgramIdlVersionRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                instruction_order,
                origin,
                metadata_account,
                source_account,
                program_address,
                authority,
                slot,
                block_time,
                indexed_at,
                offset,
                bytes_sha256,
                bytes
            FROM program_idl_writes
            WHERE source_account = ?
              AND metadata_account = source_account
              AND origin = ?
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(source_account)
        .bind(anchor_idl::LEGACY_ANCHOR_IDL_ORIGIN)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let write = ProgramIdlDataWrite {
            id: row.try_get("id")?,
            signature: row.try_get("signature")?,
            instruction_path: row.try_get("instruction_path")?,
            instruction_order: row.try_get("instruction_order")?,
            origin: row.try_get("origin")?,
            metadata_account: row.try_get("metadata_account")?,
            source_account: row.try_get("source_account")?,
            program_address: row.try_get("program_address")?,
            authority: row.try_get("authority")?,
            slot: row.try_get("slot")?,
            block_time: row.try_get("block_time")?,
            indexed_at: row.try_get("indexed_at")?,
            offset: row.try_get("offset")?,
            bytes: row.try_get("bytes")?,
            bytes_sha256: row.try_get("bytes_sha256")?,
        };

        self.upsert_anchor_legacy_direct_idl_version_for_write(&write)
            .await
    }

    pub async fn upsert_program_idl_version_for_event(
        &self,
        event: &ChainEvent,
    ) -> Result<Option<ProgramIdlVersionRecord>> {
        let Some(source) = ProgramIdlVersionEventSource::from_event(event) else {
            return Ok(None);
        };

        let assembly = if source.closed {
            assemble_program_bytes(None, Vec::new())
        } else {
            let previous_source_order = self
                .previous_idl_version_order_for_source(
                    &source.source_account,
                    source.slot,
                    &source.instruction_order,
                )
                .await?;
            let base_bytes = if source.reset_data {
                None
            } else {
                self.previous_complete_idl_bytes(
                    &source.program_address,
                    source.slot,
                    &source.instruction_order,
                )
                .await?
                .map(|record| record.idl_bytes)
            };
            let chunks = self
                .program_idl_chunks_for_source(
                    &source.source_account,
                    previous_source_order.as_ref(),
                    source.slot,
                    &source.instruction_order,
                )
                .await?;

            if chunks.is_empty() && source.preserves_previous_data {
                assemble_program_bytes(base_bytes, Vec::new())
            } else {
                assemble_program_bytes(base_bytes, chunks)
            }
        };

        let idl_text = match assembly.bytes.as_deref() {
            Some(bytes) => decode_idl_text(
                bytes,
                source.encoding.as_deref(),
                source.compression.as_deref(),
                source.format.as_deref(),
            )
            .ok()
            .flatten(),
            None => None,
        };

        self.upsert_program_idl_version_from_source(source.clone(), assembly, idl_text)
            .await?;
        self.update_event_idl_version_payload(&source.id).await?;
        self.program_idl_version_by_id(&source.program_address, &source.id)
            .await
    }

    async fn upsert_program_idl_version_from_source(
        &self,
        source: ProgramIdlVersionEventSource,
        assembly: crate::program_bytes::ProgramByteAssembly,
        idl_text: Option<String>,
    ) -> Result<()> {
        let missing_ranges_json = serde_json::to_string(&assembly.missing_ranges)?;
        let idl_bytes_blob_key = match (
            assembly.bytes.as_deref(),
            assembly.program_sha256.as_deref(),
        ) {
            (Some(bytes), Some(idl_sha256)) => {
                self.put_idl_version_blob(&source.program_address, idl_sha256, bytes)
                    .await?
            }
            _ => None,
        };
        let idl_text_blob_key = match (idl_text.as_deref(), assembly.program_sha256.as_deref()) {
            (Some(text), Some(idl_sha256)) => {
                self.put_idl_text_blob(&source.program_address, idl_sha256, text)
                    .await?
            }
            _ => None,
        };
        let idl_bytes_inline: Option<&[u8]> = if idl_bytes_blob_key.is_some() {
            None
        } else {
            assembly.bytes.as_deref()
        };
        let idl_text_inline = if idl_text_blob_key.is_some() {
            None
        } else {
            idl_text.as_deref()
        };
        let now = now_ts();

        sqlx::query(
            r#"
            INSERT INTO program_idl_versions (
                id,
                event_id,
                signature,
                instruction_path,
                instruction_order,
                kind,
                origin,
                program_address,
                metadata_account,
                source_account,
                authority,
                slot,
                block_time,
                indexed_at,
                complete,
                chunk_count,
                idl_len,
                idl_sha256,
                idl_bytes,
                idl_bytes_blob_key,
                idl_text,
                idl_text_blob_key,
                encoding,
                compression,
                format,
                data_source,
                missing_ranges_json,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                signature = excluded.signature,
                instruction_path = excluded.instruction_path,
                instruction_order = excluded.instruction_order,
                kind = excluded.kind,
                origin = excluded.origin,
                program_address = excluded.program_address,
                metadata_account = excluded.metadata_account,
                source_account = excluded.source_account,
                authority = excluded.authority,
                slot = excluded.slot,
                block_time = COALESCE(excluded.block_time, program_idl_versions.block_time),
                indexed_at = excluded.indexed_at,
                complete = excluded.complete,
                chunk_count = excluded.chunk_count,
                idl_len = excluded.idl_len,
                idl_sha256 = excluded.idl_sha256,
                idl_bytes = excluded.idl_bytes,
                idl_bytes_blob_key = excluded.idl_bytes_blob_key,
                idl_text = excluded.idl_text,
                idl_text_blob_key = excluded.idl_text_blob_key,
                encoding = excluded.encoding,
                compression = excluded.compression,
                format = excluded.format,
                data_source = excluded.data_source,
                missing_ranges_json = excluded.missing_ranges_json,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(&source.id)
        .bind(&source.event_id)
        .bind(&source.signature)
        .bind(&source.instruction_path)
        .bind(&source.instruction_order)
        .bind(&source.kind)
        .bind(&source.origin)
        .bind(&source.program_address)
        .bind(&source.metadata_account)
        .bind(&source.source_account)
        .bind(&source.authority)
        .bind(source.slot)
        .bind(source.block_time)
        .bind(source.indexed_at)
        .bind(if assembly.complete { 1_i64 } else { 0_i64 })
        .bind(assembly.chunk_count as i64)
        .bind(assembly.program_len)
        .bind(&assembly.program_sha256)
        .bind(idl_bytes_inline)
        .bind(idl_bytes_blob_key.as_deref())
        .bind(idl_text_inline)
        .bind(idl_text_blob_key.as_deref())
        .bind(&source.encoding)
        .bind(&source.compression)
        .bind(&source.format)
        .bind(&source.data_source)
        .bind(&missing_ranges_json)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_program_versions(
        &self,
        program_id: &str,
        limit: i64,
    ) -> Result<Vec<ProgramVersionRecord>> {
        let rows = sqlx::query(
            r#"
            WITH listed_versions AS (
                SELECT
                    id,
                    event_id,
                    signature,
                    instruction_path,
                    instruction_order,
                    kind,
                    program_address,
                    loader,
                    source_account,
                    authority,
                    slot,
                    block_time,
                    indexed_at,
                    complete,
                    chunk_count,
                    program_len,
                    program_sha256,
                    missing_ranges_json
                FROM program_versions
                WHERE program_address = ?
                ORDER BY slot DESC, instruction_order DESC, id DESC
                LIMIT ?
            ),
            versions_with_previous AS (
                SELECT
                    listed_versions.*,
                    (
                        SELECT previous.id
                        FROM program_versions previous
                        WHERE previous.program_address = listed_versions.program_address
                          AND previous.complete = 1
                          AND (
                                previous.program_bytes IS NOT NULL
                             OR previous.program_bytes_blob_key IS NOT NULL
                          )
                          AND (
                                previous.slot < listed_versions.slot
                             OR (
                                    previous.slot = listed_versions.slot
                                AND previous.instruction_order < listed_versions.instruction_order
                                )
                          )
                        ORDER BY previous.slot DESC, previous.instruction_order DESC, previous.id DESC
                        LIMIT 1
                    ) AS previous_version_id
                FROM listed_versions
            )
            SELECT
                versions_with_previous.*,
                previous.program_len AS previous_program_len,
                previous.program_sha256 AS previous_program_sha256
            FROM versions_with_previous
            LEFT JOIN program_versions previous
              ON previous.id = versions_with_previous.previous_version_id
            ORDER BY versions_with_previous.slot DESC,
                     versions_with_previous.instruction_order DESC,
                     versions_with_previous.id DESC;
            "#,
        )
        .bind(program_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            let slot = row.try_get::<i64, _>("slot")?;
            records.push(ProgramVersionRecord {
                id: row.try_get("id")?,
                event_id: row.try_get("event_id")?,
                signature: row.try_get("signature")?,
                instruction_path: row.try_get("instruction_path")?,
                kind: row.try_get("kind")?,
                program_address: row.try_get("program_address")?,
                loader: row.try_get("loader")?,
                source_account: row.try_get("source_account")?,
                authority: row.try_get("authority")?,
                slot,
                block_time: row.try_get("block_time")?,
                indexed_at: row.try_get("indexed_at")?,
                complete: row.try_get::<i64, _>("complete")? != 0,
                chunk_count: row.try_get("chunk_count")?,
                program_len: row.try_get("program_len")?,
                program_sha256: row.try_get("program_sha256")?,
                missing_ranges: serde_json::from_str(
                    &row.try_get::<String, _>("missing_ranges_json")?,
                )?,
                previous_version_id: row.try_get("previous_version_id")?,
                previous_program_len: row.try_get("previous_program_len")?,
                previous_program_sha256: row.try_get("previous_program_sha256")?,
            });
        }

        Ok(records)
    }

    pub async fn program_version_catalog_summaries(
        &self,
        program_ids: &[String],
    ) -> Result<Vec<ProgramVersionCatalogSummary>> {
        if program_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            WITH ordered_versions AS (
                SELECT
                    program_address,
                    id,
                    kind,
                    slot,
                    block_time,
                    complete,
                    program_len,
                    program_sha256,
                    instruction_order,
                    ROW_NUMBER() OVER (
                        PARTITION BY program_address
                        ORDER BY slot DESC, instruction_order DESC, id DESC
                    ) AS latest_rank
                FROM program_versions
                WHERE program_address IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for program_id in program_ids {
            separated.push_bind(program_id);
        }

        separated.push_unseparated(
            r#"
                )
            ),
            summarized_versions AS (
                SELECT
                    ordered_versions.*,
                    (
                        SELECT previous.id
                        FROM program_versions previous
                        WHERE previous.program_address = ordered_versions.program_address
                          AND previous.complete = 1
                          AND (
                                previous.program_bytes IS NOT NULL
                             OR previous.program_bytes_blob_key IS NOT NULL
                          )
                          AND (
                                previous.slot < ordered_versions.slot
                             OR (
                                    previous.slot = ordered_versions.slot
                                AND previous.instruction_order < ordered_versions.instruction_order
                                )
                          )
                        ORDER BY previous.slot DESC, previous.instruction_order DESC, previous.id DESC
                        LIMIT 1
                    ) AS previous_complete_version_id
                FROM ordered_versions
            )
            SELECT
                program_address,
                COUNT(*) AS version_count,
                COALESCE(SUM(CASE WHEN complete = 1 AND previous_complete_version_id IS NOT NULL THEN 1 ELSE 0 END), 0) AS diffable_count,
                COALESCE(SUM(CASE WHEN complete = 0 THEN 1 ELSE 0 END), 0) AS incomplete_count,
                MAX(CASE WHEN latest_rank = 1 THEN id END) AS latest_id,
                MAX(CASE WHEN latest_rank = 1 THEN kind END) AS latest_kind,
                MAX(CASE WHEN latest_rank = 1 THEN slot END) AS latest_slot,
                MAX(CASE WHEN latest_rank = 1 THEN block_time END) AS latest_block_time,
                MAX(CASE WHEN latest_rank = 1 THEN complete END) AS latest_complete,
                MAX(CASE WHEN latest_rank = 1 THEN program_len END) AS latest_program_len,
                MAX(CASE WHEN latest_rank = 1 THEN program_sha256 END) AS latest_program_sha256,
                MAX(CASE WHEN latest_rank = 1 THEN previous_complete_version_id END) AS latest_previous_version_id
            FROM summarized_versions
            GROUP BY program_address;
            "#,
        );

        let rows = builder.build().fetch_all(&self.pool).await?;
        let summaries = rows
            .into_iter()
            .map(|row| {
                let latest_id: Option<String> = row.try_get("latest_id")?;
                let latest = latest_id
                    .map(|id| {
                        Ok::<ProgramVersionLatestSummary, sqlx::Error>(
                            ProgramVersionLatestSummary {
                                id,
                                kind: row.try_get("latest_kind")?,
                                slot: row.try_get("latest_slot")?,
                                block_time: row.try_get("latest_block_time")?,
                                complete: row.try_get::<i64, _>("latest_complete")? != 0,
                                program_len: row.try_get("latest_program_len")?,
                                program_sha256: row.try_get("latest_program_sha256")?,
                                previous_version_id: row.try_get("latest_previous_version_id")?,
                            },
                        )
                    })
                    .transpose()?;

                Ok(ProgramVersionCatalogSummary {
                    program_id: row.try_get("program_address")?,
                    version_count: row.try_get("version_count")?,
                    diffable_count: row.try_get("diffable_count")?,
                    incomplete_count: row.try_get("incomplete_count")?,
                    latest,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(summaries)
    }

    pub async fn list_program_idl_versions(
        &self,
        program_id: &str,
        limit: i64,
    ) -> Result<Vec<ProgramIdlVersionRecord>> {
        let rows = sqlx::query(
            r#"
            WITH listed_versions AS (
                SELECT
                    id,
                    event_id,
                    signature,
                    instruction_path,
                    instruction_order,
                    kind,
                    origin,
                    program_address,
                    metadata_account,
                    source_account,
                    authority,
                    slot,
                    block_time,
                    indexed_at,
                    complete,
                    chunk_count,
                    idl_len,
                    idl_sha256,
                    encoding,
                    compression,
                    format,
                    data_source,
                    missing_ranges_json
                FROM program_idl_versions
                WHERE program_address = ?
                ORDER BY slot DESC, instruction_order DESC, id DESC
                LIMIT ?
            ),
            versions_with_previous AS (
                SELECT
                    listed_versions.*,
                    (
                        SELECT previous.id
                        FROM program_idl_versions previous
                        WHERE previous.program_address = listed_versions.program_address
                          AND previous.complete = 1
                          AND (
                                previous.idl_bytes IS NOT NULL
                             OR previous.idl_bytes_blob_key IS NOT NULL
                          )
                          AND (
                                previous.slot < listed_versions.slot
                             OR (
                                    previous.slot = listed_versions.slot
                                AND previous.instruction_order < listed_versions.instruction_order
                                )
                          )
                        ORDER BY previous.slot DESC, previous.instruction_order DESC, previous.id DESC
                        LIMIT 1
                    ) AS previous_idl_version_id
                FROM listed_versions
            )
            SELECT
                versions_with_previous.*,
                previous.idl_len AS previous_idl_len,
                previous.idl_sha256 AS previous_idl_sha256
            FROM versions_with_previous
            LEFT JOIN program_idl_versions previous
              ON previous.id = versions_with_previous.previous_idl_version_id
            ORDER BY versions_with_previous.slot DESC,
                     versions_with_previous.instruction_order DESC,
                     versions_with_previous.id DESC;
            "#,
        )
        .bind(program_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(ProgramIdlVersionRecord {
                id: row.try_get("id")?,
                event_id: row.try_get("event_id")?,
                signature: row.try_get("signature")?,
                instruction_path: row.try_get("instruction_path")?,
                kind: row.try_get("kind")?,
                origin: row.try_get("origin")?,
                program_address: row.try_get("program_address")?,
                metadata_account: row.try_get("metadata_account")?,
                source_account: row.try_get("source_account")?,
                authority: row.try_get("authority")?,
                slot: row.try_get("slot")?,
                block_time: row.try_get("block_time")?,
                indexed_at: row.try_get("indexed_at")?,
                complete: row.try_get::<i64, _>("complete")? != 0,
                chunk_count: row.try_get("chunk_count")?,
                idl_len: row.try_get("idl_len")?,
                idl_sha256: row.try_get("idl_sha256")?,
                encoding: row.try_get("encoding")?,
                compression: row.try_get("compression")?,
                format: row.try_get("format")?,
                data_source: row.try_get("data_source")?,
                missing_ranges: serde_json::from_str(
                    &row.try_get::<String, _>("missing_ranges_json")?,
                )?,
                previous_idl_version_id: row.try_get("previous_idl_version_id")?,
                previous_idl_len: row.try_get("previous_idl_len")?,
                previous_idl_sha256: row.try_get("previous_idl_sha256")?,
            });
        }

        Ok(records)
    }

    pub async fn program_idl_version_bytes(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query(
            r#"
            SELECT idl_bytes, idl_bytes_blob_key
            FROM program_idl_versions
            WHERE program_address = ?
              AND id = ?
              AND complete = 1
              AND (idl_bytes IS NOT NULL OR idl_bytes_blob_key IS NOT NULL)
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                self.blob_bytes_or_inline(
                    row.try_get("idl_bytes_blob_key")?,
                    row.try_get("idl_bytes")?,
                    "IDL version",
                )
                .await
            }
            None => Ok(None),
        }
    }

    pub async fn program_idl_version_text(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT idl_text, idl_text_blob_key
            FROM program_idl_versions
            WHERE program_address = ?
              AND id = ?
              AND complete = 1
              AND (idl_text IS NOT NULL OR idl_text_blob_key IS NOT NULL)
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                self.blob_text_or_inline(
                    row.try_get("idl_text_blob_key")?,
                    row.try_get("idl_text")?,
                    "IDL text",
                )
                .await
            }
            None => Ok(None),
        }
    }

    pub async fn program_version_bytes(
        &self,
        program_id: &str,
        version_id: &str,
        side: ProgramVersionByteSide,
    ) -> Result<Option<Vec<u8>>> {
        let Some(version) = self.program_version_lookup(program_id, version_id).await? else {
            return Ok(None);
        };

        match side {
            ProgramVersionByteSide::After => {
                self.complete_program_bytes_by_id(program_id, version_id)
                    .await
            }
            ProgramVersionByteSide::Before => {
                let Some(previous) = self
                    .previous_program_version_metadata(
                        program_id,
                        version.slot,
                        &version.instruction_order,
                    )
                    .await?
                else {
                    return Ok(None);
                };
                self.complete_program_bytes_by_id(program_id, &previous.id)
                    .await
            }
        }
    }

    pub async fn program_version_disassembly(
        &self,
        program_id: &str,
        version_id: &str,
        side: ProgramVersionByteSide,
    ) -> Result<Option<ProgramDisassemblyRecord>> {
        let Some(version) = self.program_version_lookup(program_id, version_id).await? else {
            return Ok(None);
        };

        let target_version_id = match side {
            ProgramVersionByteSide::After => version_id.to_owned(),
            ProgramVersionByteSide::Before => {
                let Some(previous) = self
                    .previous_program_version_metadata(
                        program_id,
                        version.slot,
                        &version.instruction_order,
                    )
                    .await?
                else {
                    return Ok(None);
                };
                previous.id
            }
        };

        if let Some(record) = self
            .program_disassembly_by_id(program_id, &target_version_id)
            .await?
            && record.format == DISASSEMBLY_FORMAT
        {
            return Ok(Some(record));
        }

        let Some(bytes) = self
            .complete_program_version_bytes_by_id(program_id, &target_version_id)
            .await?
        else {
            return Ok(None);
        };
        self.upsert_program_disassembly_for_bytes(
            &bytes.id,
            program_id,
            &bytes.program_sha256,
            &bytes.program_bytes,
        )
        .await?;

        self.program_disassembly_by_id(program_id, &target_version_id)
            .await
    }

    async fn upsert_program_disassembly_for_bytes(
        &self,
        version_id: &str,
        program_address: &str,
        program_sha256: &str,
        bytes: &[u8],
    ) -> Result<()> {
        if let Some(existing) = self
            .program_disassembly_by_id(program_address, version_id)
            .await?
            && existing.program_sha256 == program_sha256
            && existing.format == DISASSEMBLY_FORMAT
        {
            return Ok(());
        }

        let bytes = bytes.to_vec();
        let known_instructions = protocols::instruction_descriptors_for_program(program_address)
            .into_iter()
            .map(|instruction| KnownProgramInstruction {
                name: instruction.name,
                discriminator: instruction.discriminator,
            })
            .collect::<Vec<_>>();
        let disassembly = tokio::task::spawn_blocking(move || {
            disassemble_program_bytes(&bytes, &known_instructions)
        })
        .await
        .context("join sBPF disassembly task")??;
        let asm_text_blob_key = self
            .put_disassembly_blob(
                program_address,
                program_sha256,
                &disassembly.asm_sha256,
                &disassembly.asm_text,
            )
            .await?;
        let asm_text_inline = if asm_text_blob_key.is_some() {
            ""
        } else {
            disassembly.asm_text.as_str()
        };
        let now = now_ts();

        sqlx::query(
            r#"
            INSERT INTO program_version_disassemblies (
                version_id,
                program_address,
                program_sha256,
                format,
                instruction_count,
                entrypoint_instruction,
                asm_sha256,
                asm_text,
                asm_text_blob_key,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(version_id) DO UPDATE SET
                program_address = excluded.program_address,
                program_sha256 = excluded.program_sha256,
                format = excluded.format,
                instruction_count = excluded.instruction_count,
                entrypoint_instruction = excluded.entrypoint_instruction,
                asm_sha256 = excluded.asm_sha256,
                asm_text = excluded.asm_text,
                asm_text_blob_key = excluded.asm_text_blob_key,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(version_id)
        .bind(program_address)
        .bind(program_sha256)
        .bind(&disassembly.format)
        .bind(disassembly.instruction_count)
        .bind(disassembly.entrypoint_instruction)
        .bind(&disassembly.asm_sha256)
        .bind(asm_text_inline)
        .bind(asm_text_blob_key.as_deref())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn program_disassembly_by_id(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<ProgramDisassemblyRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                version_id,
                program_address,
                program_sha256,
                format,
                instruction_count,
                entrypoint_instruction,
                asm_sha256,
                asm_text,
                asm_text_blob_key,
                created_at,
                updated_at
            FROM program_version_disassemblies
            WHERE program_address = ?
              AND version_id = ?
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let asm_text = self
                    .blob_text_or_inline(
                        row.try_get("asm_text_blob_key")?,
                        Some(row.try_get("asm_text")?),
                        "program disassembly",
                    )
                    .await?
                    .context("program disassembly text missing")?;
                Ok(Some(ProgramDisassemblyRecord {
                    version_id: row.try_get("version_id")?,
                    program_address: row.try_get("program_address")?,
                    program_sha256: row.try_get("program_sha256")?,
                    format: row.try_get("format")?,
                    instruction_count: row.try_get("instruction_count")?,
                    entrypoint_instruction: row.try_get("entrypoint_instruction")?,
                    asm_sha256: row.try_get("asm_sha256")?,
                    asm_text,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn upsert_program_text_diff_for_version(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<()> {
        let Some(version) = self.program_version_lookup(program_id, version_id).await? else {
            return Ok(());
        };
        let Some(previous) = self
            .previous_program_version_metadata(program_id, version.slot, &version.instruction_order)
            .await?
        else {
            return Ok(());
        };

        let Some(before) = self
            .program_version_disassembly(program_id, version_id, ProgramVersionByteSide::Before)
            .await?
        else {
            return Ok(());
        };
        let Some(after) = self
            .program_version_disassembly(program_id, version_id, ProgramVersionByteSide::After)
            .await?
        else {
            return Ok(());
        };

        if before.program_sha256 == after.program_sha256 {
            return Ok(());
        }

        let diff_text =
            unified_asm_diff(&previous.id, version_id, &before.asm_text, &after.asm_text);
        let diff_sha256 = sha256_hex(diff_text.as_bytes());
        let diff_text_blob_key = self
            .put_program_diff_blob(
                program_id,
                version.slot,
                version_id,
                &before.program_sha256,
                &after.program_sha256,
                &diff_sha256,
                &diff_text,
            )
            .await?;
        let diff_text_inline = if diff_text_blob_key.is_some() {
            ""
        } else {
            diff_text.as_str()
        };
        let now = now_ts();

        sqlx::query(
            r#"
            INSERT INTO program_version_diffs (
                version_id,
                program_address,
                previous_version_id,
                before_program_sha256,
                after_program_sha256,
                format,
                diff_sha256,
                diff_text,
                diff_text_blob_key,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(version_id) DO UPDATE SET
                program_address = excluded.program_address,
                previous_version_id = excluded.previous_version_id,
                before_program_sha256 = excluded.before_program_sha256,
                after_program_sha256 = excluded.after_program_sha256,
                format = excluded.format,
                diff_sha256 = excluded.diff_sha256,
                diff_text = excluded.diff_text,
                diff_text_blob_key = excluded.diff_text_blob_key,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(version_id)
        .bind(program_id)
        .bind(&previous.id)
        .bind(&before.program_sha256)
        .bind(&after.program_sha256)
        .bind(DISASSEMBLY_FORMAT)
        .bind(&diff_sha256)
        .bind(diff_text_inline)
        .bind(diff_text_blob_key.as_deref())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn previous_program_version_order_for_source(
        &self,
        source_account: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<ProgramVersionOrder>> {
        let row = sqlx::query(
            r#"
            SELECT slot, instruction_order
            FROM program_versions
            WHERE source_account = ?
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(source_account)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(ProgramVersionOrder {
                slot: row.try_get("slot")?,
                instruction_order: row.try_get("instruction_order")?,
            })
        })
        .transpose()
    }

    async fn previous_complete_program_bytes(
        &self,
        program_id: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<Vec<u8>>> {
        let row = sqlx::query(
            r#"
            SELECT program_bytes, program_bytes_blob_key
            FROM program_versions
            WHERE program_address = ?
              AND complete = 1
              AND (program_bytes IS NOT NULL OR program_bytes_blob_key IS NOT NULL)
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                self.blob_bytes_or_inline(
                    row.try_get("program_bytes_blob_key")?,
                    row.try_get("program_bytes")?,
                    "previous program version",
                )
                .await
            }
            None => Ok(None),
        }
    }

    async fn program_data_chunks_for_source(
        &self,
        source_account: &str,
        lower_bound: Option<&ProgramVersionOrder>,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Vec<ProgramDataChunk>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT offset, bytes, bytes_blob_key
            FROM program_data_writes
            WHERE write_account =
            "#,
        );
        builder.push_bind(source_account.to_owned());

        if let Some(lower_bound) = lower_bound {
            builder.push(
                r#"
                AND (
	                       slot >
                "#,
            );
            builder.push_bind(lower_bound.slot);
            builder.push(" OR (slot = ");
            builder.push_bind(lower_bound.slot);
            builder.push(" AND instruction_order > ");
            builder.push_bind(lower_bound.instruction_order.clone());
            builder.push("))");
        }

        builder.push(
            r#"
              AND (
	                    slot <
            "#,
        );
        builder.push_bind(slot);
        builder.push(" OR (slot = ");
        builder.push_bind(slot);
        builder.push(" AND instruction_order <= ");
        builder.push_bind(instruction_order.to_owned());
        builder.push(")) ORDER BY slot ASC, instruction_order ASC, id ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut chunks = Vec::with_capacity(rows.len());
        for row in rows {
            let offset = row.try_get::<i64, _>("offset")?;
            let bytes = self
                .blob_bytes_or_inline(
                    row.try_get("bytes_blob_key")?,
                    Some(row.try_get("bytes")?),
                    "program data write",
                )
                .await?
                .context("program data write bytes missing")?;
            chunks.push(ProgramDataChunk {
                offset: u32::try_from(offset).context("program data write offset overflow")?,
                bytes,
            });
        }
        Ok(chunks)
    }

    async fn prune_program_data_write_bytes_for_source(
        &self,
        source_account: &str,
        lower_bound: Option<&ProgramVersionOrder>,
        slot: i64,
        instruction_order: &str,
    ) -> Result<()> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            UPDATE program_data_writes
            SET bytes = X'', bytes_blob_key = NULL
            WHERE write_account =
            "#,
        );
        builder.push_bind(source_account.to_owned());

        if let Some(lower_bound) = lower_bound {
            builder.push(
                r#"
                AND (
                       slot >
                "#,
            );
            builder.push_bind(lower_bound.slot);
            builder.push(" OR (slot = ");
            builder.push_bind(lower_bound.slot);
            builder.push(" AND instruction_order > ");
            builder.push_bind(lower_bound.instruction_order.clone());
            builder.push("))");
        }

        builder.push(
            r#"
              AND (
                    slot <
            "#,
        );
        builder.push_bind(slot);
        builder.push(" OR (slot = ");
        builder.push_bind(slot);
        builder.push(" AND instruction_order <= ");
        builder.push_bind(instruction_order.to_owned());
        builder.push(")) AND length(bytes) > 0");

        builder.build().execute(&self.pool).await?;
        Ok(())
    }

    async fn program_data_write_bytes_len_for_source(
        &self,
        source_account: &str,
        lower_bound: Option<&ProgramVersionOrder>,
        slot: i64,
        instruction_order: &str,
    ) -> Result<u64> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT COALESCE(SUM(length(bytes)), 0) AS bytes_len
            FROM program_data_writes
            WHERE write_account =
            "#,
        );
        builder.push_bind(source_account.to_owned());

        if let Some(lower_bound) = lower_bound {
            builder.push(
                r#"
                AND (
                       slot >
                "#,
            );
            builder.push_bind(lower_bound.slot);
            builder.push(" OR (slot = ");
            builder.push_bind(lower_bound.slot);
            builder.push(" AND instruction_order > ");
            builder.push_bind(lower_bound.instruction_order.clone());
            builder.push("))");
        }

        builder.push(
            r#"
              AND (
                    slot <
            "#,
        );
        builder.push_bind(slot);
        builder.push(" OR (slot = ");
        builder.push_bind(slot);
        builder.push(" AND instruction_order <= ");
        builder.push_bind(instruction_order.to_owned());
        builder.push("))");

        let bytes_len = builder
            .build()
            .fetch_one(&self.pool)
            .await?
            .try_get::<i64, _>("bytes_len")?;
        Ok(bytes_len.max(0) as u64)
    }

    async fn program_idl_chunks_for_source(
        &self,
        source_account: &str,
        lower_bound: Option<&ProgramVersionOrder>,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Vec<ProgramDataChunk>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT offset, bytes, bytes_blob_key
            FROM program_idl_writes
            WHERE source_account =
            "#,
        );
        builder.push_bind(source_account.to_owned());

        if let Some(lower_bound) = lower_bound {
            builder.push(
                r#"
                AND (
	                       slot >
                "#,
            );
            builder.push_bind(lower_bound.slot);
            builder.push(" OR (slot = ");
            builder.push_bind(lower_bound.slot);
            builder.push(" AND instruction_order > ");
            builder.push_bind(lower_bound.instruction_order.clone());
            builder.push("))");
        }

        builder.push(
            r#"
              AND (
	                    slot <
            "#,
        );
        builder.push_bind(slot);
        builder.push(" OR (slot = ");
        builder.push_bind(slot);
        builder.push(" AND instruction_order <= ");
        builder.push_bind(instruction_order.to_owned());
        builder.push(")) ORDER BY slot ASC, instruction_order ASC, id ASC");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut append_offset = 0_u32;
        let mut chunks = Vec::with_capacity(rows.len());
        for row in rows {
            let bytes = self
                .blob_bytes_or_inline(
                    row.try_get("bytes_blob_key")?,
                    Some(row.try_get("bytes")?),
                    "IDL data write",
                )
                .await?
                .context("IDL data write bytes missing")?;
            let offset = match row.try_get::<Option<i64>, _>("offset")? {
                Some(offset) => u32::try_from(offset).context("IDL data write offset overflow")?,
                None => {
                    let offset = append_offset;
                    append_offset = append_offset
                        .checked_add(
                            u32::try_from(bytes.len()).context("IDL append length overflow")?,
                        )
                        .context("IDL append offset overflow")?;
                    offset
                }
            };
            chunks.push(ProgramDataChunk { offset, bytes });
        }
        Ok(chunks)
    }

    async fn update_event_program_version_payload(
        &self,
        summary: &ProgramVersionSummary,
    ) -> Result<()> {
        let Some(row) = sqlx::query(
            r#"
            SELECT payload_json
            FROM events
            WHERE id = ?;
            "#,
        )
        .bind(&summary.event_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(());
        };

        let payload_json = row.try_get::<String, _>("payload_json")?;
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)?;
        apply_program_version_payload(&mut payload, summary)?;
        sqlx::query(
            r#"
            UPDATE events
            SET payload_json = ?
            WHERE id = ?;
            "#,
        )
        .bind(serde_json::to_string(&payload)?)
        .bind(&summary.event_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_event_idl_version_payload(&self, version_id: &str) -> Result<()> {
        let Some(version) = self.program_idl_version_by_any_id(version_id).await? else {
            return Ok(());
        };
        let Some(row) = sqlx::query(
            r#"
            SELECT payload_json
            FROM events
            WHERE id = ?;
            "#,
        )
        .bind(&version.event_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(());
        };

        let payload_json = row.try_get::<String, _>("payload_json")?;
        let mut payload = serde_json::from_str::<serde_json::Value>(&payload_json)?;
        apply_idl_version_payload(&mut payload, &version)?;
        sqlx::query(
            r#"
            UPDATE events
            SET payload_json = ?
            WHERE id = ?;
            "#,
        )
        .bind(serde_json::to_string(&payload)?)
        .bind(&version.event_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn previous_program_version_metadata(
        &self,
        program_id: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<ProgramVersionMetadata>> {
        let row = sqlx::query(
            r#"
            SELECT id
            FROM program_versions
            WHERE program_address = ?
              AND complete = 1
              AND (program_bytes IS NOT NULL OR program_bytes_blob_key IS NOT NULL)
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(ProgramVersionMetadata {
                id: row.try_get("id")?,
            })
        })
        .transpose()
    }

    async fn previous_idl_version_order_for_source(
        &self,
        source_account: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<ProgramVersionOrder>> {
        let row = sqlx::query(
            r#"
            SELECT slot, instruction_order
            FROM program_idl_versions
            WHERE source_account = ?
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(source_account)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(ProgramVersionOrder {
                slot: row.try_get("slot")?,
                instruction_order: row.try_get("instruction_order")?,
            })
        })
        .transpose()
    }

    async fn previous_idl_version_metadata(
        &self,
        program_id: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<IdlVersionMetadata>> {
        let row = sqlx::query(
            r#"
            SELECT id, idl_len, idl_sha256
            FROM program_idl_versions
            WHERE program_address = ?
              AND complete = 1
              AND (idl_bytes IS NOT NULL OR idl_bytes_blob_key IS NOT NULL)
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(IdlVersionMetadata {
                id: row.try_get("id")?,
                idl_len: row.try_get("idl_len")?,
                idl_sha256: row.try_get("idl_sha256")?,
            })
        })
        .transpose()
    }

    async fn previous_complete_idl_bytes(
        &self,
        program_id: &str,
        slot: i64,
        instruction_order: &str,
    ) -> Result<Option<CompleteIdlVersionBytes>> {
        let row = sqlx::query(
            r#"
            SELECT idl_bytes, idl_bytes_blob_key
            FROM program_idl_versions
            WHERE program_address = ?
              AND complete = 1
              AND idl_sha256 IS NOT NULL
              AND (idl_bytes IS NOT NULL OR idl_bytes_blob_key IS NOT NULL)
              AND (
                    slot < ?
                 OR (slot = ? AND instruction_order < ?)
              )
            ORDER BY slot DESC, instruction_order DESC, id DESC
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(slot)
        .bind(slot)
        .bind(instruction_order)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let idl_bytes = self
                    .blob_bytes_or_inline(
                        row.try_get("idl_bytes_blob_key")?,
                        row.try_get("idl_bytes")?,
                        "previous IDL version",
                    )
                    .await?
                    .context("previous IDL version bytes missing")?;
                Ok(Some(CompleteIdlVersionBytes { idl_bytes }))
            }
            None => Ok(None),
        }
    }

    async fn program_idl_version_by_id(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<ProgramIdlVersionRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                event_id,
                signature,
                instruction_path,
                instruction_order,
                kind,
                origin,
                program_address,
                metadata_account,
                source_account,
                authority,
                slot,
                block_time,
                indexed_at,
                complete,
                chunk_count,
                idl_len,
                idl_sha256,
                encoding,
                compression,
                format,
                data_source,
                missing_ranges_json
            FROM program_idl_versions
            WHERE program_address = ?
              AND id = ?
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_program_idl_version_record(row).await?)),
            None => Ok(None),
        }
    }

    async fn program_idl_version_by_any_id(
        &self,
        version_id: &str,
    ) -> Result<Option<ProgramIdlVersionRecord>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                event_id,
                signature,
                instruction_path,
                instruction_order,
                kind,
                origin,
                program_address,
                metadata_account,
                source_account,
                authority,
                slot,
                block_time,
                indexed_at,
                complete,
                chunk_count,
                idl_len,
                idl_sha256,
                encoding,
                compression,
                format,
                data_source,
                missing_ranges_json
            FROM program_idl_versions
            WHERE id = ?
            LIMIT 1;
            "#,
        )
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_program_idl_version_record(row).await?)),
            None => Ok(None),
        }
    }

    async fn row_to_program_idl_version_record(
        &self,
        row: sqlx::sqlite::SqliteRow,
    ) -> Result<ProgramIdlVersionRecord> {
        let program_address = row.try_get::<String, _>("program_address")?;
        let slot = row.try_get::<i64, _>("slot")?;
        let instruction_order = row.try_get::<String, _>("instruction_order")?;
        let previous = self
            .previous_idl_version_metadata(&program_address, slot, &instruction_order)
            .await?;

        Ok(ProgramIdlVersionRecord {
            id: row.try_get("id")?,
            event_id: row.try_get("event_id")?,
            signature: row.try_get("signature")?,
            instruction_path: row.try_get("instruction_path")?,
            kind: row.try_get("kind")?,
            origin: row.try_get("origin")?,
            program_address,
            metadata_account: row.try_get("metadata_account")?,
            source_account: row.try_get("source_account")?,
            authority: row.try_get("authority")?,
            slot,
            block_time: row.try_get("block_time")?,
            indexed_at: row.try_get("indexed_at")?,
            complete: row.try_get::<i64, _>("complete")? != 0,
            chunk_count: row.try_get("chunk_count")?,
            idl_len: row.try_get("idl_len")?,
            idl_sha256: row.try_get("idl_sha256")?,
            encoding: row.try_get("encoding")?,
            compression: row.try_get("compression")?,
            format: row.try_get("format")?,
            data_source: row.try_get("data_source")?,
            missing_ranges: serde_json::from_str(
                &row.try_get::<String, _>("missing_ranges_json")?,
            )?,
            previous_idl_version_id: previous.as_ref().map(|record| record.id.clone()),
            previous_idl_len: previous.as_ref().and_then(|record| record.idl_len),
            previous_idl_sha256: previous.and_then(|record| record.idl_sha256),
        })
    }

    async fn program_version_lookup(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<ProgramVersionLookup>> {
        let row = sqlx::query(
            r#"
            SELECT slot, instruction_order
            FROM program_versions
            WHERE program_address = ?
              AND id = ?
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(ProgramVersionLookup {
                slot: row.try_get("slot")?,
                instruction_order: row.try_get("instruction_order")?,
            })
        })
        .transpose()
    }

    async fn complete_program_bytes_by_id(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self
            .complete_program_version_bytes_by_id(program_id, version_id)
            .await?
            .map(|record| record.program_bytes))
    }

    async fn complete_program_version_bytes_by_id(
        &self,
        program_id: &str,
        version_id: &str,
    ) -> Result<Option<CompleteProgramVersionBytes>> {
        let row = sqlx::query(
            r#"
            SELECT id, program_sha256, program_bytes, program_bytes_blob_key
            FROM program_versions
            WHERE program_address = ?
              AND id = ?
              AND complete = 1
              AND program_sha256 IS NOT NULL
              AND (program_bytes IS NOT NULL OR program_bytes_blob_key IS NOT NULL)
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(version_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let program_bytes = self
                    .blob_bytes_or_inline(
                        row.try_get("program_bytes_blob_key")?,
                        row.try_get("program_bytes")?,
                        "program version",
                    )
                    .await?
                    .context("program version bytes missing")?;
                Ok(Some(CompleteProgramVersionBytes {
                    id: row.try_get("id")?,
                    program_sha256: row.try_get("program_sha256")?,
                    program_bytes,
                }))
            }
            None => Ok(None),
        }
    }

    pub async fn list_events_filtered(
        &self,
        limit: i64,
        offset: i64,
        filters: EventListFilters<'_>,
    ) -> Result<Vec<ChainEvent>> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                kind,
                target_type,
                target_address,
                actor,
                slot,
                block_time,
                indexed_at,
                summary,
                tags_json,
                payload_json,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            FROM events
            "#,
        );

        let mut has_where = false;
        push_event_filters(&mut builder, &mut has_where, &filters);

        builder.push(" ORDER BY slot DESC, indexed_at DESC, id DESC");
        builder.push(" LIMIT ").push_bind(limit);
        builder.push(" OFFSET ").push_bind(offset);

        let rows = builder.build().fetch_all(&self.pool).await?;
        rows.into_iter().map(row_to_event).collect()
    }

    pub async fn event_by_id(&self, id: &str) -> Result<Option<ChainEvent>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                kind,
                target_type,
                target_address,
                actor,
                slot,
                block_time,
                indexed_at,
                summary,
                tags_json,
                payload_json,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            FROM events
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_event).transpose()
    }

    pub async fn event_scope_counts(
        &self,
        filters: EventListFilters<'_>,
    ) -> Result<EventScopeCounts> {
        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                COUNT(*) AS all_count,
                COALESCE(SUM(CASE WHEN
                    (tags_json LIKE '%"severity:critical"%' OR tags_json LIKE '%"signal:unknown_instruction"%')
                THEN 1 ELSE 0 END), 0) AS critical_count,
                COALESCE(SUM(CASE WHEN tags_json LIKE '%"severity:high"%' THEN 1 ELSE 0 END), 0) AS high_count,
                COALESCE(SUM(CASE WHEN tags_json LIKE '%"severity:standard"%' THEN 1 ELSE 0 END), 0) AS standard_count,
                COALESCE(SUM(CASE WHEN tags_json LIKE '%"watch:defi"%' THEN 1 ELSE 0 END), 0) AS defi_count,
                COALESCE(SUM(CASE WHEN kind IN (
                    'program_deployed',
                    'program_upgraded',
                    'program_finalized',
                    'program_migrated_to_loader_v4'
                ) THEN 1 ELSE 0 END), 0) AS programs_count,
                COALESCE(SUM(CASE WHEN kind IN (
                    'program_idl_initialized',
                    'program_idl_updated',
                    'program_idl_authority_changed',
                    'program_idl_immutable',
                    'program_idl_resized',
                    'program_idl_closed'
                ) THEN 1 ELSE 0 END), 0) AS idl_count,
                COALESCE(SUM(CASE WHEN target_type = 'multisig' THEN 1 ELSE 0 END), 0) AS multisig_count
            FROM events
            "#,
        );
        let mut has_where = false;
        push_event_filters(
            &mut builder,
            &mut has_where,
            &EventListFilters {
                scope: None,
                ..filters
            },
        );

        let row = builder.build().fetch_one(&self.pool).await?;
        Ok(EventScopeCounts {
            all: row.try_get::<i64, _>("all_count")?,
            critical: row.try_get::<i64, _>("critical_count")?,
            high: row.try_get::<i64, _>("high_count")?,
            standard: row.try_get::<i64, _>("standard_count")?,
            defi: row.try_get::<i64, _>("defi_count")?,
            programs: row.try_get::<i64, _>("programs_count")?,
            idl: row.try_get::<i64, _>("idl_count")?,
            multisig: row.try_get::<i64, _>("multisig_count")?,
        })
    }

    pub async fn protocol_activity_summaries(&self) -> Result<Vec<ProtocolActivitySummary>> {
        let rows = sqlx::query(
            r#"
            SELECT
                substr(protocol_tag.value, length('protocol:') + 1) AS slug,
                COUNT(*) AS total_count,
                COALESCE(SUM(CASE WHEN
                    (events.tags_json LIKE '%"severity:critical"%' OR events.tags_json LIKE '%"signal:unknown_instruction"%')
                THEN 1 ELSE 0 END), 0) AS critical_count,
                COALESCE(SUM(CASE WHEN events.tags_json LIKE '%"severity:high"%' THEN 1 ELSE 0 END), 0) AS high_count,
                COALESCE(SUM(CASE WHEN events.tags_json LIKE '%"severity:standard"%' THEN 1 ELSE 0 END), 0) AS standard_count
            FROM events, json_each(events.tags_json) AS protocol_tag
            WHERE protocol_tag.value LIKE 'protocol:%'
            GROUP BY slug
            ORDER BY total_count DESC, critical_count DESC, slug ASC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ProtocolActivitySummary {
                    slug: row.try_get("slug")?,
                    total: row.try_get("total_count")?,
                    critical: row.try_get("critical_count")?,
                    high: row.try_get("high_count")?,
                    standard: row.try_get("standard_count")?,
                })
            })
            .collect()
    }

    pub async fn program_risk_summaries(
        &self,
        program_ids: &[String],
    ) -> Result<HashMap<String, ProgramRiskSummary>> {
        if program_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                e.target_address,
                COALESCE(SUM(CASE WHEN er.event_id IS NOT NULL THEN 1 ELSE 0 END), 0) AS reviewed_critical,
                COALESCE(SUM(CASE WHEN er.event_id IS NULL THEN 1 ELSE 0 END), 0) AS unreviewed_critical
            FROM events e
            LEFT JOIN event_reviews er
              ON er.event_id = e.id
             AND er.reviewed_at IS NOT NULL
            WHERE e.target_address IN (
            "#,
        );

        let mut separated = builder.separated(", ");
        for program_id in program_ids {
            separated.push_bind(program_id.to_owned());
        }
        separated.push_unseparated(") AND ");
        push_critical_condition(&mut builder);
        builder.push(" GROUP BY e.target_address");

        let rows = builder.build().fetch_all(&self.pool).await?;
        let mut summaries = HashMap::with_capacity(rows.len());
        for row in rows {
            let program_id = row.try_get::<String, _>("target_address")?;
            let reviewed_critical = row.try_get::<i64, _>("reviewed_critical")?;
            let unreviewed_critical = row.try_get::<i64, _>("unreviewed_critical")?;
            summaries.insert(
                program_id,
                ProgramRiskSummary::from_counts(reviewed_critical, unreviewed_critical),
            );
        }

        Ok(summaries)
    }

    pub async fn protocol_related_multisigs(&self, protocol_slug: &str) -> Result<Vec<String>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT json_extract(payload_json, '$.multisig') AS multisig
            FROM events
            WHERE tags_json LIKE ?
              AND json_extract(payload_json, '$.multisig') IS NOT NULL
              AND json_extract(payload_json, '$.multisig') <> ''
            ORDER BY multisig ASC;
            "#,
        )
        .bind(format!("%\"protocol:{protocol_slug}\"%"))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<Option<String>, _>("multisig").ok().flatten())
            .collect())
    }

    pub async fn prior_program_multisig_member_actions(
        &self,
        program_id: &str,
        multisig: &str,
        member: &str,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM events
            WHERE target_type = 'program'
              AND target_address = ?
              AND json_extract(payload_json, '$.multisig') = ?
              AND json_extract(payload_json, '$.member') = ?;
            "#,
        )
        .bind(program_id)
        .bind(multisig)
        .bind(member)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get::<i64, _>("count")?)
    }

    pub async fn prior_program_multisig_total_actions(
        &self,
        program_id: &str,
        multisig: &str,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
            FROM events
            WHERE target_type = 'program'
              AND target_address = ?
              AND json_extract(payload_json, '$.multisig') = ?;
            "#,
        )
        .bind(program_id)
        .bind(multisig)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.try_get::<i64, _>("count")?)
    }

    pub async fn top_program_multisig_signer(
        &self,
        program_id: &str,
        multisig: &str,
    ) -> Result<Option<(String, i64)>> {
        let row = sqlx::query(
            r#"
            SELECT
                json_extract(payload_json, '$.member') AS member,
                COUNT(*) AS count
            FROM events
            WHERE target_type = 'program'
              AND target_address = ?
              AND json_extract(payload_json, '$.multisig') = ?
              AND json_extract(payload_json, '$.member') IS NOT NULL
            GROUP BY member
            ORDER BY count DESC, member ASC
            LIMIT 1;
            "#,
        )
        .bind(program_id)
        .bind(multisig)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| Ok((row.try_get("member")?, row.try_get("count")?)))
            .transpose()
    }

    pub async fn program_signer_stats(
        &self,
        program_id: &str,
        limit: i64,
    ) -> Result<Vec<ProgramSignerStat>> {
        let rows = sqlx::query(
            r#"
            SELECT
                json_extract(payload_json, '$.member') AS member,
                COUNT(*) AS actions,
                MAX(slot) AS last_slot,
                COUNT(DISTINCT json_extract(payload_json, '$.multisig')) AS multisig_count
            FROM events
            WHERE target_type = 'program'
              AND target_address = ?
              AND json_extract(payload_json, '$.multisig') IS NOT NULL
              AND json_extract(payload_json, '$.member') IS NOT NULL
            GROUP BY member
            ORDER BY actions DESC, last_slot DESC, member ASC
            LIMIT ?;
            "#,
        )
        .bind(program_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ProgramSignerStat {
                member: row.try_get("member").unwrap_or_default(),
                member_label: None,
                actions: row.try_get("actions").unwrap_or_default(),
                last_slot: row.try_get("last_slot").unwrap_or_default(),
                multisig_count: row.try_get("multisig_count").unwrap_or_default(),
            })
            .collect())
    }

    pub async fn address_protocol_links(
        &self,
        address: &str,
        limit: i64,
    ) -> Result<Vec<AddressProtocolLink>> {
        let rows = sqlx::query(
            r#"
            SELECT
                tags_json,
                CASE
                    WHEN target_type = 'program' AND target_address = ? THEN 'program_surface'
                    WHEN target_type = 'multisig' AND target_address = ? THEN 'multisig'
                    WHEN json_extract(payload_json, '$.multisig') = ? THEN 'multisig'
                    WHEN json_extract(payload_json, '$.member') = ? THEN 'multisig_signer'
                    WHEN actor = ? THEN 'actor'
                    WHEN json_extract(payload_json, '$.owner') = ? THEN 'domain_owner'
                    WHEN json_extract(payload_json, '$.new_owner') = ? THEN 'domain_new_owner'
                    WHEN json_extract(payload_json, '$.refund_target') = ? THEN 'domain_refund_target'
                    ELSE 'related'
                END AS evidence
            FROM events
            WHERE
                target_type <> 'domain'
                AND (
                    target_address = ?
                    OR actor = ?
                    OR json_extract(payload_json, '$.multisig') = ?
                    OR json_extract(payload_json, '$.member') = ?
                    OR json_extract(payload_json, '$.owner') = ?
                    OR json_extract(payload_json, '$.new_owner') = ?
                    OR json_extract(payload_json, '$.refund_target') = ?
                )
            ORDER BY slot DESC, indexed_at DESC
            LIMIT ?;
            "#,
        )
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(address)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut dedup = HashMap::<String, String>::new();
        for row in rows {
            let evidence = row
                .try_get::<String, _>("evidence")
                .unwrap_or_else(|_| "related".to_owned());
            let tags_json = row.try_get::<String, _>("tags_json").unwrap_or_default();
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();

            for slug in tags
                .into_iter()
                .filter_map(|tag| tag.strip_prefix("protocol:").map(str::to_owned))
                .filter(|slug| slug != "squads")
            {
                dedup.entry(slug).or_insert_with(|| evidence.clone());
            }
        }

        Ok(dedup
            .into_iter()
            .map(|(protocol_slug, evidence)| AddressProtocolLink {
                protocol_slug,
                evidence,
            })
            .collect())
    }

    pub async fn insert_protocol_metric_observation(
        &self,
        observation: &ProtocolMetricObservation,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO protocol_metric_observations (
                id,
                signature,
                instruction_path,
                slot,
                block_time,
                protocol_slug,
                protocol_name,
                program_id,
                program_label,
                instruction_name,
                metric_key,
                metric_label,
                metric_value,
                subject_address,
                subject_kind,
                subject_label,
                payload_json,
                observed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO NOTHING;
            "#,
        )
        .bind(&observation.id)
        .bind(&observation.signature)
        .bind(&observation.instruction_path)
        .bind(observation.slot)
        .bind(observation.block_time)
        .bind(&observation.protocol_slug)
        .bind(&observation.protocol_name)
        .bind(&observation.program_id)
        .bind(&observation.program_label)
        .bind(&observation.instruction_name)
        .bind(&observation.metric_key)
        .bind(&observation.metric_label)
        .bind(observation.metric_value)
        .bind(&observation.subject_address)
        .bind(&observation.subject_kind)
        .bind(&observation.subject_label)
        .bind(serde_json::to_string(&observation.payload)?)
        .bind(observation.observed_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn previous_protocol_metric_observation(
        &self,
        observation: &ProtocolMetricObservation,
    ) -> Result<Option<ProtocolMetricObservation>> {
        let row = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                slot,
                block_time,
                protocol_slug,
                protocol_name,
                program_id,
                program_label,
                instruction_name,
                metric_key,
                metric_label,
                metric_value,
                subject_address,
                subject_kind,
                subject_label,
                payload_json,
                observed_at
            FROM protocol_metric_observations
            WHERE protocol_slug = ?
              AND instruction_name = ?
              AND metric_key = ?
              AND ((subject_address IS NULL AND ? IS NULL) OR subject_address = ?)
              AND id != ?
            ORDER BY slot DESC, observed_at DESC
            LIMIT 1;
            "#,
        )
        .bind(&observation.protocol_slug)
        .bind(&observation.instruction_name)
        .bind(&observation.metric_key)
        .bind(&observation.subject_address)
        .bind(&observation.subject_address)
        .bind(&observation.id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_protocol_metric_observation).transpose()
    }

    pub async fn recent_protocol_metric_presets(
        &self,
        limit: i64,
    ) -> Result<Vec<MetricAlertPreset>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                signature,
                instruction_path,
                slot,
                block_time,
                protocol_slug,
                protocol_name,
                program_id,
                program_label,
                instruction_name,
                metric_key,
                metric_label,
                metric_value,
                subject_address,
                subject_kind,
                subject_label,
                payload_json,
                observed_at
            FROM protocol_metric_observations
            ORDER BY slot DESC, observed_at DESC
            LIMIT ?;
            "#,
        )
        .bind(limit.max(1))
        .fetch_all(&self.pool)
        .await?;

        let mut grouped = HashMap::<
            (String, String, String, Option<String>),
            Vec<ProtocolMetricObservation>,
        >::new();
        for row in rows {
            let observation = row_to_protocol_metric_observation(row)?;
            grouped
                .entry((
                    observation.protocol_slug.clone(),
                    observation.instruction_name.clone(),
                    observation.metric_key.clone(),
                    observation.subject_address.clone(),
                ))
                .or_default()
                .push(observation);
        }

        let mut presets = grouped
            .into_values()
            .filter_map(|mut observations| {
                observations.sort_by(|left, right| {
                    right
                        .slot
                        .cmp(&left.slot)
                        .then_with(|| right.observed_at.cmp(&left.observed_at))
                });
                let latest = observations.first()?;
                let min_value = observations
                    .iter()
                    .map(|observation| observation.metric_value)
                    .fold(f64::INFINITY, f64::min);
                let max_value = observations
                    .iter()
                    .map(|observation| observation.metric_value)
                    .fold(f64::NEG_INFINITY, f64::max);
                let suggestion_padding = (latest.metric_value.abs() * 0.02).max(1.0);

                Some(MetricAlertPreset {
                    protocol_slug: latest.protocol_slug.clone(),
                    protocol_name: latest.protocol_name.clone(),
                    instruction_name: latest.instruction_name.clone(),
                    metric_key: latest.metric_key.clone(),
                    metric_label: latest.metric_label.clone(),
                    subject_address: latest.subject_address.clone(),
                    subject_kind: latest.subject_kind.clone(),
                    subject_label: latest.subject_label.clone(),
                    latest_value: latest.metric_value,
                    suggested_min_value: Some(min_value - suggestion_padding),
                    suggested_max_value: Some(max_value + suggestion_padding),
                    observation_count: observations.len(),
                    latest_slot: latest.slot,
                })
            })
            .collect::<Vec<_>>();

        presets.sort_by(|left, right| right.latest_slot.cmp(&left.latest_slot));
        Ok(presets)
    }

    pub async fn list_metric_alert_rules(&self) -> Result<Vec<MetricAlertRule>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                label,
                protocol_slug,
                protocol_name,
                instruction_name,
                metric_key,
                metric_label,
                subject_address,
                subject_kind,
                subject_label,
                min_value,
                max_value,
                created_at
            FROM metric_alert_rules
            ORDER BY created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_metric_alert_rule).collect()
    }

    pub async fn create_metric_alert_rule(
        &self,
        id: &str,
        label: &str,
        protocol_slug: &str,
        protocol_name: &str,
        instruction_name: &str,
        metric_key: &str,
        metric_label: &str,
        subject_address: Option<&str>,
        subject_kind: Option<&str>,
        subject_label: Option<&str>,
        min_value: Option<f64>,
        max_value: Option<f64>,
    ) -> Result<MetricAlertRule> {
        let created_at = now_ts();
        let row = sqlx::query(
            r#"
            INSERT INTO metric_alert_rules (
                id,
                label,
                protocol_slug,
                protocol_name,
                instruction_name,
                metric_key,
                metric_label,
                subject_address,
                subject_kind,
                subject_label,
                min_value,
                max_value,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING
                id,
                label,
                protocol_slug,
                protocol_name,
                instruction_name,
                metric_key,
                metric_label,
                subject_address,
                subject_kind,
                subject_label,
                min_value,
                max_value,
                created_at;
            "#,
        )
        .bind(id)
        .bind(label)
        .bind(protocol_slug)
        .bind(protocol_name)
        .bind(instruction_name)
        .bind(metric_key)
        .bind(metric_label)
        .bind(subject_address)
        .bind(subject_kind)
        .bind(subject_label)
        .bind(min_value)
        .bind(max_value)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_metric_alert_rule(row)
    }

    pub async fn delete_metric_alert_rule(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM metric_alert_rules
            WHERE id = ?;
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn list_firewatch_users(&self) -> Result<Vec<FirewatchUser>> {
        let rows = sqlx::query(
            r#"
            SELECT id, telegram_user_id, telegram_username, label, created_at, updated_at
            FROM firewatch_users
            ORDER BY updated_at DESC, created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_firewatch_user).collect()
    }

    pub async fn firewatch_user_by_id(&self, id: &str) -> Result<Option<FirewatchUser>> {
        let row = sqlx::query(
            r#"
            SELECT id, telegram_user_id, telegram_username, label, created_at, updated_at
            FROM firewatch_users
            WHERE id = ?
            LIMIT 1;
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(row_to_firewatch_user).transpose()
    }

    pub async fn upsert_firewatch_user(
        &self,
        telegram_user_id: Option<i64>,
        telegram_username: Option<&str>,
        label: Option<&str>,
    ) -> Result<FirewatchUser> {
        let normalized_username = telegram_username
            .map(normalize_telegram_username)
            .filter(|value| !value.is_empty());
        let normalized_label = label.map(str::trim).filter(|value| !value.is_empty());
        let existing = if let Some(telegram_user_id) = telegram_user_id {
            sqlx::query(
                r#"
                SELECT id, telegram_user_id, telegram_username, label, created_at, updated_at
                FROM firewatch_users
                WHERE telegram_user_id = ?
                LIMIT 1;
                "#,
            )
            .bind(telegram_user_id)
            .fetch_optional(&self.pool)
            .await?
        } else if let Some(username) = normalized_username.as_deref() {
            sqlx::query(
                r#"
                SELECT id, telegram_user_id, telegram_username, label, created_at, updated_at
                FROM firewatch_users
                WHERE lower(telegram_username) = lower(?)
                LIMIT 1;
                "#,
            )
            .bind(username)
            .fetch_optional(&self.pool)
            .await?
        } else {
            None
        };

        let now = now_ts();
        if let Some(row) = existing {
            let id: String = row.try_get("id")?;
            let user = row_to_firewatch_user(row)?;
            let next_username = normalized_username
                .as_deref()
                .or(user.telegram_username.as_deref());
            let next_label = normalized_label.or(user.label.as_deref());
            let row = sqlx::query(
                r#"
                UPDATE firewatch_users
                SET telegram_user_id = COALESCE(?, telegram_user_id),
                    telegram_username = ?,
                    label = ?,
                    updated_at = ?
                WHERE id = ?
                RETURNING id, telegram_user_id, telegram_username, label, created_at, updated_at;
                "#,
            )
            .bind(telegram_user_id)
            .bind(next_username)
            .bind(next_label)
            .bind(now)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

            return row_to_firewatch_user(row);
        }

        let row = sqlx::query(
            r#"
            INSERT INTO firewatch_users (
                id,
                telegram_user_id,
                telegram_username,
                label,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?)
            RETURNING id, telegram_user_id, telegram_username, label, created_at, updated_at;
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(telegram_user_id)
        .bind(normalized_username.as_deref())
        .bind(normalized_label)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        row_to_firewatch_user(row)
    }

    pub async fn list_user_entitlements(&self) -> Result<Vec<UserEntitlement>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, plan_id, source, status, starts_at, expires_at, created_at, updated_at
            FROM user_entitlements
            ORDER BY updated_at DESC, created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_user_entitlement).collect()
    }

    pub async fn list_user_entitlements_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserEntitlement>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, plan_id, source, status, starts_at, expires_at, created_at, updated_at
            FROM user_entitlements
            WHERE user_id = ?
            ORDER BY starts_at DESC, created_at DESC;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_user_entitlement).collect()
    }

    pub async fn user_has_active_custom_entitlement(
        &self,
        user_id: &str,
        now: i64,
    ) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM user_entitlements
            WHERE user_id = ?
              AND status = 'active'
              AND plan_id IN ('paid', 'custom', 'pro', 'ferno-member')
              AND starts_at <= ?
              AND (expires_at IS NULL OR expires_at > ?);
            "#,
        )
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    pub async fn grant_user_entitlement(
        &self,
        user_id: &str,
        plan_id: &str,
        source: &str,
        duration_months: Option<i64>,
        expires_at: Option<i64>,
    ) -> Result<UserEntitlement> {
        let now = now_ts();
        let computed_expires_at = expires_at
            .or_else(|| duration_months.map(|months| now + months.max(1) * 30 * 24 * 60 * 60));
        let row = sqlx::query(
            r#"
            INSERT INTO user_entitlements (
                id,
                user_id,
                plan_id,
                source,
                status,
                starts_at,
                expires_at,
                created_at,
                updated_at
            )
            VALUES (?, ?, ?, ?, 'active', ?, ?, ?, ?)
            RETURNING id, user_id, plan_id, source, status, starts_at, expires_at, created_at, updated_at;
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(plan_id)
        .bind(source)
        .bind(now)
        .bind(computed_expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        self.activate_user_pending_subscriptions(user_id).await?;
        self.mark_user_payment_links_paid(user_id, now).await?;

        row_to_user_entitlement(row)
    }

    pub async fn list_payment_links(&self) -> Result<Vec<PaymentLink>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, user_id, plan_id, public_url, solana_pay_url, reference, recipient,
                amount, spl_token, label, message, memo, status, expires_at, paid_at, created_at
            FROM payment_links
            ORDER BY created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_payment_link).collect()
    }

    pub async fn list_payment_links_for_user(&self, user_id: &str) -> Result<Vec<PaymentLink>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, user_id, plan_id, public_url, solana_pay_url, reference, recipient,
                amount, spl_token, label, message, memo, status, expires_at, paid_at, created_at
            FROM payment_links
            WHERE user_id = ?
            ORDER BY created_at DESC;
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_payment_link).collect()
    }

    pub async fn create_payment_link(
        &self,
        user_id: &str,
        plan_id: &str,
        token_hash: &str,
        public_url: &str,
        solana_pay_url: &str,
        reference: &str,
        recipient: &str,
        amount: &str,
        spl_token: Option<&str>,
        label: &str,
        message: &str,
        memo: &str,
        expires_at: i64,
    ) -> Result<PaymentLink> {
        let created_at = now_ts();
        let row = sqlx::query(
            r#"
            INSERT INTO payment_links (
                id,
                user_id,
                plan_id,
                token_hash,
                public_url,
                solana_pay_url,
                reference,
                recipient,
                amount,
                spl_token,
                label,
                message,
                memo,
                status,
                expires_at,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)
            RETURNING
                id, user_id, plan_id, public_url, solana_pay_url, reference, recipient,
                amount, spl_token, label, message, memo, status, expires_at, paid_at, created_at;
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(plan_id)
        .bind(token_hash)
        .bind(public_url)
        .bind(solana_pay_url)
        .bind(reference)
        .bind(recipient)
        .bind(amount)
        .bind(spl_token)
        .bind(label)
        .bind(message)
        .bind(memo)
        .bind(expires_at)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_payment_link(row)
    }

    pub async fn payment_link_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<(PaymentLink, FirewatchUser)>> {
        let row = sqlx::query(
            r#"
            SELECT
                pl.id AS payment_link_id,
                pl.user_id AS payment_link_user_id,
                pl.plan_id AS payment_link_plan_id,
                pl.public_url AS payment_link_public_url,
                pl.solana_pay_url AS payment_link_solana_pay_url,
                pl.reference AS payment_link_reference,
                pl.recipient AS payment_link_recipient,
                pl.amount AS payment_link_amount,
                pl.spl_token AS payment_link_spl_token,
                pl.label AS payment_link_label,
                pl.message AS payment_link_message,
                pl.memo AS payment_link_memo,
                pl.status AS payment_link_status,
                pl.expires_at AS payment_link_expires_at,
                pl.paid_at AS payment_link_paid_at,
                pl.created_at AS payment_link_created_at,
                u.id AS user_id,
                u.telegram_user_id AS user_telegram_user_id,
                u.telegram_username AS user_telegram_username,
                u.label AS user_label,
                u.created_at AS user_created_at,
                u.updated_at AS user_updated_at
            FROM payment_links pl
            JOIN firewatch_users u ON u.id = pl.user_id
            WHERE pl.token_hash = ?
            LIMIT 1;
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok((
                row_to_payment_link_prefixed(&row, "payment_link_")?,
                row_to_firewatch_user_prefixed(&row, "user_")?,
            ))
        })
        .transpose()
    }

    pub async fn create_telegram_config_session(
        &self,
        user_id: &str,
        chat_id: &str,
        chat_title: Option<&str>,
        chat_type: Option<&str>,
        token_hash: &str,
        expires_at: i64,
    ) -> Result<TelegramConfigSession> {
        let created_at = now_ts();
        let row = sqlx::query(
            r#"
            INSERT INTO telegram_config_sessions (
                id,
                user_id,
                chat_id,
                chat_title,
                chat_type,
                token_hash,
                expires_at,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id, user_id, chat_id, chat_title, chat_type, expires_at, used_at, created_at;
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(chat_id)
        .bind(chat_title)
        .bind(chat_type)
        .bind(token_hash)
        .bind(expires_at)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_telegram_config_session(row)
    }

    pub async fn telegram_config_session_by_token_hash(
        &self,
        token_hash: &str,
        now: i64,
    ) -> Result<Option<(TelegramConfigSession, FirewatchUser)>> {
        let row = sqlx::query(
            r#"
            SELECT
                s.id AS session_id,
                s.user_id AS session_user_id,
                s.chat_id AS session_chat_id,
                s.chat_title AS session_chat_title,
                s.chat_type AS session_chat_type,
                s.expires_at AS session_expires_at,
                s.used_at AS session_used_at,
                s.created_at AS session_created_at,
                u.id AS user_id,
                u.telegram_user_id AS user_telegram_user_id,
                u.telegram_username AS user_telegram_username,
                u.label AS user_label,
                u.created_at AS user_created_at,
                u.updated_at AS user_updated_at
            FROM telegram_config_sessions s
            JOIN firewatch_users u ON u.id = s.user_id
            WHERE s.token_hash = ?
              AND s.expires_at > ?
            LIMIT 1;
            "#,
        )
        .bind(token_hash)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok((
                row_to_telegram_config_session_prefixed(&row, "session_")?,
                row_to_firewatch_user_prefixed(&row, "user_")?,
            ))
        })
        .transpose()
    }

    pub async fn pending_payment_links_by_references(
        &self,
        references: &[String],
        now: i64,
    ) -> Result<Vec<PaymentLink>> {
        if references.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = QueryBuilder::<Sqlite>::new(
            r#"
            SELECT
                id, user_id, plan_id, public_url, solana_pay_url, reference, recipient,
                amount, spl_token, label, message, memo, status, expires_at, paid_at, created_at
            FROM payment_links
            WHERE status = 'pending'
              AND expires_at > 
            "#,
        );
        builder.push_bind(now);
        builder.push(" AND reference IN (");
        let mut separated = builder.separated(", ");
        for reference in references {
            separated.push_bind(reference);
        }
        separated.push_unseparated(")");

        let rows = builder.build().fetch_all(&self.pool).await?;

        rows.into_iter().map(row_to_payment_link).collect()
    }

    pub async fn mark_payment_link_paid(&self, id: &str, paid_at: i64) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE payment_links
            SET status = 'paid', paid_at = ?
            WHERE id = ? AND status = 'pending';
            "#,
        )
        .bind(paid_at)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn activate_user_pending_subscriptions(&self, user_id: &str) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE telegram_subscriptions
            SET status = 'active'
            WHERE user_id = ? AND status = 'pending_payment';
            "#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn mark_user_payment_links_paid(&self, user_id: &str, paid_at: i64) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE payment_links
            SET status = 'paid', paid_at = COALESCE(paid_at, ?)
            WHERE user_id = ? AND status = 'pending';
            "#,
        )
        .bind(paid_at)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn list_promo_codes(&self) -> Result<Vec<PromoCode>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                code,
                label,
                audience,
                plan_id,
                duration_months,
                max_redemptions,
                redeemed_count,
                starts_at,
                expires_at,
                active,
                created_at
            FROM promo_codes
            ORDER BY created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_promo_code).collect()
    }

    pub async fn create_promo_code(
        &self,
        id: &str,
        code: &str,
        label: &str,
        audience: &str,
        plan_id: &str,
        duration_months: i64,
        max_redemptions: Option<i64>,
        starts_at: Option<i64>,
        expires_at: Option<i64>,
    ) -> Result<PromoCode> {
        let created_at = now_ts();
        let row = sqlx::query(
            r#"
            INSERT INTO promo_codes (
                id,
                code,
                label,
                audience,
                plan_id,
                duration_months,
                max_redemptions,
                starts_at,
                expires_at,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING
                id,
                code,
                label,
                audience,
                plan_id,
                duration_months,
                max_redemptions,
                redeemed_count,
                starts_at,
                expires_at,
                active,
                created_at;
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(label)
        .bind(audience)
        .bind(plan_id)
        .bind(duration_months)
        .bind(max_redemptions)
        .bind(starts_at)
        .bind(expires_at)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_promo_code(row)
    }

    pub async fn delete_promo_code(&self, id: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM promo_codes
            WHERE id = ?;
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn matching_metric_alert_rules(
        &self,
        observation: &ProtocolMetricObservation,
    ) -> Result<Vec<MetricAlertRule>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                label,
                protocol_slug,
                protocol_name,
                instruction_name,
                metric_key,
                metric_label,
                subject_address,
                subject_kind,
                subject_label,
                min_value,
                max_value,
                created_at
            FROM metric_alert_rules
            WHERE protocol_slug = ?
              AND instruction_name = ?
              AND metric_key = ?
              AND ((subject_address IS NULL AND ? IS NULL) OR subject_address = ?)
            ORDER BY created_at DESC;
            "#,
        )
        .bind(&observation.protocol_slug)
        .bind(&observation.instruction_name)
        .bind(&observation.metric_key)
        .bind(&observation.subject_address)
        .bind(&observation.subject_address)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_metric_alert_rule).collect()
    }

    pub async fn observe_program_activity(
        &self,
        program_id: &str,
        protocol_slug: Option<&str>,
        protocol_name: Option<&str>,
        program_label: Option<&str>,
        signature: &str,
        instruction_path: &str,
        slot: i64,
        block_time: Option<i64>,
    ) -> Result<Option<ProgramActivityState>> {
        let previous = sqlx::query(
            r#"
            SELECT
                program_id,
                protocol_slug,
                protocol_name,
                program_label,
                last_signature,
                last_instruction_path,
                last_slot,
                last_block_time,
                updated_at
            FROM program_activity_state
            WHERE program_id = ?;
            "#,
        )
        .bind(program_id)
        .fetch_optional(&self.pool)
        .await?
        .map(row_to_program_activity_state)
        .transpose()?;

        sqlx::query(
            r#"
            INSERT INTO program_activity_state (
                program_id,
                protocol_slug,
                protocol_name,
                program_label,
                last_signature,
                last_instruction_path,
                last_slot,
                last_block_time,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(program_id) DO UPDATE SET
                protocol_slug = excluded.protocol_slug,
                protocol_name = excluded.protocol_name,
                program_label = excluded.program_label,
                last_signature = excluded.last_signature,
                last_instruction_path = excluded.last_instruction_path,
                last_slot = excluded.last_slot,
                last_block_time = excluded.last_block_time,
                updated_at = excluded.updated_at
            WHERE excluded.last_slot >= program_activity_state.last_slot;
            "#,
        )
        .bind(program_id)
        .bind(protocol_slug)
        .bind(protocol_name)
        .bind(program_label)
        .bind(signature)
        .bind(instruction_path)
        .bind(slot)
        .bind(block_time)
        .bind(now_ts())
        .execute(&self.pool)
        .await?;

        Ok(previous)
    }

    pub async fn upsert_liquidity_pool(&self, pool: &LiquidityPoolRecord) -> Result<bool> {
        let existed = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM liquidity_pools
            WHERE pool_address = ?;
            "#,
        )
        .bind(&pool.pool_address)
        .fetch_one(&self.pool)
        .await?
            > 0;

        sqlx::query(
            r#"
            INSERT INTO liquidity_pools (
                pool_address,
                protocol_slug,
                protocol_name,
                dex_id,
                program_id,
                program_label,
                token_x_mint,
                token_y_mint,
                reserve_x,
                reserve_y,
                source,
                first_seen_slot,
                first_seen_signature,
                last_seen_slot,
                last_seen_signature,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pool_address) DO UPDATE SET
                protocol_slug = excluded.protocol_slug,
                protocol_name = excluded.protocol_name,
                dex_id = excluded.dex_id,
                program_id = excluded.program_id,
                program_label = excluded.program_label,
                token_x_mint = excluded.token_x_mint,
                token_y_mint = excluded.token_y_mint,
                reserve_x = COALESCE(excluded.reserve_x, liquidity_pools.reserve_x),
                reserve_y = COALESCE(excluded.reserve_y, liquidity_pools.reserve_y),
                source = excluded.source,
                first_seen_slot = CASE
                    WHEN liquidity_pools.first_seen_slot IS NULL THEN excluded.first_seen_slot
                    WHEN excluded.first_seen_slot IS NULL THEN liquidity_pools.first_seen_slot
                    ELSE min(liquidity_pools.first_seen_slot, excluded.first_seen_slot)
                END,
                first_seen_signature = CASE
                    WHEN liquidity_pools.first_seen_slot IS NULL THEN excluded.first_seen_signature
                    WHEN excluded.first_seen_slot IS NULL THEN liquidity_pools.first_seen_signature
                    WHEN excluded.first_seen_slot < liquidity_pools.first_seen_slot THEN excluded.first_seen_signature
                    ELSE liquidity_pools.first_seen_signature
                END,
                last_seen_slot = CASE
                    WHEN liquidity_pools.last_seen_slot IS NULL THEN excluded.last_seen_slot
                    WHEN excluded.last_seen_slot IS NULL THEN liquidity_pools.last_seen_slot
                    ELSE max(liquidity_pools.last_seen_slot, excluded.last_seen_slot)
                END,
                last_seen_signature = CASE
                    WHEN liquidity_pools.last_seen_slot IS NULL THEN excluded.last_seen_signature
                    WHEN excluded.last_seen_slot IS NULL THEN liquidity_pools.last_seen_signature
                    WHEN excluded.last_seen_slot >= liquidity_pools.last_seen_slot THEN excluded.last_seen_signature
                    ELSE liquidity_pools.last_seen_signature
                END,
                updated_at = excluded.updated_at;
            "#,
        )
        .bind(&pool.pool_address)
        .bind(&pool.protocol_slug)
        .bind(&pool.protocol_name)
        .bind(&pool.dex_id)
        .bind(&pool.program_id)
        .bind(&pool.program_label)
        .bind(&pool.token_x_mint)
        .bind(&pool.token_y_mint)
        .bind(&pool.reserve_x)
        .bind(&pool.reserve_y)
        .bind(&pool.source)
        .bind(pool.first_seen_slot)
        .bind(&pool.first_seen_signature)
        .bind(pool.last_seen_slot)
        .bind(&pool.last_seen_signature)
        .bind(pool.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(!existed)
    }

    pub async fn upsert_liquidity_pool_latest_state(
        &self,
        state: &LiquidityPoolLatestStateRecord,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO liquidity_pool_latest_state (
                pool_address,
                dex_id,
                base_token_mint,
                quote_token_mint,
                base_reserve,
                quote_reserve,
                base_vault,
                quote_vault,
                estimated_liquidity_usd,
                latest_signature,
                latest_slot,
                latest_block_time,
                updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pool_address) DO UPDATE SET
                dex_id = excluded.dex_id,
                base_token_mint = excluded.base_token_mint,
                quote_token_mint = excluded.quote_token_mint,
                base_reserve = excluded.base_reserve,
                quote_reserve = excluded.quote_reserve,
                base_vault = excluded.base_vault,
                quote_vault = excluded.quote_vault,
                estimated_liquidity_usd = excluded.estimated_liquidity_usd,
                latest_signature = excluded.latest_signature,
                latest_slot = excluded.latest_slot,
                latest_block_time = excluded.latest_block_time,
                updated_at = excluded.updated_at
            WHERE excluded.latest_slot >= liquidity_pool_latest_state.latest_slot;
            "#,
        )
        .bind(&state.pool_address)
        .bind(&state.dex_id)
        .bind(&state.base_token_mint)
        .bind(&state.quote_token_mint)
        .bind(state.base_reserve)
        .bind(state.quote_reserve)
        .bind(&state.base_vault)
        .bind(&state.quote_vault)
        .bind(state.estimated_liquidity_usd)
        .bind(&state.latest_signature)
        .bind(state.latest_slot)
        .bind(state.latest_block_time)
        .bind(state.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn upsert_liquidity_pool_snapshot(
        &self,
        snapshot: &LiquidityPoolSnapshotRecord,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO liquidity_pool_snapshots (
                id,
                pool_address,
                dex_id,
                base_token_mint,
                quote_token_mint,
                signature,
                slot,
                block_time,
                base_reserve,
                quote_reserve,
                base_vault,
                quote_vault,
                estimated_liquidity_usd,
                retention_class,
                observed_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                dex_id = excluded.dex_id,
                base_token_mint = excluded.base_token_mint,
                quote_token_mint = excluded.quote_token_mint,
                slot = excluded.slot,
                block_time = excluded.block_time,
                base_reserve = excluded.base_reserve,
                quote_reserve = excluded.quote_reserve,
                base_vault = excluded.base_vault,
                quote_vault = excluded.quote_vault,
                estimated_liquidity_usd = excluded.estimated_liquidity_usd,
                retention_class = CASE
                    WHEN liquidity_pool_snapshots.retention_class = 'watched' THEN liquidity_pool_snapshots.retention_class
                    WHEN excluded.retention_class = 'watched' THEN excluded.retention_class
                    ELSE excluded.retention_class
                END,
                observed_at = max(liquidity_pool_snapshots.observed_at, excluded.observed_at);
            "#,
        )
        .bind(&snapshot.id)
        .bind(&snapshot.pool_address)
        .bind(&snapshot.dex_id)
        .bind(&snapshot.base_token_mint)
        .bind(&snapshot.quote_token_mint)
        .bind(&snapshot.signature)
        .bind(snapshot.slot)
        .bind(snapshot.block_time)
        .bind(snapshot.base_reserve)
        .bind(snapshot.quote_reserve)
        .bind(&snapshot.base_vault)
        .bind(&snapshot.quote_vault)
        .bind(snapshot.estimated_liquidity_usd)
        .bind(&snapshot.retention_class)
        .bind(snapshot.observed_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn prune_liquidity_pool_snapshots(
        &self,
        default_retention_days: i64,
        watched_retention_days: i64,
    ) -> Result<u64> {
        let now = now_ts();
        let default_cutoff = now - default_retention_days.max(1) * 24 * 60 * 60;
        let watched_cutoff = now - watched_retention_days.max(1) * 24 * 60 * 60;
        let result = sqlx::query(
            r#"
            DELETE FROM liquidity_pool_snapshots
            WHERE (
                retention_class = ?
                AND COALESCE(block_time, observed_at) < ?
            )
            OR (
                retention_class != ?
                AND COALESCE(block_time, observed_at) < ?
            );
            "#,
        )
        .bind(LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED)
        .bind(watched_cutoff)
        .bind(LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED)
        .bind(default_cutoff)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn list_telegram_subscriptions(&self) -> Result<Vec<TelegramSubscription>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags_json,
                status,
                payment_link_id,
                created_at
            FROM telegram_subscriptions
            ORDER BY created_at DESC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_telegram_subscription).collect()
    }

    pub async fn list_telegram_subscriptions_for_config_session(
        &self,
        user_id: &str,
        chat_id: &str,
    ) -> Result<Vec<TelegramSubscription>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags_json,
                status,
                payment_link_id,
                created_at
            FROM telegram_subscriptions
            WHERE chat_id = ?
              AND (user_id = ? OR user_id IS NULL)
              AND status IN ('active', 'pending_payment')
            ORDER BY created_at DESC;
            "#,
        )
        .bind(chat_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_telegram_subscription).collect()
    }

    pub async fn telegram_subscription_count(&self) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM telegram_subscriptions;
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(count)
    }

    pub async fn create_telegram_subscription(
        &self,
        id: &str,
        label: &str,
        user_id: Option<&str>,
        chat_id: &str,
        thread_id: Option<i64>,
        scope: TelegramEventScope,
        target_address: Option<&str>,
        required_tags: &[String],
        status: &str,
        payment_link_id: Option<&str>,
    ) -> Result<TelegramSubscription> {
        let created_at = now_ts();
        let row = sqlx::query(
            r#"
            INSERT INTO telegram_subscriptions (
                id,
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags_json,
                status,
                payment_link_id,
                created_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING
                id,
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags_json,
                status,
                payment_link_id,
                created_at;
            "#,
        )
        .bind(id)
        .bind(label)
        .bind(user_id)
        .bind(chat_id)
        .bind(thread_id)
        .bind(scope.as_str())
        .bind(target_address)
        .bind(serde_json::to_string(required_tags)?)
        .bind(status)
        .bind(payment_link_id)
        .bind(created_at)
        .fetch_one(&self.pool)
        .await?;

        row_to_telegram_subscription(row)
    }

    pub async fn ensure_telegram_subscription(
        &self,
        label: &str,
        user_id: Option<&str>,
        chat_id: &str,
        thread_id: Option<i64>,
        scope: TelegramEventScope,
        target_address: Option<&str>,
        required_tags: &[String],
    ) -> Result<(TelegramSubscription, bool)> {
        let required_tags_json = serde_json::to_string(required_tags)?;
        let existing = sqlx::query(
            r#"
            SELECT
                id,
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags_json,
                status,
                payment_link_id,
                created_at
            FROM telegram_subscriptions
            WHERE chat_id = ?
              AND ((thread_id IS NULL AND ? IS NULL) OR thread_id = ?)
              AND scope = ?
              AND ((target_address IS NULL AND ? IS NULL) OR target_address = ?)
              AND required_tags_json = ?
              AND status = 'active'
            LIMIT 1;
            "#,
        )
        .bind(chat_id)
        .bind(thread_id)
        .bind(thread_id)
        .bind(scope.as_str())
        .bind(target_address)
        .bind(target_address)
        .bind(required_tags_json)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            return Ok((row_to_telegram_subscription(row)?, false));
        }

        Ok((
            self.create_telegram_subscription(
                &uuid::Uuid::new_v4().to_string(),
                label,
                user_id,
                chat_id,
                thread_id,
                scope,
                target_address,
                required_tags,
                "active",
                None,
            )
            .await?,
            true,
        ))
    }

    pub async fn delete_telegram_subscription(&self, id: &str) -> Result<bool> {
        sqlx::query(
            r#"
            DELETE FROM telegram_deliveries
            WHERE subscription_id = ?;
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        let result = sqlx::query(
            r#"
            DELETE FROM telegram_subscriptions
            WHERE id = ?;
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    pub async fn matching_telegram_subscriptions(
        &self,
        event: &ChainEvent,
    ) -> Result<Vec<TelegramSubscription>> {
        let subscriptions = self.list_telegram_subscriptions().await?;
        Ok(subscriptions
            .into_iter()
            .filter(|subscription| subscription.matches_event(event))
            .collect())
    }

    pub async fn enqueue_telegram_delivery(
        &self,
        event: &ChainEvent,
        subscription: &TelegramSubscription,
        initial_next_retry_at: i64,
    ) -> Result<Option<PendingTelegramDelivery>> {
        let delivery_id = format!("{}:{}", event.id, subscription.id);
        let created_at = now_ts();
        let result = sqlx::query(
            r#"
            INSERT INTO telegram_deliveries (
                id,
                event_id,
                subscription_id,
                created_at,
                delivery_attempts,
                delivered_at,
                last_error,
                next_retry_at
            )
            VALUES (?, ?, ?, ?, 0, NULL, NULL, ?)
            ON CONFLICT(id) DO NOTHING;
            "#,
        )
        .bind(&delivery_id)
        .bind(&event.id)
        .bind(&subscription.id)
        .bind(created_at)
        .bind(initial_next_retry_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Ok(Some(PendingTelegramDelivery {
            id: delivery_id,
            event: event.clone(),
            subscription: subscription.clone(),
            delivery_attempts: 0,
        }))
    }

    pub async fn pending_telegram_deliveries(
        &self,
        now: i64,
        limit: i64,
    ) -> Result<Vec<PendingTelegramDelivery>> {
        let rows = sqlx::query(
            r#"
            SELECT
                td.id AS telegram_delivery_id,
                td.delivery_attempts AS telegram_delivery_attempts,
                s.id AS subscription_id,
                s.label AS subscription_label,
                s.user_id AS subscription_user_id,
                s.chat_id AS subscription_chat_id,
                s.thread_id AS subscription_thread_id,
                s.scope AS subscription_scope,
                s.target_address AS subscription_target_address,
                s.required_tags_json AS subscription_required_tags_json,
                s.status AS subscription_status,
                s.payment_link_id AS subscription_payment_link_id,
                s.created_at AS subscription_created_at,
                e.id,
                e.signature,
                e.instruction_path,
                e.kind,
                e.target_type,
                e.target_address,
                e.actor,
                e.slot,
                e.block_time,
                e.indexed_at,
                e.summary,
                e.tags_json,
                e.payload_json,
                e.delivery_attempts,
                e.delivered_at,
                e.last_error,
                e.next_retry_at
            FROM telegram_deliveries td
            JOIN telegram_subscriptions s ON s.id = td.subscription_id
            JOIN events e ON e.id = td.event_id
            WHERE td.delivered_at IS NULL AND td.next_retry_at <= ? AND s.status = 'active'
            ORDER BY td.next_retry_at ASC, e.slot DESC
            LIMIT ?;
            "#,
        )
        .bind(now)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(row_to_pending_telegram_delivery)
            .collect()
    }

    pub async fn mark_telegram_delivery_success(
        &self,
        delivery_id: &str,
        delivered_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE telegram_deliveries
            SET delivered_at = ?, last_error = NULL, next_retry_at = ?
            WHERE id = ?;
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .bind(delivery_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_raydium_amm_v4_swap_false_positive_deliveries(
        &self,
        delivered_at: i64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE telegram_deliveries
            SET delivered_at = ?,
                last_error = NULL,
                next_retry_at = ?
            WHERE delivered_at IS NULL
              AND event_id IN (
                SELECT id
                FROM events
                WHERE kind = 'program_unknown_instruction'
                  AND target_address = '675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8'
                  AND json_extract(payload_json, '$.protocol') = 'raydium'
                  AND json_extract(payload_json, '$.program_label') = 'Legacy AMM v4'
                  AND (
                    json_extract(payload_json, '$.discriminator') LIKE '09%'
                    OR json_extract(payload_json, '$.discriminator') LIKE '0b%'
                    OR json_extract(payload_json, '$.discriminator') LIKE '10%'
                    OR json_extract(payload_json, '$.discriminator') LIKE '11%'
                  )
              );
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn mark_raydium_clmm_limit_order_false_positive_deliveries(
        &self,
        delivered_at: i64,
    ) -> Result<u64> {
        let event_result = sqlx::query(
            r#"
            UPDATE events
            SET delivered_at = ?,
                last_error = NULL,
                next_retry_at = ?
            WHERE delivered_at IS NULL
              AND kind = 'program_unknown_instruction'
              AND target_address = 'CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK'
              AND json_extract(payload_json, '$.protocol') = 'raydium'
              AND json_extract(payload_json, '$.program_label') = 'CLMM'
              AND json_extract(payload_json, '$.discriminator') IN (
                '9d20dab7471d1293',
                '4c7c800fd55725fa',
                'cd4e74215c691a60'
              );
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;

        let delivery_result = sqlx::query(
            r#"
            UPDATE telegram_deliveries
            SET delivered_at = ?,
                last_error = NULL,
                next_retry_at = ?
            WHERE delivered_at IS NULL
              AND event_id IN (
                SELECT id
                FROM events
                WHERE kind = 'program_unknown_instruction'
                  AND target_address = 'CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK'
                  AND json_extract(payload_json, '$.protocol') = 'raydium'
                  AND json_extract(payload_json, '$.program_label') = 'CLMM'
                  AND json_extract(payload_json, '$.discriminator') IN (
                    '9d20dab7471d1293',
                    '4c7c800fd55725fa',
                    'cd4e74215c691a60'
                  )
              );
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;

        Ok(event_result.rows_affected() + delivery_result.rows_affected())
    }

    pub async fn mark_kamino_reserve_config_parser_false_positive_deliveries(
        &self,
        delivered_at: i64,
    ) -> Result<u64> {
        let event_result = sqlx::query(
            r#"
            UPDATE events
            SET delivered_at = ?,
                last_error = NULL,
                next_retry_at = ?
            WHERE delivered_at IS NULL
              AND kind = 'program_admin_instruction'
              AND target_address = 'KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD'
              AND json_extract(payload_json, '$.protocol') = 'kamino_lend'
              AND json_extract(payload_json, '$.instruction_name') = 'update_reserve_config'
              AND json_extract(payload_json, '$.kamino_update_config_mode') = 'unknown'
              AND json_extract(payload_json, '$.kamino_update_config_value_len') = '0';
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;

        let delivery_result = sqlx::query(
            r#"
            UPDATE telegram_deliveries
            SET delivered_at = ?,
                last_error = NULL,
                next_retry_at = ?
            WHERE delivered_at IS NULL
              AND event_id IN (
                SELECT id
                FROM events
                WHERE kind = 'program_admin_instruction'
                  AND target_address = 'KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD'
                  AND json_extract(payload_json, '$.protocol') = 'kamino_lend'
                  AND json_extract(payload_json, '$.instruction_name') = 'update_reserve_config'
                  AND json_extract(payload_json, '$.kamino_update_config_mode') = 'unknown'
                  AND json_extract(payload_json, '$.kamino_update_config_value_len') = '0'
              );
            "#,
        )
        .bind(delivered_at)
        .bind(delivered_at)
        .execute(&self.pool)
        .await?;

        Ok(event_result.rows_affected() + delivery_result.rows_affected())
    }

    pub async fn mark_telegram_delivery_failure(
        &self,
        delivery_id: &str,
        delivery_attempts: i64,
        last_error: &str,
        next_retry_at: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE telegram_deliveries
            SET delivery_attempts = ?, last_error = ?, next_retry_at = ?
            WHERE id = ?;
            "#,
        )
        .bind(delivery_attempts)
        .bind(last_error)
        .bind(next_retry_at)
        .bind(delivery_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn last_processed_slot(&self) -> Result<Option<u64>> {
        let row = sqlx::query(
            r#"
            SELECT value
            FROM app_state
            WHERE key = 'last_processed_slot';
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .and_then(|row| row.try_get::<String, _>("value").ok())
            .and_then(|value| value.parse::<u64>().ok()))
    }

    pub async fn set_last_processed_slot(&self, slot: u64) -> Result<()> {
        self.set_app_state_value("last_processed_slot", &slot.to_string())
            .await
    }

    pub async fn clear_last_processed_slot(&self) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM app_state
            WHERE key = 'last_processed_slot';
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn telegram_last_update_id(&self) -> Result<Option<i64>> {
        Ok(self
            .app_state_value("telegram_last_update_id")
            .await?
            .and_then(|value| value.parse::<i64>().ok()))
    }

    pub async fn set_telegram_last_update_id(&self, update_id: i64) -> Result<()> {
        self.set_app_state_value("telegram_last_update_id", &update_id.to_string())
            .await
    }

    pub async fn increment_processed_transaction_count(&self) -> Result<i64> {
        self.increment_app_state_counter("processed_transaction_count", 1)
            .await
    }

    pub async fn increment_matched_transaction_count(&self) -> Result<i64> {
        self.increment_app_state_counter("matched_transaction_count", 1)
            .await
    }

    pub async fn record_backfilled_slot(&self, slot: u64) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO app_state (key, value)
            VALUES ('backfill_start_slot', ?)
            ON CONFLICT(key) DO UPDATE SET value =
                CASE
                    WHEN CAST(excluded.value AS INTEGER) < CAST(app_state.value AS INTEGER)
                    THEN excluded.value
                    ELSE app_state.value
                END;
            "#,
        )
        .bind(slot.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn missing_block_time_slots(&self, limit: i64) -> Result<Vec<u64>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT slot
            FROM (
                SELECT slot FROM events WHERE block_time IS NULL
                UNION
                SELECT slot FROM program_versions WHERE block_time IS NULL
                UNION
                SELECT slot FROM program_idl_versions WHERE block_time IS NULL
                UNION
                SELECT slot FROM program_data_writes WHERE block_time IS NULL
                UNION
                SELECT slot FROM program_idl_writes WHERE block_time IS NULL
                UNION
                SELECT slot FROM protocol_metric_observations WHERE block_time IS NULL
            )
            ORDER BY slot DESC
            LIMIT ?;
            "#,
        )
        .bind(limit.max(0))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|row| row.try_get::<i64, _>("slot").ok())
            .filter_map(|slot| u64::try_from(slot).ok())
            .collect())
    }

    pub async fn update_block_time_for_slot(
        &self,
        slot: u64,
        block_time: i64,
    ) -> Result<BlockTimeUpdateReport> {
        let slot = i64::try_from(slot).context("slot exceeds SQLite integer range")?;
        let events = update_missing_block_time(&self.pool, "events", slot, block_time).await?;
        let program_versions =
            update_missing_block_time(&self.pool, "program_versions", slot, block_time).await?;
        let program_idl_versions =
            update_missing_block_time(&self.pool, "program_idl_versions", slot, block_time).await?;
        let program_data_writes =
            update_missing_block_time(&self.pool, "program_data_writes", slot, block_time).await?;
        let program_idl_writes =
            update_missing_block_time(&self.pool, "program_idl_writes", slot, block_time).await?;
        let protocol_metric_observations =
            update_missing_block_time(&self.pool, "protocol_metric_observations", slot, block_time)
                .await?;

        Ok(BlockTimeUpdateReport {
            events,
            program_versions,
            program_idl_versions,
            program_data_writes,
            program_idl_writes,
            protocol_metric_observations,
        })
    }

    pub async fn summary(&self) -> Result<EventsSummary> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) AS total_count,
                MIN(slot) AS first_indexed_slot,
                COALESCE(
                    (SELECT COUNT(*) FROM telegram_deliveries WHERE delivered_at IS NOT NULL),
                    0
                ) AS delivered_count,
                COALESCE(
                    (SELECT COUNT(*) FROM telegram_deliveries WHERE delivered_at IS NULL),
                    0
                ) AS pending_delivery_count
            FROM events;
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(EventsSummary {
            total_count: row.try_get::<i64, _>("total_count")?,
            delivered_count: row.try_get::<i64, _>("delivered_count")?,
            pending_delivery_count: row.try_get::<i64, _>("pending_delivery_count")?,
            first_indexed_slot: row
                .try_get::<Option<i64>, _>("first_indexed_slot")?
                .and_then(|slot| u64::try_from(slot).ok()),
            backfill_start_slot: self
                .app_state_value("backfill_start_slot")
                .await?
                .and_then(|value| value.parse::<u64>().ok()),
            last_processed_slot: self.last_processed_slot().await?,
            processed_transaction_count: self
                .app_state_value("processed_transaction_count")
                .await?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
            matched_transaction_count: self
                .app_state_value("matched_transaction_count")
                .await?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0),
        })
    }

    async fn app_state_value(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            r#"
            SELECT value
            FROM app_state
            WHERE key = ?;
            "#,
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|row| row.try_get::<String, _>("value").ok()))
    }

    async fn set_app_state_value(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO app_state (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value;
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn increment_app_state_counter(&self, key: &str, delta: i64) -> Result<i64> {
        let row = sqlx::query(
            r#"
            INSERT INTO app_state (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE
            SET value = CAST(app_state.value AS INTEGER) + CAST(excluded.value AS INTEGER)
            RETURNING value;
            "#,
        )
        .bind(key)
        .bind(delta.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row
            .try_get::<String, _>("value")?
            .parse::<i64>()
            .unwrap_or(delta))
    }

    pub async fn migrate_inline_blobs(&self) -> Result<BlobMigrationReport> {
        if !self.blob_store.enabled() {
            bail!("BLOB_STORE_DIR must be set before migrating inline SQLite blobs");
        }

        let mut report = BlobMigrationReport::default();
        self.migrate_program_version_blobs(&mut report).await?;
        self.compact_complete_program_data_write_bytes(&mut report)
            .await?;
        self.migrate_program_idl_version_blobs(&mut report).await?;
        self.migrate_disassembly_blobs(&mut report).await?;
        Ok(report)
    }

    pub async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM;").execute(&self.pool).await?;
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE);")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn storage_report(&self) -> Result<DatabaseStorageReport> {
        let database_path = sqlx::query(
            r#"
            PRAGMA database_list;
            "#,
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .find_map(|row| {
            let name = row.try_get::<String, _>("name").ok()?;
            (name == "main")
                .then(|| row.try_get::<String, _>("file").ok())
                .flatten()
        })
        .filter(|path| !path.is_empty());

        let page_size = pragma_i64(&self.pool, "PRAGMA page_size;").await?;
        let page_count = pragma_i64(&self.pool, "PRAGMA page_count;").await?;
        let freelist_count = pragma_i64(&self.pool, "PRAGMA freelist_count;").await?;
        let total_bytes = page_size.saturating_mul(page_count);
        let free_bytes = page_size.saturating_mul(freelist_count);
        let used_bytes = total_bytes.saturating_sub(free_bytes);
        let tables = self.table_storage_stats().await?;

        Ok(DatabaseStorageReport {
            database_path,
            page_size,
            page_count,
            freelist_count,
            total_bytes,
            free_bytes,
            used_bytes,
            tables,
        })
    }

    async fn table_storage_stats(&self) -> Result<Vec<DatabaseTableStorageStat>> {
        let rows = match sqlx::query(
            r#"
            SELECT name, SUM(pgsize) AS bytes, COUNT(*) AS pages
            FROM dbstat
            WHERE name NOT LIKE 'sqlite_%'
            GROUP BY name
            ORDER BY bytes DESC, name ASC;
            "#,
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(error = %error, "SQLite dbstat storage report unavailable");
                return Ok(Vec::new());
            }
        };

        rows.into_iter()
            .map(|row| {
                Ok(DatabaseTableStorageStat {
                    name: row.try_get("name")?,
                    bytes: row.try_get("bytes")?,
                    pages: row.try_get("pages")?,
                })
            })
            .collect()
    }

    async fn migrate_program_version_blobs(&self, report: &mut BlobMigrationReport) -> Result<()> {
        loop {
            let rows = sqlx::query(
                r#"
                SELECT id, program_address, slot, program_sha256, program_bytes
                FROM program_versions
                WHERE program_bytes_blob_key IS NULL
                  AND program_sha256 IS NOT NULL
                  AND program_bytes IS NOT NULL
                  AND length(program_bytes) > 0
                LIMIT 100;
                "#,
            )
            .fetch_all(&self.pool)
            .await?;
            if rows.is_empty() {
                return Ok(());
            }

            for row in rows {
                let id = row.try_get::<String, _>("id")?;
                let program_address = row.try_get::<String, _>("program_address")?;
                let slot = row.try_get::<i64, _>("slot")?;
                let program_sha256 = row.try_get::<String, _>("program_sha256")?;
                let bytes = row.try_get::<Vec<u8>, _>("program_bytes")?;
                let bytes_len = bytes.len() as u64;
                let blob_key = self
                    .put_program_version_blob(&program_address, slot, &id, &program_sha256, &bytes)
                    .await?
                    .context("program version blob store disabled")?;

                let result = sqlx::query(
                    r#"
                    UPDATE program_versions
                    SET program_bytes = NULL, program_bytes_blob_key = ?
                    WHERE id = ?
                      AND program_bytes_blob_key IS NULL;
                    "#,
                )
                .bind(&blob_key)
                .bind(&id)
                .execute(&self.pool)
                .await?;
                if result.rows_affected() > 0 {
                    report.program_versions += 1;
                    report.bytes_moved += bytes_len;
                }
            }
        }
    }

    async fn compact_complete_program_data_write_bytes(
        &self,
        report: &mut BlobMigrationReport,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"
            SELECT source_account, slot, instruction_order
            FROM program_versions
            WHERE complete = 1
              AND (program_bytes IS NOT NULL OR program_bytes_blob_key IS NOT NULL)
            ORDER BY slot ASC, instruction_order ASC, id ASC;
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let source_account = row.try_get::<String, _>("source_account")?;
            let slot = row.try_get::<i64, _>("slot")?;
            let instruction_order = row.try_get::<String, _>("instruction_order")?;
            let lower_bound = self
                .previous_program_version_order_for_source(
                    &source_account,
                    slot,
                    &instruction_order,
                )
                .await?;
            let bytes_len = self
                .program_data_write_bytes_len_for_source(
                    &source_account,
                    lower_bound.as_ref(),
                    slot,
                    &instruction_order,
                )
                .await?;
            if bytes_len == 0 {
                continue;
            }

            self.prune_program_data_write_bytes_for_source(
                &source_account,
                lower_bound.as_ref(),
                slot,
                &instruction_order,
            )
            .await?;
            report.program_data_writes += 1;
            report.bytes_moved += bytes_len;
        }

        Ok(())
    }

    async fn migrate_program_idl_version_blobs(
        &self,
        report: &mut BlobMigrationReport,
    ) -> Result<()> {
        loop {
            let rows = sqlx::query(
                r#"
                SELECT
                    id,
                    program_address,
                    idl_sha256,
                    idl_bytes,
                    idl_bytes_blob_key,
                    idl_text,
                    idl_text_blob_key
                FROM program_idl_versions
                WHERE (
                        idl_bytes_blob_key IS NULL
                    AND idl_bytes IS NOT NULL
                    AND length(idl_bytes) > 0
                )
                   OR (
                        idl_text_blob_key IS NULL
                    AND idl_text IS NOT NULL
                    AND length(idl_text) > 0
                )
                LIMIT 100;
                "#,
            )
            .fetch_all(&self.pool)
            .await?;
            if rows.is_empty() {
                return Ok(());
            }

            for row in rows {
                let id = row.try_get::<String, _>("id")?;
                let program_address = row.try_get::<String, _>("program_address")?;
                let idl_sha256 = row.try_get::<Option<String>, _>("idl_sha256")?;
                let idl_bytes = row.try_get::<Option<Vec<u8>>, _>("idl_bytes")?;
                let idl_bytes_blob_key = row.try_get::<Option<String>, _>("idl_bytes_blob_key")?;
                let idl_text = row.try_get::<Option<String>, _>("idl_text")?;
                let idl_text_blob_key = row.try_get::<Option<String>, _>("idl_text_blob_key")?;

                if idl_bytes_blob_key.is_none()
                    && let Some(bytes) = idl_bytes.as_ref()
                    && !bytes.is_empty()
                {
                    let fallback_hash;
                    let hash = match idl_sha256.as_deref() {
                        Some(hash) => hash,
                        None => {
                            fallback_hash = sha256_hex(bytes);
                            &fallback_hash
                        }
                    };
                    let blob_key = self
                        .put_idl_version_blob(&program_address, hash, bytes)
                        .await?
                        .context("IDL version blob store disabled")?;
                    let result = sqlx::query(
                        r#"
                        UPDATE program_idl_versions
                        SET idl_bytes = NULL, idl_bytes_blob_key = ?
                        WHERE id = ?
                          AND idl_bytes_blob_key IS NULL;
                        "#,
                    )
                    .bind(&blob_key)
                    .bind(&id)
                    .execute(&self.pool)
                    .await?;
                    if result.rows_affected() > 0 {
                        report.program_idl_version_bytes += 1;
                        report.bytes_moved += bytes.len() as u64;
                    }
                }

                if idl_text_blob_key.is_none()
                    && let Some(text) = idl_text.as_ref()
                    && !text.is_empty()
                {
                    let fallback_hash;
                    let hash = match idl_sha256.as_deref() {
                        Some(hash) => hash,
                        None => {
                            fallback_hash = sha256_hex(text.as_bytes());
                            &fallback_hash
                        }
                    };
                    let blob_key = self
                        .put_idl_text_blob(&program_address, hash, text)
                        .await?
                        .context("IDL text blob store disabled")?;
                    let result = sqlx::query(
                        r#"
                        UPDATE program_idl_versions
                        SET idl_text = NULL, idl_text_blob_key = ?
                        WHERE id = ?
                          AND idl_text_blob_key IS NULL;
                        "#,
                    )
                    .bind(&blob_key)
                    .bind(&id)
                    .execute(&self.pool)
                    .await?;
                    if result.rows_affected() > 0 {
                        report.program_idl_version_texts += 1;
                        report.bytes_moved += text.len() as u64;
                    }
                }
            }
        }
    }

    async fn migrate_disassembly_blobs(&self, report: &mut BlobMigrationReport) -> Result<()> {
        loop {
            let rows = sqlx::query(
                r#"
                SELECT version_id, program_address, program_sha256, asm_sha256, asm_text
                FROM program_version_disassemblies
                WHERE asm_text_blob_key IS NULL
                  AND length(asm_text) > 0
                LIMIT 25;
                "#,
            )
            .fetch_all(&self.pool)
            .await?;
            if rows.is_empty() {
                return Ok(());
            }

            for row in rows {
                let version_id = row.try_get::<String, _>("version_id")?;
                let program_address = row.try_get::<String, _>("program_address")?;
                let program_sha256 = row.try_get::<String, _>("program_sha256")?;
                let asm_sha256 = row.try_get::<String, _>("asm_sha256")?;
                let asm_text = row.try_get::<String, _>("asm_text")?;
                let bytes_len = asm_text.len() as u64;
                let blob_key = self
                    .put_disassembly_blob(&program_address, &program_sha256, &asm_sha256, &asm_text)
                    .await?
                    .context("program disassembly blob store disabled")?;

                let result = sqlx::query(
                    r#"
                    UPDATE program_version_disassemblies
                    SET asm_text = '', asm_text_blob_key = ?
                    WHERE version_id = ?
                      AND asm_text_blob_key IS NULL;
                    "#,
                )
                .bind(&blob_key)
                .bind(&version_id)
                .execute(&self.pool)
                .await?;
                if result.rows_affected() > 0 {
                    report.disassemblies += 1;
                    report.bytes_moved += bytes_len;
                }
            }
        }
    }

    async fn put_program_version_blob(
        &self,
        program_address: &str,
        slot: i64,
        version_id: &str,
        program_sha256: &str,
        bytes: &[u8],
    ) -> Result<Option<String>> {
        self.put_blob(
            program_version_blob_key(program_address, slot, version_id, program_sha256),
            bytes,
            "program version",
        )
        .await
    }

    async fn put_idl_version_blob(
        &self,
        program_address: &str,
        idl_sha256: &str,
        bytes: &[u8],
    ) -> Result<Option<String>> {
        self.put_blob(
            idl_version_blob_key(program_address, idl_sha256),
            bytes,
            "IDL version",
        )
        .await
    }

    async fn put_idl_text_blob(
        &self,
        program_address: &str,
        idl_sha256: &str,
        text: &str,
    ) -> Result<Option<String>> {
        self.put_blob(
            idl_text_blob_key(program_address, idl_sha256),
            text.as_bytes(),
            "IDL text",
        )
        .await
    }

    async fn put_disassembly_blob(
        &self,
        program_address: &str,
        program_sha256: &str,
        asm_sha256: &str,
        asm_text: &str,
    ) -> Result<Option<String>> {
        self.put_blob(
            disassembly_blob_key(program_address, program_sha256, asm_sha256),
            asm_text.as_bytes(),
            "program disassembly",
        )
        .await
    }

    async fn put_program_diff_blob(
        &self,
        program_address: &str,
        slot: i64,
        version_id: &str,
        before_program_sha256: &str,
        after_program_sha256: &str,
        diff_sha256: &str,
        diff_text: &str,
    ) -> Result<Option<String>> {
        self.put_blob(
            program_diff_blob_key(
                program_address,
                slot,
                version_id,
                before_program_sha256,
                after_program_sha256,
                diff_sha256,
            ),
            diff_text.as_bytes(),
            "program diff",
        )
        .await
    }

    async fn put_blob(&self, key: String, bytes: &[u8], label: &str) -> Result<Option<String>> {
        self.blob_store
            .put_bytes(&key, bytes)
            .await
            .with_context(|| format!("store {label} blob {key}"))
    }

    async fn blob_bytes_or_inline(
        &self,
        blob_key: Option<String>,
        inline: Option<Vec<u8>>,
        label: &str,
    ) -> Result<Option<Vec<u8>>> {
        let Some(blob_key) = blob_key else {
            return Ok(inline);
        };

        if let Some(bytes) = self.blob_store.get_bytes(&blob_key).await? {
            return Ok(Some(bytes));
        }
        if let Some(bytes) = inline
            && !bytes.is_empty()
        {
            return Ok(Some(bytes));
        }

        bail!("missing {label} blob {blob_key}");
    }

    async fn blob_text_or_inline(
        &self,
        blob_key: Option<String>,
        inline: Option<String>,
        label: &str,
    ) -> Result<Option<String>> {
        let Some(blob_key) = blob_key else {
            return Ok(inline);
        };

        if let Some(bytes) = self.blob_store.get_bytes(&blob_key).await? {
            return String::from_utf8(bytes)
                .map(Some)
                .with_context(|| format!("{label} blob {blob_key} is not valid UTF-8"));
        }
        if let Some(text) = inline
            && !text.is_empty()
        {
            return Ok(Some(text));
        }

        bail!("missing {label} blob {blob_key}");
    }

    async fn ensure_column(
        &self,
        table_name: &str,
        column_name: &str,
        definition: &str,
    ) -> Result<()> {
        if self.table_has_column(table_name, column_name).await? {
            return Ok(());
        }

        let statement = format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition};");
        sqlx::query(&statement).execute(&self.pool).await?;
        Ok(())
    }

    async fn table_has_column(&self, table_name: &str, column_name: &str) -> Result<bool> {
        let pragma = format!("PRAGMA table_info({table_name});");
        let rows = sqlx::query(&pragma).fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .any(|row| row.try_get::<String, _>("name").ok().as_deref() == Some(column_name)))
    }
}

fn program_version_blob_key(
    program_address: &str,
    slot: i64,
    version_id: &str,
    program_sha256: &str,
) -> String {
    format!(
        "programs/{}/versions/{}-{}/program-{}.bin",
        safe_path_component(program_address),
        slot,
        safe_path_component(version_id),
        safe_path_component(program_sha256)
    )
}

fn idl_version_blob_key(program_address: &str, idl_sha256: &str) -> String {
    format!(
        "idl-bytes/{}/{}.bin",
        safe_path_component(program_address),
        safe_path_component(idl_sha256)
    )
}

fn idl_text_blob_key(program_address: &str, idl_sha256: &str) -> String {
    format!(
        "idl-text/{}/{}.json",
        safe_path_component(program_address),
        safe_path_component(idl_sha256)
    )
}

fn disassembly_blob_key(program_address: &str, program_sha256: &str, asm_sha256: &str) -> String {
    format!(
        "programs/{}/disassembly/{}/{}.asm",
        safe_path_component(program_address),
        safe_path_component(program_sha256),
        safe_path_component(asm_sha256)
    )
}

fn program_diff_blob_key(
    program_address: &str,
    slot: i64,
    version_id: &str,
    before_program_sha256: &str,
    after_program_sha256: &str,
    diff_sha256: &str,
) -> String {
    format!(
        "programs/{}/diffs/{}-{}/{}..{}-{}.patch",
        safe_path_component(program_address),
        slot,
        safe_path_component(version_id),
        safe_path_component(&short_hash(before_program_sha256)),
        safe_path_component(&short_hash(after_program_sha256)),
        safe_path_component(diff_sha256)
    )
}

fn short_hash(value: &str) -> String {
    value.chars().take(12).collect()
}

fn unified_asm_diff(
    before_label: &str,
    after_label: &str,
    before_text: &str,
    after_text: &str,
) -> String {
    const CONTEXT_LINES: usize = 4;

    let before = before_text.lines().collect::<Vec<_>>();
    let after = after_text.lines().collect::<Vec<_>>();
    let max_len = before.len().max(after.len());
    let changed = (0..max_len)
        .filter(|index| before.get(*index) != after.get(*index))
        .collect::<Vec<_>>();

    let mut output = String::new();
    output.push_str(&format!("--- previous.asm {before_label}\n"));
    output.push_str(&format!("+++ current.asm {after_label}\n"));
    if changed.is_empty() {
        return output;
    }

    let mut ranges = Vec::<(usize, usize)>::new();
    for index in changed {
        let start = index.saturating_sub(CONTEXT_LINES);
        let end = (index + CONTEXT_LINES + 1).min(max_len);
        if let Some((_, previous_end)) = ranges.last_mut()
            && start <= *previous_end
        {
            *previous_end = (*previous_end).max(end);
            continue;
        }
        ranges.push((start, end));
    }

    for (start, end) in ranges {
        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            start + 1,
            end.saturating_sub(start),
            start + 1,
            end.saturating_sub(start)
        ));
        for index in start..end {
            match (before.get(index), after.get(index)) {
                (Some(left), Some(right)) if left == right => {
                    output.push(' ');
                    output.push_str(right);
                    output.push('\n');
                }
                (Some(left), Some(right)) => {
                    output.push('-');
                    output.push_str(left);
                    output.push('\n');
                    output.push('+');
                    output.push_str(right);
                    output.push('\n');
                }
                (Some(left), None) => {
                    output.push('-');
                    output.push_str(left);
                    output.push('\n');
                }
                (None, Some(right)) => {
                    output.push('+');
                    output.push_str(right);
                    output.push('\n');
                }
                (None, None) => {}
            }
        }
    }

    output
}

fn safe_path_component(value: &str) -> String {
    let component = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();

    if component.is_empty() {
        "_".to_owned()
    } else {
        component
    }
}

#[derive(Clone, Debug)]
struct ProgramVersionEventSource {
    id: String,
    event_id: String,
    signature: String,
    instruction_path: String,
    instruction_order: String,
    kind: String,
    program_address: String,
    loader: String,
    source_account: String,
    authority: Option<String>,
    slot: i64,
    block_time: Option<i64>,
    indexed_at: i64,
    uses_direct_account_writes: bool,
}

impl ProgramVersionEventSource {
    fn from_event(event: &ChainEvent) -> Option<Self> {
        if event.target_type != "program"
            || !matches!(
                event.kind.as_str(),
                event_kind::PROGRAM_DEPLOYED | event_kind::PROGRAM_UPGRADED
            )
        {
            return None;
        }

        let loader = event.payload.get("loader")?.as_str()?.to_owned();
        let buffer_account = event
            .payload
            .get("buffer_account")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        let (source_account, uses_direct_account_writes) = match buffer_account {
            Some(buffer_account) => (buffer_account, false),
            None if loader != "upgradeable" => (event.target_address.clone(), true),
            None => return None,
        };
        let authority = event
            .payload
            .get("authority")
            .and_then(|value| value.as_str())
            .map(str::to_owned)
            .or_else(|| event.actor.clone());

        Some(Self {
            id: event.id.clone(),
            event_id: event.id.clone(),
            signature: event.signature.clone(),
            instruction_path: event.instruction_path.clone(),
            instruction_order: instruction_order_key(&event.instruction_path),
            kind: event.kind.clone(),
            program_address: event.target_address.clone(),
            loader,
            source_account,
            authority,
            slot: event.slot,
            block_time: event.block_time,
            indexed_at: event.indexed_at,
            uses_direct_account_writes,
        })
    }
}

#[derive(Clone, Debug)]
struct ProgramIdlVersionEventSource {
    id: String,
    event_id: String,
    signature: String,
    instruction_path: String,
    instruction_order: String,
    kind: String,
    origin: String,
    program_address: String,
    metadata_account: String,
    source_account: String,
    authority: Option<String>,
    slot: i64,
    block_time: Option<i64>,
    indexed_at: i64,
    encoding: Option<String>,
    compression: Option<String>,
    format: Option<String>,
    data_source: Option<String>,
    reset_data: bool,
    preserves_previous_data: bool,
    closed: bool,
}

impl ProgramIdlVersionEventSource {
    fn from_event(event: &ChainEvent) -> Option<Self> {
        if event.target_type != "program" || !is_program_idl_event_kind(&event.kind) {
            return None;
        }

        let metadata_account = event.payload.get("metadata_account")?.as_str()?.to_owned();
        let source_account = event
            .payload
            .get("idl_source_account")
            .and_then(|value| value.as_str())
            .unwrap_or(&metadata_account)
            .to_owned();
        let origin = event
            .payload
            .get("idl_origin")
            .and_then(|value| value.as_str())
            .unwrap_or(anchor_idl::PROGRAM_METADATA_IDL_ORIGIN)
            .to_owned();
        let instruction_name = event
            .payload
            .get("instruction_name")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let has_inline_data = event
            .payload
            .get("inline_data_len")
            .and_then(|value| value.as_i64())
            .is_some_and(|value| value > 0);
        let reset_data = origin == anchor_idl::LEGACY_ANCHOR_IDL_ORIGIN
            || matches!(instruction_name, "initialize" | "set_data")
            || has_inline_data;
        let preserves_previous_data = matches!(
            event.kind.as_str(),
            event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
                | event_kind::PROGRAM_IDL_IMMUTABLE
                | event_kind::PROGRAM_IDL_RESIZED
        );
        let closed = event.kind == event_kind::PROGRAM_IDL_CLOSED;

        Some(Self {
            id: event.id.clone(),
            event_id: event.id.clone(),
            signature: event.signature.clone(),
            instruction_path: event.instruction_path.clone(),
            instruction_order: instruction_order_key(&event.instruction_path),
            kind: event.kind.clone(),
            origin,
            program_address: event.target_address.clone(),
            metadata_account,
            source_account,
            authority: event
                .payload
                .get("authority")
                .and_then(|value| value.as_str())
                .map(str::to_owned)
                .or_else(|| event.actor.clone()),
            slot: event.slot,
            block_time: event.block_time,
            indexed_at: event.indexed_at,
            encoding: payload_string(&event.payload, "encoding"),
            compression: payload_string(&event.payload, "compression"),
            format: payload_string(&event.payload, "format"),
            data_source: payload_string(&event.payload, "data_source"),
            reset_data,
            preserves_previous_data,
            closed,
        })
    }
}

pub(crate) fn apply_program_version_payload(
    payload: &mut serde_json::Value,
    summary: &ProgramVersionSummary,
) -> Result<()> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("program version payload is not a JSON object"))?;
    object.insert(
        "program_version_id".to_owned(),
        serde_json::json!(summary.id.clone()),
    );
    object.insert(
        "program_bytes_available".to_owned(),
        serde_json::json!(summary.complete),
    );
    object.insert(
        "program_bytes_chunk_count".to_owned(),
        serde_json::json!(summary.chunk_count),
    );
    object.insert(
        "program_bytes_len".to_owned(),
        summary
            .program_len
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "program_bytes_sha256".to_owned(),
        summary
            .program_sha256
            .clone()
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "program_bytes_source_account".to_owned(),
        serde_json::json!(summary.source_account.clone()),
    );
    object.insert(
        "program_bytes_missing_ranges".to_owned(),
        serde_json::to_value(&summary.missing_ranges)?,
    );
    Ok(())
}

pub(crate) fn apply_idl_version_payload(
    payload: &mut serde_json::Value,
    version: &ProgramIdlVersionRecord,
) -> Result<()> {
    let object = payload
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("IDL version payload is not a JSON object"))?;
    object.insert(
        "idl_version_id".to_owned(),
        serde_json::json!(version.id.clone()),
    );
    object.insert(
        "idl_bytes_available".to_owned(),
        serde_json::json!(version.complete),
    );
    object.insert(
        "idl_bytes_chunk_count".to_owned(),
        serde_json::json!(version.chunk_count),
    );
    object.insert(
        "idl_bytes_len".to_owned(),
        version
            .idl_len
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "idl_bytes_sha256".to_owned(),
        version
            .idl_sha256
            .clone()
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null),
    );
    object.insert(
        "idl_origin".to_owned(),
        serde_json::json!(version.origin.clone()),
    );
    object.insert(
        "idl_source_account".to_owned(),
        serde_json::json!(version.source_account.clone()),
    );
    object.insert(
        "idl_bytes_missing_ranges".to_owned(),
        serde_json::to_value(&version.missing_ranges)?,
    );
    Ok(())
}

fn is_program_idl_event_kind(kind: &str) -> bool {
    matches!(
        kind,
        event_kind::PROGRAM_IDL_INITIALIZED
            | event_kind::PROGRAM_IDL_UPDATED
            | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
            | event_kind::PROGRAM_IDL_IMMUTABLE
            | event_kind::PROGRAM_IDL_RESIZED
            | event_kind::PROGRAM_IDL_CLOSED
    )
}

fn payload_string(payload: &serde_json::Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

fn decode_idl_text(
    bytes: &[u8],
    encoding: Option<&str>,
    compression: Option<&str>,
    format: Option<&str>,
) -> Result<Option<String>> {
    if format.is_some_and(|value| value != "json" && value != "yaml" && value != "toml") {
        return Ok(None);
    }

    let decoded_bytes = decompress_metadata_bytes(bytes, compression)?;
    let text = match encoding.unwrap_or("utf8") {
        "utf8" => String::from_utf8(decoded_bytes).ok(),
        "none" => Some(hex::encode(decoded_bytes)),
        "base58" => Some(bs58::encode(decoded_bytes).into_string()),
        "base64" => Some(BASE64.encode(decoded_bytes)),
        _ => None,
    };

    Ok(text)
}

fn decompress_metadata_bytes(bytes: &[u8], compression: Option<&str>) -> Result<Vec<u8>> {
    match compression.unwrap_or("none") {
        "none" => Ok(bytes.to_vec()),
        "zlib" => {
            let mut decoder = ZlibDecoder::new(bytes);
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            Ok(output)
        }
        "gzip" => {
            let mut decoder = GzDecoder::new(bytes);
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            Ok(output)
        }
        _ => Ok(bytes.to_vec()),
    }
}

fn row_to_event(row: sqlx::sqlite::SqliteRow) -> Result<ChainEvent> {
    Ok(ChainEvent {
        id: row.try_get("id")?,
        signature: row.try_get("signature")?,
        instruction_path: row.try_get("instruction_path")?,
        kind: row.try_get("kind")?,
        target_type: row.try_get("target_type")?,
        target_address: row.try_get("target_address")?,
        actor: row.try_get("actor")?,
        slot: row.try_get("slot")?,
        block_time: row.try_get("block_time")?,
        indexed_at: row.try_get("indexed_at")?,
        summary: row.try_get("summary")?,
        tags: serde_json::from_str::<Vec<String>>(&row.try_get::<String, _>("tags_json")?)?,
        payload: serde_json::from_str::<serde_json::Value>(
            &row.try_get::<String, _>("payload_json")?,
        )?,
        delivery_attempts: row.try_get("delivery_attempts")?,
        delivered_at: row.try_get("delivered_at")?,
        last_error: row.try_get("last_error")?,
        next_retry_at: row.try_get("next_retry_at")?,
    })
}

fn row_to_telegram_subscription(row: sqlx::sqlite::SqliteRow) -> Result<TelegramSubscription> {
    Ok(TelegramSubscription {
        id: row.try_get("id")?,
        label: row.try_get("label")?,
        user_id: row.try_get("user_id")?,
        chat_id: row.try_get("chat_id")?,
        thread_id: row.try_get("thread_id")?,
        scope: row
            .try_get::<String, _>("scope")?
            .parse::<TelegramEventScope>()
            .map_err(anyhow::Error::msg)?,
        target_address: row.try_get("target_address")?,
        required_tags: serde_json::from_str::<Vec<String>>(
            &row.try_get::<String, _>("required_tags_json")?,
        )?,
        status: row.try_get("status")?,
        payment_link_id: row.try_get("payment_link_id")?,
        created_at: row.try_get("created_at")?,
    })
}

fn row_to_pending_telegram_delivery(
    row: sqlx::sqlite::SqliteRow,
) -> Result<PendingTelegramDelivery> {
    Ok(PendingTelegramDelivery {
        id: row.try_get("telegram_delivery_id")?,
        delivery_attempts: row.try_get("telegram_delivery_attempts")?,
        subscription: TelegramSubscription {
            id: row.try_get("subscription_id")?,
            label: row.try_get("subscription_label")?,
            user_id: row.try_get("subscription_user_id")?,
            chat_id: row.try_get("subscription_chat_id")?,
            thread_id: row.try_get("subscription_thread_id")?,
            scope: row
                .try_get::<String, _>("subscription_scope")?
                .parse::<TelegramEventScope>()
                .map_err(anyhow::Error::msg)?,
            target_address: row.try_get("subscription_target_address")?,
            required_tags: serde_json::from_str::<Vec<String>>(
                &row.try_get::<String, _>("subscription_required_tags_json")?,
            )?,
            status: row.try_get("subscription_status")?,
            payment_link_id: row.try_get("subscription_payment_link_id")?,
            created_at: row.try_get("subscription_created_at")?,
        },
        event: row_to_event(row)?,
    })
}

fn row_to_program_activity_state(row: sqlx::sqlite::SqliteRow) -> Result<ProgramActivityState> {
    Ok(ProgramActivityState {
        program_id: row.try_get("program_id")?,
        protocol_slug: row.try_get("protocol_slug")?,
        protocol_name: row.try_get("protocol_name")?,
        program_label: row.try_get("program_label")?,
        last_signature: row.try_get("last_signature")?,
        last_instruction_path: row.try_get("last_instruction_path")?,
        last_slot: row.try_get("last_slot")?,
        last_block_time: row.try_get("last_block_time")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn row_to_protocol_metric_observation(
    row: sqlx::sqlite::SqliteRow,
) -> Result<ProtocolMetricObservation> {
    Ok(ProtocolMetricObservation {
        id: row.try_get("id")?,
        signature: row.try_get("signature")?,
        instruction_path: row.try_get("instruction_path")?,
        slot: row.try_get("slot")?,
        block_time: row.try_get("block_time")?,
        protocol_slug: row.try_get("protocol_slug")?,
        protocol_name: row.try_get("protocol_name")?,
        program_id: row.try_get("program_id")?,
        program_label: row.try_get("program_label")?,
        instruction_name: row.try_get("instruction_name")?,
        metric_key: row.try_get("metric_key")?,
        metric_label: row.try_get("metric_label")?,
        metric_value: row.try_get("metric_value")?,
        subject_address: row.try_get("subject_address")?,
        subject_kind: row.try_get("subject_kind")?,
        subject_label: row.try_get("subject_label")?,
        payload: serde_json::from_str::<serde_json::Value>(
            &row.try_get::<String, _>("payload_json")?,
        )?,
        observed_at: row.try_get("observed_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        liquidity_pools::{
            LIQUIDITY_POOL_SNAPSHOT_RETENTION_DEFAULT, LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED,
        },
        models::{event_kind, target_type},
        program_bytes::{instruction_order_key, sha256_hex},
    };
    use serde_json::json;

    #[tokio::test]
    async fn prunes_liquidity_pool_snapshots_by_retention_class() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "firewatch-liquidity-snapshot-retention-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let now = now_ts();
        let default_old = sample_liquidity_snapshot(
            "default-old",
            LIQUIDITY_POOL_SNAPSHOT_RETENTION_DEFAULT,
            now - 5 * 24 * 60 * 60,
        );
        let watched_recent = sample_liquidity_snapshot(
            "watched-recent",
            LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED,
            now - 5 * 24 * 60 * 60,
        );
        let watched_old = sample_liquidity_snapshot(
            "watched-old",
            LIQUIDITY_POOL_SNAPSHOT_RETENTION_WATCHED,
            now - 8 * 24 * 60 * 60,
        );

        db.upsert_liquidity_pool_snapshot(&default_old).await?;
        db.upsert_liquidity_pool_snapshot(&watched_recent).await?;
        db.upsert_liquidity_pool_snapshot(&watched_old).await?;

        let deleted = db.prune_liquidity_pool_snapshots(3, 7).await?;
        assert_eq!(deleted, 2);

        let remaining =
            sqlx::query_scalar::<_, String>("SELECT id FROM liquidity_pool_snapshots ORDER BY id;")
                .fetch_all(&db.pool)
                .await?;
        assert_eq!(remaining, vec!["watched-recent".to_owned()]);

        Ok(())
    }

    fn sample_liquidity_snapshot(
        id: &str,
        retention_class: &str,
        block_time: i64,
    ) -> LiquidityPoolSnapshotRecord {
        LiquidityPoolSnapshotRecord {
            id: id.to_owned(),
            pool_address: "Pool111111111111111111111111111111111111111".to_owned(),
            dex_id: "test-dex".to_owned(),
            base_token_mint: "BaseMint1111111111111111111111111111111111".to_owned(),
            quote_token_mint: "QuoteMint11111111111111111111111111111111".to_owned(),
            signature: format!("{id}-signature"),
            slot: block_time,
            block_time: Some(block_time),
            base_reserve: Some(10.0),
            quote_reserve: Some(20.0),
            base_vault: Some("BaseVault111111111111111111111111111111111".to_owned()),
            quote_vault: Some("QuoteVault1111111111111111111111111111111".to_owned()),
            estimated_liquidity_usd: Some(30.0),
            retention_class: retention_class.to_owned(),
            observed_at: block_time,
        }
    }

    #[tokio::test]
    async fn recent_unlabeled_addresses_reads_recent_event_window() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "firewatch-recent-unlabeled-addresses-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let labeled_target = "LabeledTarget111111111111111111111111111111".to_owned();
        let actor = "Actor11111111111111111111111111111111111111".to_owned();
        let member = "Member111111111111111111111111111111111111".to_owned();
        db.upsert_address_label(
            &labeled_target,
            "known.sol",
            "test",
            "domain_account",
            None,
            100,
        )
        .await?;
        let labels = db
            .address_labels(std::slice::from_ref(&labeled_target))
            .await?;
        assert_eq!(
            labels
                .get(&labeled_target)
                .map(|label| label.label.as_str()),
            Some("known.sol")
        );

        let name_account = "NameAccount11111111111111111111111111111111".to_owned();
        db.upsert_sns_domain(
            &name_account,
            "domain.sol",
            Some(&actor),
            None,
            None,
            false,
            Some(42),
            Some(event_kind::NAME_SERVICE_DOMAIN_UPDATED),
            100,
            None,
        )
        .await?;
        let domains = db
            .sns_domains_for_name_accounts(std::slice::from_ref(&name_account))
            .await?;
        assert_eq!(
            domains
                .get(&name_account)
                .map(|domain| domain.domain_name.as_str()),
            Some("domain.sol")
        );

        db.upsert_sns_primary_domain(&actor, &name_account, "primary.sol", 100)
            .await?;
        let primary_domains = db
            .sns_primary_domains_for_owners(std::slice::from_ref(&actor))
            .await?;
        assert_eq!(
            primary_domains
                .get(&actor)
                .map(|domain| domain.domain_name.as_str()),
            Some("primary.sol")
        );
        db.insert_event(&ChainEvent {
            id: "sig:0:test:labeled".to_owned(),
            signature: "sig".to_owned(),
            instruction_path: "0".to_owned(),
            kind: event_kind::PROGRAM_UPGRADED.to_owned(),
            target_type: target_type::PROGRAM.to_owned(),
            target_address: labeled_target.clone(),
            actor: Some(actor.clone()),
            slot: 42,
            block_time: Some(100),
            indexed_at: 100,
            summary: "test".to_owned(),
            tags: Vec::new(),
            payload: json!({ "member": member }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 100,
        })
        .await?;

        let addresses = db.recent_unlabeled_addresses(10).await?;
        assert!(!addresses.contains(&labeled_target));
        assert!(addresses.contains(&actor));
        assert!(addresses.contains(&member));

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn program_version_catalog_summaries_skip_incomplete_previous_versions() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "firewatch-program-version-summary-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let program = "ProgramSummary1111111111111111111111111111111".to_owned();
        for (id, instruction_order, slot, complete, bytes) in [
            ("old", "0000", 10_i64, 1_i64, Some(b"old".to_vec())),
            ("incomplete", "0001", 20_i64, 0_i64, None),
            ("latest", "0002", 30_i64, 1_i64, Some(b"latest".to_vec())),
        ] {
            sqlx::query(
                r#"
                INSERT INTO program_versions (
                    id,
                    event_id,
                    signature,
                    instruction_path,
                    instruction_order,
                    kind,
                    program_address,
                    loader,
                    source_account,
                    slot,
                    block_time,
                    indexed_at,
                    complete,
                    chunk_count,
                    program_len,
                    program_sha256,
                    program_bytes,
                    missing_ranges_json,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '[]', ?);
                "#,
            )
            .bind(id)
            .bind(format!("event-{id}"))
            .bind(format!("sig-{id}"))
            .bind(id)
            .bind(instruction_order)
            .bind(event_kind::PROGRAM_UPGRADED)
            .bind(&program)
            .bind("upgradeable")
            .bind("buffer")
            .bind(slot)
            .bind(slot + 100)
            .bind(slot + 200)
            .bind(complete)
            .bind(if bytes.is_some() { 1_i64 } else { 0_i64 })
            .bind(bytes.as_ref().map(|bytes| bytes.len() as i64))
            .bind(bytes.as_ref().map(|bytes| sha256_hex(bytes)))
            .bind(bytes)
            .bind(slot + 300)
            .execute(&db.pool)
            .await?;
        }

        let summaries = db
            .program_version_catalog_summaries(std::slice::from_ref(&program))
            .await?;
        assert_eq!(summaries.len(), 1);

        let summary = &summaries[0];
        assert_eq!(summary.program_id, program);
        assert_eq!(summary.version_count, 3);
        assert_eq!(summary.incomplete_count, 1);
        assert_eq!(summary.diffable_count, 1);
        let latest = summary.latest.as_ref().expect("latest summary");
        assert_eq!(latest.id, "latest");
        assert_eq!(latest.previous_version_id.as_deref(), Some("old"));

        let versions = db.list_program_versions(&program, 200).await?;
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].id, "latest");
        assert_eq!(versions[0].previous_version_id.as_deref(), Some("old"));
        assert_eq!(versions[0].previous_program_len, Some(3));
        assert_eq!(
            versions[0].previous_program_sha256.as_deref(),
            Some(sha256_hex(b"old").as_str())
        );

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn list_program_idl_versions_skips_incomplete_previous_versions() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "firewatch-idl-version-list-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let program = "ProgramIdlSummary1111111111111111111111111111".to_owned();
        for (id, instruction_order, slot, complete, bytes) in [
            (
                "old-idl",
                "0000",
                10_i64,
                1_i64,
                Some(br#"{"old":true}"#.to_vec()),
            ),
            ("incomplete-idl", "0001", 20_i64, 0_i64, None),
            (
                "latest-idl",
                "0002",
                30_i64,
                1_i64,
                Some(br#"{"latest":true}"#.to_vec()),
            ),
        ] {
            sqlx::query(
                r#"
                INSERT INTO program_idl_versions (
                    id,
                    event_id,
                    signature,
                    instruction_path,
                    instruction_order,
                    kind,
                    origin,
                    program_address,
                    metadata_account,
                    source_account,
                    slot,
                    block_time,
                    indexed_at,
                    complete,
                    chunk_count,
                    idl_len,
                    idl_sha256,
                    idl_bytes,
                    encoding,
                    compression,
                    format,
                    data_source,
                    missing_ranges_json,
                    updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, '[]', ?);
                "#,
            )
            .bind(id)
            .bind(format!("event-{id}"))
            .bind(format!("sig-{id}"))
            .bind(id)
            .bind(instruction_order)
            .bind(event_kind::PROGRAM_IDL_UPDATED)
            .bind("anchor")
            .bind(&program)
            .bind("metadata")
            .bind("source")
            .bind(slot)
            .bind(slot + 100)
            .bind(slot + 200)
            .bind(complete)
            .bind(if bytes.is_some() { 1_i64 } else { 0_i64 })
            .bind(bytes.as_ref().map(|bytes| bytes.len() as i64))
            .bind(bytes.as_ref().map(|bytes| sha256_hex(bytes)))
            .bind(bytes)
            .bind("utf8")
            .bind("zlib")
            .bind("json")
            .bind("account")
            .bind(slot + 300)
            .execute(&db.pool)
            .await?;
        }

        let versions = db.list_program_idl_versions(&program, 200).await?;
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].id, "latest-idl");
        assert_eq!(
            versions[0].previous_idl_version_id.as_deref(),
            Some("old-idl")
        );
        assert_eq!(versions[0].previous_idl_len, Some(12));
        assert_eq!(
            versions[0].previous_idl_sha256.as_deref(),
            Some(sha256_hex(br#"{"old":true}"#).as_str())
        );

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn assembles_program_version_from_buffer_writes() -> Result<()> {
        let db_path = std::env::temp_dir().join(format!(
            "firewatch-program-version-test-{}.db",
            uuid::Uuid::new_v4()
        ));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let buffer = "Buffer111111111111111111111111111111111111".to_owned();
        let program = "Program11111111111111111111111111111111111".to_owned();
        let authority = "Authority111111111111111111111111111111111".to_owned();
        for (path, offset, bytes) in [("0", 0_i64, b"abc".to_vec()), ("1", 3_i64, b"def".to_vec())]
        {
            db.insert_program_data_write(&ProgramDataWrite {
                id: format!("sig:{path}:program_data_write:{buffer}:{offset}"),
                signature: "sig".to_owned(),
                instruction_path: path.to_owned(),
                instruction_order: instruction_order_key(path),
                loader: "upgradeable".to_owned(),
                write_account: buffer.clone(),
                authority: Some(authority.clone()),
                slot: 10,
                block_time: Some(100),
                indexed_at: 100,
                offset,
                bytes_sha256: sha256_hex(&bytes),
                bytes,
            })
            .await?;
        }

        let event = ChainEvent {
            id: format!("sig:2:{}:{program}", event_kind::PROGRAM_UPGRADED),
            signature: "sig".to_owned(),
            instruction_path: "2".to_owned(),
            kind: event_kind::PROGRAM_UPGRADED.to_owned(),
            target_type: target_type::PROGRAM.to_owned(),
            target_address: program.clone(),
            actor: Some(authority.clone()),
            slot: 10,
            block_time: Some(100),
            indexed_at: 100,
            summary: "Program upgraded for test".to_owned(),
            tags: Vec::new(),
            payload: json!({
                "loader": "upgradeable",
                "program": program,
                "buffer_account": buffer,
                "authority": authority,
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 100,
        };
        db.insert_event(&event).await?;
        let summary = db
            .upsert_program_version_for_event(&event)
            .await?
            .expect("program version");

        assert!(summary.complete);
        assert_eq!(summary.program_len, Some(6));
        assert_eq!(
            db.program_version_bytes(
                &event.target_address,
                &event.id,
                ProgramVersionByteSide::After
            )
            .await?
            .as_deref(),
            Some(&b"abcdef"[..])
        );
        assert!(
            db.program_version_bytes(
                &event.target_address,
                &event.id,
                ProgramVersionByteSide::Before,
            )
            .await?
            .is_none()
        );

        let mut stale_event = event.clone();
        stale_event.block_time = None;
        db.upsert_program_version_for_event(&stale_event).await?;
        let stored_block_time: Option<i64> =
            sqlx::query_scalar("SELECT block_time FROM program_versions WHERE id = ?")
                .bind(&event.id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(stored_block_time, Some(100));

        let _ = std::fs::remove_file(db_path);
        Ok(())
    }

    #[tokio::test]
    async fn migrates_inline_program_blobs_to_blob_store() -> Result<()> {
        let suffix = uuid::Uuid::new_v4();
        let db_path = std::env::temp_dir().join(format!("firewatch-blob-migration-{suffix}.db"));
        let blob_dir = std::env::temp_dir().join(format!("firewatch-blob-migration-{suffix}"));
        let database_url = format!("sqlite://{}", db_path.display());
        let db = Database::connect(&database_url, BlobStore::new(None, None)?).await?;
        db.migrate().await?;

        let buffer = "Buffer111111111111111111111111111111111111".to_owned();
        let program = "Program11111111111111111111111111111111111".to_owned();
        let authority = "Authority111111111111111111111111111111111".to_owned();
        for (path, offset, bytes) in [("0", 0_i64, b"abc".to_vec()), ("1", 3_i64, b"def".to_vec())]
        {
            db.insert_program_data_write(&ProgramDataWrite {
                id: format!("sig:{path}:program_data_write:{buffer}:{offset}"),
                signature: "sig".to_owned(),
                instruction_path: path.to_owned(),
                instruction_order: instruction_order_key(path),
                loader: "upgradeable".to_owned(),
                write_account: buffer.clone(),
                authority: Some(authority.clone()),
                slot: 10,
                block_time: Some(100),
                indexed_at: 100,
                offset,
                bytes_sha256: sha256_hex(&bytes),
                bytes,
            })
            .await?;
        }

        let event = ChainEvent {
            id: format!("sig:2:{}:{program}", event_kind::PROGRAM_UPGRADED),
            signature: "sig".to_owned(),
            instruction_path: "2".to_owned(),
            kind: event_kind::PROGRAM_UPGRADED.to_owned(),
            target_type: target_type::PROGRAM.to_owned(),
            target_address: program.clone(),
            actor: Some(authority.clone()),
            slot: 10,
            block_time: Some(100),
            indexed_at: 100,
            summary: "Program upgraded for test".to_owned(),
            tags: Vec::new(),
            payload: json!({
                "loader": "upgradeable",
                "program": program,
                "buffer_account": buffer,
                "authority": authority,
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 100,
        };
        db.insert_event(&event).await?;
        db.upsert_program_version_for_event(&event).await?;
        drop(db);

        let db =
            Database::connect(&database_url, BlobStore::new(Some(blob_dir.clone()), None)?).await?;
        db.migrate().await?;
        let report = db.migrate_inline_blobs().await?;

        assert_eq!(report.program_versions, 1);
        assert_eq!(
            db.program_version_bytes(
                &event.target_address,
                &event.id,
                ProgramVersionByteSide::After,
            )
            .await?
            .as_deref(),
            Some(&b"abcdef"[..])
        );

        let inline_write_bytes: i64 =
            sqlx::query_scalar("SELECT COALESCE(SUM(length(bytes)), 0) FROM program_data_writes")
                .fetch_one(&db.pool)
                .await?;
        let inline_version_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(program_bytes)), 0) FROM program_versions",
        )
        .fetch_one(&db.pool)
        .await?;
        let blob_key_count: i64 = sqlx::query_scalar(
            r#"
            SELECT
                (SELECT COUNT(*) FROM program_data_writes WHERE bytes_blob_key IS NOT NULL)
              + (SELECT COUNT(*) FROM program_versions WHERE program_bytes_blob_key IS NOT NULL);
            "#,
        )
        .fetch_one(&db.pool)
        .await?;

        assert_eq!(inline_write_bytes, 0);
        assert_eq!(inline_version_bytes, 0);
        assert_eq!(blob_key_count, 1);

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(blob_dir);
        Ok(())
    }
}

fn normalize_telegram_username(value: &str) -> String {
    value.trim().trim_start_matches('@').to_ascii_lowercase()
}

fn row_to_firewatch_user(row: sqlx::sqlite::SqliteRow) -> Result<FirewatchUser> {
    row_to_firewatch_user_prefixed(&row, "")
}

fn row_to_firewatch_user_prefixed(
    row: &sqlx::sqlite::SqliteRow,
    prefix: &str,
) -> Result<FirewatchUser> {
    Ok(FirewatchUser {
        id: row.try_get(format!("{prefix}id").as_str())?,
        telegram_user_id: row.try_get(format!("{prefix}telegram_user_id").as_str())?,
        telegram_username: row.try_get(format!("{prefix}telegram_username").as_str())?,
        label: row.try_get(format!("{prefix}label").as_str())?,
        created_at: row.try_get(format!("{prefix}created_at").as_str())?,
        updated_at: row.try_get(format!("{prefix}updated_at").as_str())?,
    })
}

fn row_to_user_entitlement(row: sqlx::sqlite::SqliteRow) -> Result<UserEntitlement> {
    Ok(UserEntitlement {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        plan_id: row.try_get("plan_id")?,
        source: row.try_get("source")?,
        status: row.try_get("status")?,
        starts_at: row.try_get("starts_at")?,
        expires_at: row.try_get("expires_at")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn row_to_payment_link(row: sqlx::sqlite::SqliteRow) -> Result<PaymentLink> {
    row_to_payment_link_prefixed(&row, "")
}

fn row_to_payment_link_prefixed(
    row: &sqlx::sqlite::SqliteRow,
    prefix: &str,
) -> Result<PaymentLink> {
    Ok(PaymentLink {
        id: row.try_get(format!("{prefix}id").as_str())?,
        user_id: row.try_get(format!("{prefix}user_id").as_str())?,
        plan_id: row.try_get(format!("{prefix}plan_id").as_str())?,
        public_url: row.try_get(format!("{prefix}public_url").as_str())?,
        solana_pay_url: row.try_get(format!("{prefix}solana_pay_url").as_str())?,
        reference: row.try_get(format!("{prefix}reference").as_str())?,
        recipient: row.try_get(format!("{prefix}recipient").as_str())?,
        amount: row.try_get(format!("{prefix}amount").as_str())?,
        spl_token: row.try_get(format!("{prefix}spl_token").as_str())?,
        label: row.try_get(format!("{prefix}label").as_str())?,
        message: row.try_get(format!("{prefix}message").as_str())?,
        memo: row.try_get(format!("{prefix}memo").as_str())?,
        status: row.try_get(format!("{prefix}status").as_str())?,
        expires_at: row.try_get(format!("{prefix}expires_at").as_str())?,
        paid_at: row.try_get(format!("{prefix}paid_at").as_str())?,
        created_at: row.try_get(format!("{prefix}created_at").as_str())?,
    })
}

fn row_to_telegram_config_session(row: sqlx::sqlite::SqliteRow) -> Result<TelegramConfigSession> {
    row_to_telegram_config_session_prefixed(&row, "")
}

fn row_to_telegram_config_session_prefixed(
    row: &sqlx::sqlite::SqliteRow,
    prefix: &str,
) -> Result<TelegramConfigSession> {
    Ok(TelegramConfigSession {
        id: row.try_get(format!("{prefix}id").as_str())?,
        user_id: row.try_get(format!("{prefix}user_id").as_str())?,
        chat_id: row.try_get(format!("{prefix}chat_id").as_str())?,
        chat_title: row.try_get(format!("{prefix}chat_title").as_str())?,
        chat_type: row.try_get(format!("{prefix}chat_type").as_str())?,
        expires_at: row.try_get(format!("{prefix}expires_at").as_str())?,
        used_at: row.try_get(format!("{prefix}used_at").as_str())?,
        created_at: row.try_get(format!("{prefix}created_at").as_str())?,
    })
}

fn row_to_metric_alert_rule(row: sqlx::sqlite::SqliteRow) -> Result<MetricAlertRule> {
    Ok(MetricAlertRule {
        id: row.try_get("id")?,
        label: row.try_get("label")?,
        protocol_slug: row.try_get("protocol_slug")?,
        protocol_name: row.try_get("protocol_name")?,
        instruction_name: row.try_get("instruction_name")?,
        metric_key: row.try_get("metric_key")?,
        metric_label: row.try_get("metric_label")?,
        subject_address: row.try_get("subject_address")?,
        subject_kind: row.try_get("subject_kind")?,
        subject_label: row.try_get("subject_label")?,
        min_value: row.try_get("min_value")?,
        max_value: row.try_get("max_value")?,
        created_at: row.try_get("created_at")?,
    })
}

fn row_to_promo_code(row: sqlx::sqlite::SqliteRow) -> Result<PromoCode> {
    Ok(PromoCode {
        id: row.try_get("id")?,
        code: row.try_get("code")?,
        label: row.try_get("label")?,
        audience: row.try_get("audience")?,
        plan_id: row.try_get("plan_id")?,
        duration_months: row.try_get("duration_months")?,
        max_redemptions: row.try_get("max_redemptions")?,
        redeemed_count: row.try_get("redeemed_count")?,
        starts_at: row.try_get("starts_at")?,
        expires_at: row.try_get("expires_at")?,
        active: row.try_get::<i64, _>("active")? == 1,
        created_at: row.try_get("created_at")?,
    })
}

fn push_where_prefix(builder: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool) {
    if *has_where {
        builder.push(" AND ");
    } else {
        builder.push(" WHERE ");
        *has_where = true;
    }
}

fn push_event_filters(
    builder: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    filters: &EventListFilters<'_>,
) {
    if let Some(protocol_slug) = filters.protocol_slug {
        let protocol_tag = format!("%\"protocol:{protocol_slug}\"%");
        push_where_prefix(builder, has_where);
        if filters.related_multisigs.is_empty() {
            builder.push("tags_json LIKE ").push_bind(protocol_tag);
        } else {
            builder.push("(");
            builder.push("tags_json LIKE ").push_bind(protocol_tag);
            builder.push(" OR (target_type = 'multisig' AND target_address IN (");
            let mut separated = builder.separated(", ");
            for multisig in filters.related_multisigs {
                separated.push_bind(multisig.to_owned());
            }
            separated.push_unseparated(")))");
        }
    }

    if let Some(target_address) = filters.target_address {
        push_where_prefix(builder, has_where);
        builder
            .push("target_address = ")
            .push_bind(target_address.to_string());
    }

    if let Some(scope) = filters.scope {
        push_scope_filter(builder, has_where, scope);
    }

    if let Some(severity) = filters.severity {
        push_severity_filter(builder, has_where, severity);
    }

    if filters.known_defi_only {
        push_where_prefix(builder, has_where);
        builder
            .push("tags_json LIKE ")
            .push_bind("%\"watch:defi\"%".to_owned());
    }

    if let Some(tag) = filters.tag.map(str::trim).filter(|value| !value.is_empty()) {
        push_where_prefix(builder, has_where);
        push_like_expression(builder, "tags_json", &like_pattern(tag));
    }

    if let Some(from_ts) = filters.from_ts {
        push_where_prefix(builder, has_where);
        builder
            .push("COALESCE(block_time, indexed_at) >= ")
            .push_bind(from_ts);
    }

    if let Some(before_ts) = filters.before_ts {
        push_where_prefix(builder, has_where);
        builder
            .push("COALESCE(block_time, indexed_at) < ")
            .push_bind(before_ts);
    }

    if let Some(search) = filters
        .search
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let pattern = like_pattern(search);
        push_where_prefix(builder, has_where);
        builder.push("(");
        push_like_expression(builder, "summary", &pattern);
        builder.push(" OR ");
        push_like_expression(builder, "kind", &pattern);
        builder.push(" OR ");
        push_like_expression(builder, "target_address", &pattern);
        builder.push(" OR ");
        push_like_expression(builder, "actor", &pattern);
        builder.push(" OR ");
        push_like_expression(builder, "tags_json", &pattern);
        builder.push(" OR ");
        push_like_expression(
            builder,
            "json_extract(payload_json, '$.protocol_name')",
            &pattern,
        );
        builder.push(" OR ");
        push_like_expression(
            builder,
            "json_extract(payload_json, '$.instruction_name')",
            &pattern,
        );
        builder.push(")");
    }

    let default_or_squads_protocol = match filters.protocol_slug {
        Some(slug) => slug == "squads",
        None => true,
    };
    if filters.target_address.is_none() && default_or_squads_protocol {
        push_relevant_activity_filter(builder, has_where);
    }
}

fn push_scope_filter(builder: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool, scope: &str) {
    match scope {
        "critical" => {
            push_where_prefix(builder, has_where);
            push_critical_condition(builder);
        }
        "defi" => {
            push_where_prefix(builder, has_where);
            builder
                .push("tags_json LIKE ")
                .push_bind("%\"watch:defi\"%".to_owned());
        }
        "programs" => {
            push_where_prefix(builder, has_where);
            push_programs_condition(builder);
        }
        "idl" => {
            push_where_prefix(builder, has_where);
            push_idl_condition(builder);
        }
        "multisig" => {
            push_where_prefix(builder, has_where);
            builder.push("target_type = 'multisig'");
        }
        _ => {}
    }
}

fn push_severity_filter(
    builder: &mut QueryBuilder<'_, Sqlite>,
    has_where: &mut bool,
    severity: &str,
) {
    match severity {
        "critical" => {
            push_where_prefix(builder, has_where);
            push_critical_condition(builder);
        }
        "high" => {
            push_where_prefix(builder, has_where);
            builder.push("(");
            push_not_critical_condition(builder);
            builder.push(" AND ");
            push_high_condition(builder);
            builder.push(")");
        }
        "standard" => {
            push_where_prefix(builder, has_where);
            builder.push("(");
            push_not_critical_condition(builder);
            builder.push(" AND NOT ");
            push_high_condition(builder);
            builder.push(")");
        }
        _ => {}
    }
}

fn push_critical_condition(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push("(");
    builder
        .push("tags_json LIKE ")
        .push_bind("%\"severity:critical\"%".to_owned());
    builder.push(" OR ");
    builder
        .push("tags_json LIKE ")
        .push_bind("%\"signal:unknown_instruction\"%".to_owned());
    builder.push(")");
}

fn push_not_critical_condition(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push("NOT ");
    push_critical_condition(builder);
}

async fn update_missing_block_time(
    pool: &SqlitePool,
    table: &str,
    slot: i64,
    block_time: i64,
) -> Result<u64> {
    let mut builder = QueryBuilder::<Sqlite>::new("UPDATE ");
    builder.push(table);
    builder.push(" SET block_time = ");
    builder.push_bind(block_time);
    builder.push(" WHERE slot = ");
    builder.push_bind(slot);
    builder.push(" AND block_time IS NULL;");

    let result = builder.build().execute(pool).await?;
    Ok(result.rows_affected())
}

async fn pragma_i64(pool: &SqlitePool, statement: &str) -> Result<i64> {
    let row = sqlx::query(statement).fetch_one(pool).await?;
    row.try_get::<i64, _>(0).context("read SQLite pragma value")
}

fn push_high_condition(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push("(");
    builder
        .push("tags_json LIKE ")
        .push_bind("%\"severity:high\"%".to_owned());
    builder.push(" OR ");
    builder
        .push("tags_json LIKE ")
        .push_bind("%\"impact:high\"%".to_owned());
    builder.push(")");
}

fn push_programs_condition(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push("kind IN (");
    let mut separated = builder.separated(", ");
    for kind in [
        "program_deployed",
        "program_upgraded",
        "program_finalized",
        "program_migrated_to_loader_v4",
    ] {
        separated.push_bind(kind.to_owned());
    }
    separated.push_unseparated(")");
}

fn push_idl_condition(builder: &mut QueryBuilder<'_, Sqlite>) {
    builder.push("kind IN (");
    let mut separated = builder.separated(", ");
    for kind in [
        "program_idl_initialized",
        "program_idl_updated",
        "program_idl_authority_changed",
        "program_idl_immutable",
        "program_idl_resized",
        "program_idl_closed",
    ] {
        separated.push_bind(kind.to_owned());
    }
    separated.push_unseparated(")");
}

fn push_like_expression(builder: &mut QueryBuilder<'_, Sqlite>, expression: &str, pattern: &str) {
    builder.push("LOWER(COALESCE(");
    builder.push(expression);
    builder.push(", '')) LIKE ");
    builder.push_bind(pattern.to_owned());
    builder.push(" ESCAPE '\\'");
}

fn like_pattern(value: &str) -> String {
    let mut pattern = String::from("%");
    for character in value.trim().to_lowercase().chars() {
        match character {
            '%' | '_' | '\\' => {
                pattern.push('\\');
                pattern.push(character);
            }
            _ => pattern.push(character),
        }
    }
    pattern.push('%');
    pattern
}

fn push_relevant_activity_filter(builder: &mut QueryBuilder<'_, Sqlite>, has_where: &mut bool) {
    push_where_prefix(builder, has_where);
    builder.push("(");
    builder.push("target_type <> 'multisig'");
    builder.push(" OR tags_json LIKE ");
    builder.push_bind("%\"watch:defi\"%".to_owned());
    builder.push(" OR tags_json LIKE ");
    builder.push_bind("%\"severity:critical\"%".to_owned());
    builder.push(")");
}
