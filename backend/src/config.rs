use crate::protocols;
use anyhow::{Context, Result};
use reqwest::Url;
use std::{env, fs, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub http_bind: String,
    pub database_url: String,
    pub blob_store_dir: Option<PathBuf>,
    pub solana_grpc_endpoint: String,
    pub solana_grpc_x_token: Option<String>,
    pub solana_rpc_http_url: String,
    pub solana_explorer_base_url: String,
    pub solana_explorer_cluster: String,
    pub telegram_bot_token: Option<String>,
    pub telegram_api_base: String,
    pub firewatch_web_base_url: String,
    pub origin_api_token: Option<String>,
    pub cors_allowed_origin: String,
    pub delivery_poll_seconds: u64,
    pub reconnect_delay_seconds: u64,
    pub sns_refresh_seconds: u64,
    pub critical_sol_domains: Vec<String>,
    pub ferno_block_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let raw_grpc_endpoint =
            env::var("SOLANA_GRPC_ENDPOINT").context("SOLANA_GRPC_ENDPOINT is required")?;
        let raw_grpc_x_token = env::var("SOLANA_GRPC_X_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let (solana_grpc_endpoint, solana_grpc_x_token) =
            normalize_grpc_endpoint(raw_grpc_endpoint, raw_grpc_x_token);
        let solana_rpc_http_url = env::var("SOLANA_RPC_HTTP_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| {
                derive_rpc_http_url(&solana_grpc_endpoint, solana_grpc_x_token.as_deref())
            });
        let solana_explorer_cluster = env::var("SOLANA_EXPLORER_CLUSTER")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| detect_explorer_cluster(&solana_grpc_endpoint).to_owned());

        Ok(Self {
            http_bind: env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_owned()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/indexer.db".to_owned()),
            blob_store_dir: env::var("BLOB_STORE_DIR")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            solana_grpc_endpoint,
            solana_grpc_x_token,
            solana_rpc_http_url,
            solana_explorer_base_url: env::var("SOLANA_EXPLORER_BASE_URL")
                .unwrap_or_else(|_| "https://explorer.solana.com".to_owned()),
            solana_explorer_cluster,
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            telegram_api_base: env::var("TELEGRAM_API_BASE")
                .unwrap_or_else(|_| "https://api.telegram.org".to_owned()),
            firewatch_web_base_url: env::var("FIREWATCH_WEB_BASE_URL")
                .ok()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "https://watch.ferno.ag".to_owned()),
            origin_api_token: env::var("ORIGIN_API_TOKEN")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            cors_allowed_origin: env::var("CORS_ALLOWED_ORIGIN")
                .unwrap_or_else(|_| "http://127.0.0.1:5173".to_owned()),
            delivery_poll_seconds: env_u64("DELIVERY_POLL_SECONDS", 5),
            reconnect_delay_seconds: env_u64("RECONNECT_DELAY_SECONDS", 5),
            sns_refresh_seconds: env_u64("SNS_REFRESH_SECONDS", 300),
            critical_sol_domains: env_csv("CRITICAL_SOL_DOMAINS", protocols::FERNO_SOL_DOMAINS),
            ferno_block_base_url: env::var("FERNO_BLOCK_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "https://block.ferno.ag/block".to_owned()),
        })
    }

    pub fn prepare_dirs(&self) -> Result<()> {
        if let Some(database_path) = self.sqlite_path() {
            if let Some(parent) = database_path.parent() {
                fs::create_dir_all(parent)?;
            }
        }
        if let Some(blob_store_dir) = self.blob_store_dir.as_ref() {
            fs::create_dir_all(blob_store_dir)?;
        }
        Ok(())
    }

    fn sqlite_path(&self) -> Option<PathBuf> {
        let raw = self
            .database_url
            .strip_prefix("sqlite://")
            .or_else(|| self.database_url.strip_prefix("sqlite:"))?;

        if raw.is_empty() || raw == ":memory:" {
            return None;
        }

        Some(PathBuf::from(raw))
    }
}

fn normalize_grpc_endpoint(endpoint: String, x_token: Option<String>) -> (String, Option<String>) {
    if x_token.is_some() {
        return (endpoint, x_token);
    }

    let Ok(mut url) = Url::parse(&endpoint) else {
        return (endpoint, None);
    };

    let segments = match url.path_segments() {
        Some(segments) => segments
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>(),
        None => return (endpoint, None),
    };

    if segments.len() != 1 {
        return (endpoint, None);
    }

    let token = segments[0].to_owned();
    url.set_path("");

    (
        url.to_string().trim_end_matches('/').to_owned(),
        Some(token),
    )
}

fn derive_rpc_http_url(endpoint: &str, x_token: Option<&str>) -> String {
    match x_token {
        Some(token) => format!("{}/{}", endpoint.trim_end_matches('/'), token),
        None => endpoint.to_owned(),
    }
}

fn detect_explorer_cluster(endpoint: &str) -> &'static str {
    let endpoint = endpoint.to_ascii_lowercase();
    if endpoint.contains("devnet") {
        "devnet"
    } else if endpoint.contains("testnet") {
        "testnet"
    } else {
        "mainnet-beta"
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_csv(key: &str, defaults: &[&str]) -> Vec<String> {
    let mut values = env::var(key)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if values.is_empty() {
        values = defaults.iter().map(|value| value.to_string()).collect();
    }

    values.sort();
    values.dedup();
    values
}
