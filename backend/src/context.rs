use crate::{
    config::Config,
    database::Database,
    models::{IdlInstructionAlertRule, TelegramBotProfile},
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Notify, RwLock};

pub type IdlInstructionAlertRuleCache =
    HashMap<String, HashMap<String, Vec<IdlInstructionAlertRule>>>;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub http_client: reqwest::Client,
    pub telegram_bot_profile: Arc<RwLock<Option<Option<TelegramBotProfile>>>>,
    pub idl_instruction_alert_rules: Arc<RwLock<IdlInstructionAlertRuleCache>>,
    pub solana_filter_refresh: Arc<Notify>,
}

pub type SharedState = Arc<AppState>;

impl AppState {
    pub fn shared(config: Config, db: Database, http_client: reqwest::Client) -> SharedState {
        Arc::new(Self {
            config,
            db,
            http_client,
            telegram_bot_profile: Arc::new(RwLock::new(None)),
            idl_instruction_alert_rules: Arc::new(RwLock::new(HashMap::new())),
            solana_filter_refresh: Arc::new(Notify::new()),
        })
    }
}
