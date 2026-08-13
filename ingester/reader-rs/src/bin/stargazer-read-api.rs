use anyhow::{Context, Result};
use clap::Parser;
use stargazer_ledger_reader::api::{router, ReadApiConfig, ReadApiState};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(about = "Serve bounded, read-only queries from a Stargazer compact index")]
struct Args {
    /// Completed compact SQLite index.
    #[arg(long, env = "STARGAZER_INDEX_DB")]
    db: PathBuf,

    /// Optional finalized wallet_activity SQLite database. If omitted, the
    /// API probes the compact DB for the serving table on every request.
    #[arg(long, env = "STARGAZER_SERVING_DB")]
    serving_db: Option<PathBuf>,

    /// Optional directory containing current.json and alerts.json from the
    /// Classic controller watcher.
    #[arg(long, env = "STARGAZER_GOVERNANCE_DIR")]
    governance_dir: Option<PathBuf>,

    /// Interface to listen on. Use 0.0.0.0 to accept LAN connections.
    #[arg(long, env = "STARGAZER_API_HOST", default_value = "127.0.0.1")]
    host: IpAddr,

    #[arg(long, env = "STARGAZER_API_PORT", default_value_t = 8787)]
    port: u16,

    /// Abort SQLite statements that exceed this deadline.
    #[arg(long, env = "STARGAZER_API_QUERY_TIMEOUT_MS", default_value_t = 3000)]
    query_timeout_ms: u64,

    /// Maximum SQLite reads executing concurrently.
    #[arg(
        long,
        env = "STARGAZER_API_MAX_CONCURRENT_QUERIES",
        default_value_t = 4
    )]
    max_concurrent_queries: usize,

    /// Maximum rows returned by wallet pages and leaderboards.
    #[arg(long, env = "STARGAZER_API_MAX_PAGE_SIZE", default_value_t = 250)]
    max_page_size: usize,

    /// Allowed browser origins. Repeat the flag or use a comma-delimited env
    /// value. Use '*' only when the API is intentionally public.
    #[arg(
        long = "cors-origin",
        env = "STARGAZER_API_CORS_ORIGINS",
        value_delimiter = ',',
        default_value = "http://127.0.0.1:5173,http://localhost:5173"
    )]
    cors_origins: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("stargazer_read_api=info,tower_http=info")),
        )
        .init();
    let args = Args::parse();
    let address = SocketAddr::new(args.host, args.port);
    let state = ReadApiState::open(ReadApiConfig {
        index_db: args.db,
        serving_db: args.serving_db,
        governance_dir: args.governance_dir,
        query_timeout: Duration::from_millis(args.query_timeout_ms),
        max_concurrent_queries: args.max_concurrent_queries,
        max_page_size: args.max_page_size,
    })?;
    let app = router(state, &args.cors_origins)?;
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("binding read API to {address}"))?;
    tracing::info!(%address, "Stargazer read API listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("installing Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("installing SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
