use crate::store::{
    ClaimPairingOutcome, PausePolicyOutcome, PolicySummary, Store, TelegramIdentity,
};
use crate::{
    normalize_wallet_address, unix_timestamp, validate_bot_username, validate_pairing_token,
    PolicyState,
};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const POLL_TIMEOUT_SECONDS: u32 = 30;
const MAX_POLICIES_IN_MESSAGE: usize = 15;

pub struct TelegramBot {
    client: Client,
    api_prefix: String,
    username: String,
    store: Store,
    public_app_url: Option<String>,
}

pub struct TelegramBotConfig {
    pub token: String,
    pub username: String,
    pub api_base: String,
    pub public_app_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramEnvelope<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TelegramUser {
    id: i64,
    is_bot: bool,
    username: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Clone, Debug, Deserialize)]
struct TelegramMessage {
    chat: TelegramChat,
    #[serde(rename = "from")]
    sender: Option<TelegramUser>,
    text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramWebhookInfo {
    url: String,
}

#[derive(Debug, Serialize)]
struct GetUpdatesRequest {
    offset: i64,
    timeout: u32,
    allowed_updates: [&'static str; 1],
}

#[derive(Debug, Serialize)]
struct SendMessageRequest<'a> {
    chat_id: i64,
    text: &'a str,
    disable_web_page_preview: bool,
}

#[derive(Debug, Serialize)]
struct SetCommandsRequest {
    commands: Vec<TelegramCommandRegistration>,
}

#[derive(Debug, Serialize)]
struct TelegramCommandRegistration {
    command: &'static str,
    description: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedCommand<'a> {
    name: &'a str,
    argument: &'a str,
}

impl TelegramBot {
    pub async fn initialize(config: TelegramBotConfig, store: Store) -> Result<Self> {
        validate_bot_token(&config.token)?;
        let username = validate_bot_username(&config.username)?;
        let api_base = config.api_base.trim_end_matches('/');
        let parsed_base = reqwest::Url::parse(api_base).context("parsing Telegram API base URL")?;
        if !matches!(parsed_base.scheme(), "https" | "http") {
            bail!("Telegram API base URL must use http or https");
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(u64::from(POLL_TIMEOUT_SECONDS) + 15))
            .user_agent("stargazer-alert-control/0.1")
            .build()
            .context("building Telegram HTTP client")?;
        let bot = Self {
            client,
            api_prefix: format!("{api_base}/bot{}", config.token),
            username,
            store,
            public_app_url: normalize_public_app_url(config.public_app_url)?,
        };
        let me: TelegramUser = bot.call("getMe", &serde_json::json!({})).await?;
        if !me.is_bot {
            bail!("configured Telegram token does not belong to a bot");
        }
        let actual_username = me
            .username
            .as_deref()
            .context("Telegram getMe response has no bot username")?;
        if !actual_username.eq_ignore_ascii_case(&bot.username) {
            bail!("configured Telegram bot username does not match the bot token");
        }
        let webhook: TelegramWebhookInfo =
            bot.call("getWebhookInfo", &serde_json::json!({})).await?;
        if !webhook.url.is_empty() {
            bail!(
                "Telegram bot still has a webhook configured; remove it explicitly before using long polling"
            );
        }
        bot.register_commands().await?;
        Ok(bot)
    }

    pub async fn run(self) -> Result<()> {
        let mut offset = 0_i64;
        loop {
            let updates = match self.get_updates(offset).await {
                Ok(updates) => updates,
                Err(error) => {
                    tracing::warn!(error = %error, "Telegram long poll failed; retrying");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            for update in updates {
                offset = offset.max(update.update_id.saturating_add(1));
                if let Err(error) = self.handle_update(update).await {
                    tracing::warn!(error = %error, "Telegram update handling failed");
                }
            }
        }
    }

    async fn get_updates(&self, offset: i64) -> Result<Vec<TelegramUpdate>> {
        self.call(
            "getUpdates",
            &GetUpdatesRequest {
                offset,
                timeout: POLL_TIMEOUT_SECONDS,
                allowed_updates: ["message"],
            },
        )
        .await
    }

    async fn handle_update(&self, update: TelegramUpdate) -> Result<()> {
        let Some(message) = update.message else {
            return Ok(());
        };
        let Some(text) = message.text.as_deref() else {
            return Ok(());
        };
        let Some(command) = parse_command(text, &self.username) else {
            return Ok(());
        };
        if message.chat.kind != "private" {
            self.send_message(
                message.chat.id,
                "Open a private chat with this bot to manage wallet alerts.",
            )
            .await?;
            return Ok(());
        }
        let Some(sender) = message.sender else {
            return Ok(());
        };
        let identity = TelegramIdentity {
            chat_id: message.chat.id,
            user_id: sender.id,
            username: sender.username,
        };
        let reply = match command.name {
            "start" => self.start_reply(&identity, command.argument).await?,
            "help" => help_message().to_owned(),
            "list" => self.list_reply(&identity).await?,
            "settings" => self.settings_reply(&identity).await?,
            "unwatch" => self.unwatch_reply(&identity, command.argument).await?,
            _ => help_message().to_owned(),
        };
        self.send_message(message.chat.id, &reply).await
    }

    async fn start_reply(&self, identity: &TelegramIdentity, argument: &str) -> Result<String> {
        if argument.is_empty() {
            return Ok("Stargazer links this private Telegram chat to a wallet monitoring policy. Start from the Stargazer wallet report to create a 15-minute setup link, then open it here.\n\nUse /list to review linked wallets or /help for commands.".to_owned());
        }
        if argument.split_whitespace().count() != 1 {
            return Ok("This setup link is invalid. Create a new Telegram link from the Stargazer wallet report.".to_owned());
        }
        if validate_pairing_token(argument).is_err() {
            return Ok("This setup link is invalid. Create a new Telegram link from the Stargazer wallet report.".to_owned());
        }
        let token = argument.to_owned();
        let store = self.store.clone();
        let identity = identity.clone();
        let now = unix_timestamp()?;
        let outcome =
            tokio::task::spawn_blocking(move || store.claim_pairing(&token, &identity, now))
                .await
                .context("Telegram pairing worker stopped")??;
        Ok(match outcome {
            ClaimPairingOutcome::Claimed {
                wallet_address,
                policy_state: PolicyState::Active,
            } => format!(
                "Wallet linked: {wallet_address}\n\nA pilot entitlement was granted. The alert delivery worker is not connected in this POC, so this bot is not yet sending monitoring alerts. Use /list to review the policy or /unwatch {wallet_address} to stop it."
            ),
            ClaimPairingOutcome::Claimed {
                wallet_address,
                policy_state: PolicyState::AwaitingEntitlement,
            } => format!(
                "Wallet linked: {wallet_address}\n\nDelivery is not active yet because billing is not configured. No payment was collected and no paid entitlement was created. Use /list to review the policy."
            ),
            ClaimPairingOutcome::Claimed { .. } => {
                "Wallet linked. Use /list to review the policy.".to_owned()
            }
            ClaimPairingOutcome::AlreadyClaimed => {
                "This setup link has already been used. Use /list to review linked wallets or create a new link in Stargazer.".to_owned()
            }
            ClaimPairingOutcome::Expired => {
                "This setup link expired after 15 minutes. Create a new Telegram link from the Stargazer wallet report.".to_owned()
            }
            ClaimPairingOutcome::Unknown => {
                "This setup link is invalid. Create a new Telegram link from the Stargazer wallet report.".to_owned()
            }
            ClaimPairingOutcome::IdentityConflict => {
                "This Telegram chat is already associated with another Telegram identity. Contact support instead of retrying the link.".to_owned()
            }
        })
    }

    async fn list_reply(&self, identity: &TelegramIdentity) -> Result<String> {
        let store = self.store.clone();
        let identity = identity.clone();
        let now = unix_timestamp()?;
        let policies = tokio::task::spawn_blocking(move || store.list_policies(&identity, now))
            .await
            .context("Telegram policy-list worker stopped")??;
        Ok(format_policy_list(&policies))
    }

    async fn settings_reply(&self, identity: &TelegramIdentity) -> Result<String> {
        let mut reply = self.list_reply(identity).await?;
        match self.public_app_url.as_deref() {
            Some(url) => {
                reply.push_str("\n\nChange severity coverage by creating a new setup link at ");
                reply.push_str(url);
            }
            None => reply.push_str(
                "\n\nChange severity coverage by creating a new setup link in the Stargazer web app.",
            ),
        }
        Ok(reply)
    }

    async fn unwatch_reply(&self, identity: &TelegramIdentity, argument: &str) -> Result<String> {
        if argument.is_empty() || argument.split_whitespace().count() != 1 {
            return Ok("Usage: /unwatch G...\nThe full wallet address is required to prevent accidental removal.".to_owned());
        }
        let wallet_address = match normalize_wallet_address(argument) {
            Ok(address) => address,
            Err(_) => {
                return Ok(
                    "Usage: /unwatch G...\nEnter a checksum-valid Stellar G- or M-address."
                        .to_owned(),
                )
            }
        };
        let store = self.store.clone();
        let identity = identity.clone();
        let wallet_for_task = wallet_address.clone();
        let now = unix_timestamp()?;
        let outcome = tokio::task::spawn_blocking(move || {
            store.pause_policy(&identity, &wallet_for_task, now)
        })
        .await
        .context("Telegram unwatch worker stopped")??;
        Ok(match outcome {
            PausePolicyOutcome::Paused => {
                format!("Alerts paused for {wallet_address}. A pilot entitlement, if present, was revoked.")
            }
            PausePolicyOutcome::NotFound => {
                format!("No watch was found for {wallet_address}. Use /list to see linked wallets.")
            }
            PausePolicyOutcome::IdentityConflict => {
                "This Telegram chat is associated with another Telegram identity. Contact support."
                    .to_owned()
            }
        })
    }

    async fn register_commands(&self) -> Result<()> {
        let _: bool = self
            .call(
                "setMyCommands",
                &SetCommandsRequest {
                    commands: vec![
                        TelegramCommandRegistration {
                            command: "start",
                            description: "Link a Stargazer setup token",
                        },
                        TelegramCommandRegistration {
                            command: "list",
                            description: "List wallet policies",
                        },
                        TelegramCommandRegistration {
                            command: "settings",
                            description: "Review alert settings",
                        },
                        TelegramCommandRegistration {
                            command: "unwatch",
                            description: "Pause alerts for one wallet",
                        },
                        TelegramCommandRegistration {
                            command: "help",
                            description: "Show bot commands",
                        },
                    ],
                },
            )
            .await?;
        Ok(())
    }

    async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let _: serde_json::Value = self
            .call(
                "sendMessage",
                &SendMessageRequest {
                    chat_id,
                    text,
                    disable_web_page_preview: true,
                },
            )
            .await?;
        Ok(())
    }

    async fn call<TRequest, TResponse>(&self, method: &str, body: &TRequest) -> Result<TResponse>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        let response = self
            .client
            .post(format!("{}/{method}", self.api_prefix))
            .json(body)
            .send()
            .await
            .map_err(|error| sanitized_transport_error(method, &error))?;
        let status = response.status();
        let envelope: TelegramEnvelope<TResponse> = response
            .json()
            .await
            .map_err(|error| sanitized_transport_error(method, &error))?;
        if !status.is_success() || !envelope.ok {
            let description = envelope
                .description
                .as_deref()
                .unwrap_or("Telegram rejected the request");
            bail!("Telegram {method} failed with HTTP {status}: {description}");
        }
        envelope
            .result
            .with_context(|| format!("Telegram {method} returned no result"))
    }
}

fn parse_command<'a>(text: &'a str, bot_username: &str) -> Option<ParsedCommand<'a>> {
    let text = text.trim();
    let mut parts = text.splitn(2, char::is_whitespace);
    let command = parts.next()?.strip_prefix('/')?;
    let argument = parts.next().unwrap_or("").trim();
    let (name, target) = match command.split_once('@') {
        Some((name, target)) => (name, Some(target)),
        None => (command, None),
    };
    if name.is_empty() || target.is_some_and(|target| !target.eq_ignore_ascii_case(bot_username)) {
        return None;
    }
    Some(ParsedCommand { name, argument })
}

fn format_policy_list(policies: &[PolicySummary]) -> String {
    if policies.is_empty() {
        return "No wallets are linked to this Telegram chat. Start from a Stargazer wallet report to create a setup link.".to_owned();
    }
    let mut output = format!("Wallet policies ({})", policies.len());
    for policy in policies.iter().take(MAX_POLICIES_IN_MESSAGE) {
        let severities = match (policy.critical_alerts, policy.non_critical_alerts) {
            (true, true) => "critical + non-critical",
            (true, false) => "critical",
            (false, true) => "non-critical",
            (false, false) => "none",
        };
        let state = match policy.state {
            PolicyState::Active => "pilot entitled; delivery worker offline",
            PolicyState::AwaitingEntitlement => "awaiting entitlement",
            PolicyState::Paused => "paused",
            PolicyState::PendingTelegram => "pending Telegram",
        };
        output.push_str(&format!(
            "\n\n{}\n{} · {}",
            policy.wallet_address, severities, state
        ));
    }
    if policies.len() > MAX_POLICIES_IN_MESSAGE {
        output.push_str(&format!(
            "\n\n{} more policies are omitted from this Telegram view.",
            policies.len() - MAX_POLICIES_IN_MESSAGE
        ));
    }
    output
}

fn help_message() -> &'static str {
    "Stargazer alert commands\n\n/start <setup token> — link a wallet from a 15-minute setup URL\n/list — list linked wallets and policy state\n/settings — review settings and find the web configuration\n/unwatch G... — pause one wallet policy\n/help — show this message"
}

fn normalize_public_app_url(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().trim_end_matches('/');
    if value.is_empty() {
        return Ok(None);
    }
    let parsed = reqwest::Url::parse(value).context("parsing public Stargazer app URL")?;
    if !matches!(parsed.scheme(), "https" | "http") {
        bail!("public Stargazer app URL must use http or https");
    }
    Ok(Some(value.to_owned()))
}

fn validate_bot_token(value: &str) -> Result<()> {
    let (id, secret) = value
        .split_once(':')
        .context("Telegram bot token must have the '<bot id>:<secret>' form")?;
    if id.is_empty()
        || !id.bytes().all(|byte| byte.is_ascii_digit())
        || secret.len() < 20
        || !secret
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        bail!("Telegram bot token has an invalid format");
    }
    Ok(())
}

fn sanitized_transport_error(method: &str, error: &reqwest::Error) -> anyhow::Error {
    let category = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connection"
    } else if error.is_decode() {
        "response decode"
    } else {
        "transport"
    };
    anyhow::anyhow!("Telegram {method} {category} error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_parser_accepts_private_and_addressed_commands() {
        assert_eq!(
            parse_command("/start abc", "stargazer_bot"),
            Some(ParsedCommand {
                name: "start",
                argument: "abc"
            })
        );
        assert_eq!(
            parse_command(" /list@Stargazer_Bot ", "stargazer_bot"),
            Some(ParsedCommand {
                name: "list",
                argument: ""
            })
        );
        assert_eq!(parse_command("/list@another_bot", "stargazer_bot"), None);
        assert_eq!(parse_command("ordinary message", "stargazer_bot"), None);
    }

    #[test]
    fn token_validation_rejects_path_injection_and_placeholders() {
        assert!(validate_bot_token("123456:abcdefghijklmnopqrstuvwxyz_ABC-123").is_ok());
        assert!(validate_bot_token("replace-me").is_err());
        assert!(validate_bot_token("123:../../getMe").is_err());
    }

    #[test]
    fn policy_list_is_bounded_for_telegram_message_size() {
        let policies = (0..20)
            .map(|index| PolicySummary {
                wallet_address: format!("G{index:055}"),
                critical_alerts: true,
                non_critical_alerts: true,
                state: PolicyState::Active,
            })
            .collect::<Vec<_>>();
        let message = format_policy_list(&policies);
        assert!(message.contains("5 more policies"));
        assert!(message.len() < 4096);
    }
}
