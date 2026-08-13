use anyhow::{Context, Result};
use clap::Parser;
use stargazer_alert_control::api::{router, AlertApiState};
use stargazer_alert_control::store::Store;
use stargazer_alert_control::telegram::{TelegramBot, TelegramBotConfig};
use stargazer_alert_control::BillingMode;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(about = "Serve Stargazer Telegram pairing and alert-policy control state")]
struct Args {
    /// Dedicated writable SQLite database. Never point this at the compact
    /// ledger index or serving snapshot.
    #[arg(long, env = "STARGAZER_ALERT_CONTROL_DB")]
    db: PathBuf,

    #[arg(
        long,
        env = "STARGAZER_ALERT_CONTROL_HOST",
        default_value = "127.0.0.1"
    )]
    host: IpAddr,

    #[arg(long, env = "STARGAZER_ALERT_CONTROL_PORT", default_value_t = 8788)]
    port: u16,

    /// Allowed browser origins. Prefer server-side frontend actions; they do
    /// not require CORS. Repeat this flag or use a comma-delimited env value.
    #[arg(
        long = "cors-origin",
        env = "STARGAZER_ALERT_CONTROL_CORS_ORIGINS",
        value_delimiter = ',',
        default_value = "http://127.0.0.1:5173,http://localhost:5173"
    )]
    cors_origins: Vec<String>,

    #[arg(long, env = "STARGAZER_TELEGRAM_BOT_USERNAME")]
    telegram_bot_username: String,

    /// BotFather token. Prefer the environment variable so the secret is not
    /// exposed in process arguments.
    #[arg(long, env = "STARGAZER_TELEGRAM_BOT_TOKEN", hide_env_values = true)]
    telegram_bot_token: Option<String>,

    #[arg(
        long,
        env = "STARGAZER_TELEGRAM_API_BASE",
        default_value = "https://api.telegram.org"
    )]
    telegram_api_base: String,

    #[arg(long, env = "STARGAZER_PUBLIC_APP_URL")]
    public_app_url: Option<String>,

    /// `unconfigured` creates no entitlement and collects no payment. `pilot`
    /// grants a bounded operator-configured pilot; no paid mode exists yet.
    #[arg(
        long,
        env = "STARGAZER_ALERT_BILLING_MODE",
        default_value = "unconfigured"
    )]
    billing_mode: BillingMode,

    #[arg(long, env = "STARGAZER_ALERT_PILOT_DAYS", default_value_t = 30)]
    pilot_days: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("stargazer_alert_control=info,tower_http=info")),
        )
        .init();
    let args = Args::parse();
    let store = Store::initialize(&args.db, args.billing_mode, args.pilot_days)?;

    let bot = match args.telegram_bot_token {
        Some(token) => {
            let bot = TelegramBot::initialize(
                TelegramBotConfig {
                    token,
                    username: args.telegram_bot_username.clone(),
                    api_base: args.telegram_api_base,
                    public_app_url: args.public_app_url,
                },
                store.clone(),
            )
            .await
            .context("initializing Telegram bot")?;
            Some(bot)
        }
        None => {
            tracing::warn!(
                "Telegram token is absent; health remains available but pairing POST fails closed"
            );
            None
        }
    };

    let state = AlertApiState::new(store, &args.telegram_bot_username, bot.is_some())?;
    let app = router(state, &args.cors_origins)?;
    let address = SocketAddr::new(args.host, args.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .with_context(|| format!("binding alert-control API to {address}"))?;
    tracing::info!(%address, billing_mode = %args.billing_mode, "Stargazer alert control listening");

    let server = axum::serve(listener, app);
    match bot {
        Some(bot) => {
            tokio::select! {
                result = server => result.context("serving alert-control API")?,
                result = bot.run() => result.context("running Telegram long poll")?,
                () = shutdown_signal() => {},
            }
        }
        None => {
            tokio::select! {
                result = server => result.context("serving alert-control API")?,
                () = shutdown_signal() => {},
            }
        }
    }
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
