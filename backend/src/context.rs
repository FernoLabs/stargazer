use crate::{config::Config, database::Database, models::TelegramBotProfile};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub http_client: reqwest::Client,
    pub telegram_bot_profile: Arc<RwLock<Option<Option<TelegramBotProfile>>>>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn shared(config: Config, db: Database, http_client: reqwest::Client) -> SharedState {
        Arc::new(Self {
            config,
            db,
            http_client,
            telegram_bot_profile: Arc::new(RwLock::new(None)),
        })
    }
}
