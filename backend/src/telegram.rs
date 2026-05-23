use crate::{
    context::SharedState,
    delivery,
    models::{TelegramBotProfile, TelegramEventScope},
    protocols,
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

const HELP_MESSAGE: &str = "FireWatch commands:\n/help - show this message\n/info - show FireWatch live status for the indexer\n/list - list FireWatch rules in this chat\n/watch - open quick watch actions\n/watch all - subscribe this chat to all program updates and curated DeFi signals\n/watch defi - subscribe this chat to the focused DeFi stream\n/watch protocol <slug> - subscribe this chat to one curated DeFi protocol\n/watch program <address> - subscribe this chat to one program's upgrades and IDL changes\n/watch multisig <address> - subscribe this chat to one Squads multisig\n/unwatch - open quick remove actions\n/unwatch all - remove all FireWatch rules from this chat\n/unwatch defi - remove only the focused DeFi rule from this chat\n/unwatch protocol <slug> - remove one protocol rule from this chat\n/unwatch program <address> - remove one program rule from this chat\n/unwatch multisig <address> - remove one multisig rule from this chat\n/cancel - close the current action\n\nAlerts only apply to new matching events after the command is accepted.";
const WATCH_MENU_MESSAGE: &str = "Choose what to watch in this chat:";
const UNWATCH_MENU_MESSAGE: &str = "Choose what to remove from this chat:";

#[derive(Deserialize)]
struct TelegramApiEnvelope<T> {
    ok: bool,
    result: T,
    description: Option<String>,
}

#[derive(Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
    channel_post: Option<TelegramMessage>,
    callback_query: Option<TelegramCallbackQuery>,
    my_chat_member: Option<TelegramChatMemberUpdated>,
}

#[derive(Deserialize)]
struct TelegramMessage {
    message_id: i64,
    chat: TelegramChat,
    text: Option<String>,
    message_thread_id: Option<i64>,
}

#[derive(Deserialize)]
struct TelegramChat {
    id: i64,
    title: Option<String>,
}

#[derive(Deserialize)]
struct TelegramChatMemberUpdated {
    chat: TelegramChat,
    old_chat_member: TelegramChatMember,
    new_chat_member: TelegramChatMember,
}

#[derive(Deserialize)]
struct TelegramCallbackQuery {
    id: String,
    data: Option<String>,
    message: Option<TelegramMessage>,
}

#[derive(Deserialize)]
struct TelegramChatMember {
    status: String,
}

#[derive(Deserialize)]
struct TelegramBotUser {
    id: i64,
    first_name: String,
    username: Option<String>,
}

#[derive(Serialize)]
struct TelegramGetUpdatesRequest {
    offset: i64,
    timeout: i64,
    allowed_updates: Vec<&'static str>,
}

#[derive(Serialize)]
struct TelegramSetMyCommandsRequest {
    commands: Vec<TelegramBotCommand>,
}

#[derive(Serialize)]
struct TelegramBotCommand {
    command: &'static str,
    description: &'static str,
}

#[derive(Serialize)]
struct TelegramSendMessageRequest<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_markup: Option<TelegramInlineKeyboardMarkup>,
}

#[derive(Serialize)]
struct TelegramInlineKeyboardMarkup {
    inline_keyboard: Vec<Vec<TelegramInlineKeyboardButton>>,
}

#[derive(Serialize)]
struct TelegramInlineKeyboardButton {
    text: &'static str,
    callback_data: &'static str,
}

#[derive(Serialize)]
struct TelegramAnswerCallbackQueryRequest<'a> {
    callback_query_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<&'a str>,
}

#[derive(Serialize)]
struct TelegramEditMessageTextRequest<'a> {
    chat_id: &'a str,
    message_id: i64,
    text: &'a str,
    disable_web_page_preview: bool,
}

pub async fn run_command_loop(state: SharedState) -> Result<()> {
    if state.config.telegram_bot_token.is_none() {
        info!("telegram bot token not configured, command loop disabled");
        loop {
            sleep(Duration::from_secs(3600)).await;
        }
    }

    loop {
        if let Err(error) = register_bot_commands(&state).await {
            warn!(error = %error, "failed to register telegram bot commands");
        }

        match poll_updates_forever(state.clone()).await {
            Ok(()) => warn!("telegram command loop ended unexpectedly"),
            Err(error) => warn!(error = %error, "telegram command loop disconnected"),
        }

        sleep(Duration::from_secs(state.config.reconnect_delay_seconds)).await;
    }
}

pub async fn bot_profile(state: &SharedState) -> Result<Option<TelegramBotProfile>> {
    if state.config.telegram_bot_token.is_none() {
        return Ok(None);
    }

    if let Some(cached) = state.telegram_bot_profile.read().await.clone() {
        return Ok(cached);
    }

    let response = state
        .http_client
        .get(telegram_method_url(state, "getMe")?)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!("telegram getMe returned {status}: {}", text.trim()));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<TelegramBotUser>>(&text)?;
    let profile = if parsed.ok {
        Some(TelegramBotProfile {
            id: parsed.result.id,
            first_name: parsed.result.first_name,
            username: parsed.result.username,
        })
    } else {
        return Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram getMe failed".to_owned())
        ));
    };

    *state.telegram_bot_profile.write().await = Some(profile.clone());
    Ok(profile)
}

async fn poll_updates_forever(state: SharedState) -> Result<()> {
    let mut next_update_id = state.db.telegram_last_update_id().await?.unwrap_or(0);

    loop {
        let updates = fetch_updates(&state, next_update_id).await?;

        for update in updates {
            let advance_to = update.update_id + 1;

            if let Err(error) = handle_update(&state, &update).await {
                warn!(update_id = update.update_id, error = %error, "failed to handle telegram update");
            }

            state.db.set_telegram_last_update_id(advance_to).await?;
            next_update_id = advance_to;
        }
    }
}

async fn fetch_updates(state: &SharedState, offset: i64) -> Result<Vec<TelegramUpdate>> {
    let body = TelegramGetUpdatesRequest {
        offset,
        timeout: 25,
        allowed_updates: vec![
            "message",
            "channel_post",
            "callback_query",
            "my_chat_member",
        ],
    };
    let response = state
        .http_client
        .post(telegram_method_url(state, "getUpdates")?)
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "telegram getUpdates returned {status}: {}",
            text.trim()
        ));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<Vec<TelegramUpdate>>>(&text)?;
    if parsed.ok {
        Ok(parsed.result)
    } else {
        Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram getUpdates failed".to_owned())
        ))
    }
}

async fn handle_update(state: &SharedState, update: &TelegramUpdate) -> Result<()> {
    if let Some(chat_member_update) = &update.my_chat_member {
        handle_chat_member_update(state, chat_member_update).await?;
    }

    if let Some(message) = &update.message {
        handle_message(state, message).await?;
    }

    if let Some(message) = &update.channel_post {
        handle_message(state, message).await?;
    }

    if let Some(callback_query) = &update.callback_query {
        handle_callback_query(state, callback_query).await?;
    }

    Ok(())
}

async fn handle_chat_member_update(
    state: &SharedState,
    update: &TelegramChatMemberUpdated,
) -> Result<()> {
    if became_active_member(
        &update.old_chat_member.status,
        &update.new_chat_member.status,
    ) {
        send_help_menu(state, &update.chat.id.to_string(), None).await?;
    }

    Ok(())
}

async fn handle_message(state: &SharedState, message: &TelegramMessage) -> Result<()> {
    let Some(text) = message.text.as_deref().map(str::trim) else {
        return Ok(());
    };

    if !text.starts_with('/') {
        return Ok(());
    }

    let mut parts = text.split_whitespace();
    let command = parts.next().unwrap_or_default();
    let args = parts.collect::<Vec<_>>().join(" ");
    let normalized_command = normalize_command(command);

    match normalized_command.as_str() {
        "start" | "help" => {
            send_help_menu(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
            )
            .await?;
        }
        "info" => {
            let reply = build_info_message(state, message).await?;
            delivery::send_telegram_text(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
                &reply,
            )
            .await?;
        }
        "list" => {
            let reply = build_list_message(state, message).await?;
            delivery::send_telegram_text(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
                &reply,
            )
            .await?;
        }
        "watch" if args.is_empty() => {
            send_watch_menu(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
            )
            .await?;
        }
        "watch" if args.eq_ignore_ascii_case("all") => {
            handle_watch_all(state, message).await?;
        }
        "watch" if args.eq_ignore_ascii_case("defi") => {
            handle_watch_defi(state, message).await?;
        }
        "watch" if args.to_ascii_lowercase().starts_with("protocol ") => {
            handle_watch_protocol(state, message, args["protocol ".len()..].trim()).await?;
        }
        "watch" if args.to_ascii_lowercase().starts_with("program ") => {
            handle_watch_program(state, message, args["program ".len()..].trim()).await?;
        }
        "watch" if args.to_ascii_lowercase().starts_with("multisig ") => {
            handle_watch_multisig(state, message, args["multisig ".len()..].trim()).await?;
        }
        "watch_all" => {
            handle_watch_all(state, message).await?;
        }
        "watch_defi" => {
            handle_watch_defi(state, message).await?;
        }
        "unwatch" if args.is_empty() => {
            send_unwatch_menu(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
            )
            .await?;
        }
        "unwatch" if args.eq_ignore_ascii_case("all") => {
            handle_unwatch_all(state, message).await?;
        }
        "unwatch" if args.eq_ignore_ascii_case("defi") => {
            handle_unwatch_defi(state, message).await?;
        }
        "unwatch" if args.to_ascii_lowercase().starts_with("protocol ") => {
            handle_unwatch_protocol(state, message, args["protocol ".len()..].trim()).await?;
        }
        "unwatch" if args.to_ascii_lowercase().starts_with("program ") => {
            handle_unwatch_program(state, message, args["program ".len()..].trim()).await?;
        }
        "unwatch" if args.to_ascii_lowercase().starts_with("multisig ") => {
            handle_unwatch_multisig(state, message, args["multisig ".len()..].trim()).await?;
        }
        "cancel" => {
            let reply = build_cancel_message(state, message).await?;
            delivery::send_telegram_text(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
                &reply,
            )
            .await?;
        }
        _ => {
            send_watch_menu_with_note(
                state,
                &message.chat.id.to_string(),
                message.message_thread_id,
                "Use the quick actions below, or send /help for the full command list.",
            )
            .await?;
        }
    }

    Ok(())
}

async fn handle_callback_query(
    state: &SharedState,
    callback_query: &TelegramCallbackQuery,
) -> Result<()> {
    let Some(data) = callback_query.data.as_deref() else {
        answer_callback_query(state, &callback_query.id, None).await?;
        return Ok(());
    };

    let Some(message) = callback_query.message.as_ref() else {
        answer_callback_query(
            state,
            &callback_query.id,
            Some("This action is no longer available."),
        )
        .await?;
        return Ok(());
    };

    match data {
        "watch:all" => {
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Subscribing this chat to broad coverage…"),
            )
            .await?;
            let reply = ensure_watch_all(state, message).await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        "watch:defi" => {
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Subscribing this chat to curated DeFi alerts…"),
            )
            .await?;
            let reply = ensure_watch_defi(state, message).await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        "watch:cancel" => {
            let reply = build_cancel_message(state, message).await?;
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Closed menu. Alerts stay active."),
            )
            .await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        "unwatch:all" => {
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Removing all FireWatch rules from this chat…"),
            )
            .await?;
            let reply = ensure_unwatch_all(state, message).await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        "unwatch:defi" => {
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Removing the focused DeFi rule from this chat…"),
            )
            .await?;
            let reply = ensure_unwatch_defi(state, message).await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        "unwatch:cancel" => {
            let reply = build_cancel_message(state, message).await?;
            answer_callback_query(
                state,
                &callback_query.id,
                Some("Closed menu. Alerts stay active."),
            )
            .await?;
            close_menu_message(
                state,
                &message.chat.id.to_string(),
                message.message_id,
                &reply,
            )
            .await?;
        }
        _ => {
            answer_callback_query(state, &callback_query.id, Some("Unknown action.")).await?;
        }
    }

    Ok(())
}

async fn handle_watch_all(state: &SharedState, message: &TelegramMessage) -> Result<()> {
    let reply = ensure_watch_all(state, message).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn ensure_watch_all(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let label_prefix = message
        .chat
        .title
        .as_deref()
        .unwrap_or("telegram chat")
        .to_owned();

    let (_, program_created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch all - program updates"),
            &chat_id,
            thread_id,
            TelegramEventScope::ProgramUpdates,
            None,
            &Vec::new(),
        )
        .await?;

    let (_, defi_created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch all - DeFi watch"),
            &chat_id,
            thread_id,
            TelegramEventScope::DefiWatch,
            None,
            &Vec::new(),
        )
        .await?;

    Ok(match (program_created, defi_created) {
        (true, true) => {
            "This chat is now subscribed to all new program updates and curated DeFi signals."
        }
        (false, false) => {
            "This chat was already subscribed to program updates and curated DeFi signals."
        }
        _ => "This chat is now subscribed to program updates and curated DeFi signals.",
    }
    .to_owned())
}

async fn handle_watch_defi(state: &SharedState, message: &TelegramMessage) -> Result<()> {
    let reply = ensure_watch_defi(state, message).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_watch_protocol(
    state: &SharedState,
    message: &TelegramMessage,
    slug_or_name: &str,
) -> Result<()> {
    let reply = ensure_watch_protocol(state, message, slug_or_name).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_watch_program(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<()> {
    let reply = ensure_watch_program(state, message, address).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_watch_multisig(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<()> {
    let reply = ensure_watch_multisig(state, message, address).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_unwatch_all(state: &SharedState, message: &TelegramMessage) -> Result<()> {
    let reply = ensure_unwatch_all(state, message).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_unwatch_defi(state: &SharedState, message: &TelegramMessage) -> Result<()> {
    let reply = ensure_unwatch_defi(state, message).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_unwatch_protocol(
    state: &SharedState,
    message: &TelegramMessage,
    slug_or_name: &str,
) -> Result<()> {
    let reply = ensure_unwatch_protocol(state, message, slug_or_name).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_unwatch_program(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<()> {
    let reply = ensure_unwatch_program(state, message, address).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn handle_unwatch_multisig(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<()> {
    let reply = ensure_unwatch_multisig(state, message, address).await?;
    delivery::send_telegram_text(
        state,
        &message.chat.id.to_string(),
        message.message_thread_id,
        &reply,
    )
    .await?;

    Ok(())
}

async fn ensure_watch_defi(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let label_prefix = message
        .chat
        .title
        .as_deref()
        .unwrap_or("telegram chat")
        .to_owned();

    let (_, created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch defi"),
            &chat_id,
            thread_id,
            TelegramEventScope::DefiWatch,
            None,
            &Vec::new(),
        )
        .await?;

    let broad_rule_count = count_non_defi_subscriptions_for_chat(state, message).await?;
    let mut reply = if created {
        "This chat is now subscribed to high-signal DeFi alerts: program upgrades, unknown instructions, metric anomalies, and high-impact admin changes."
    } else {
        "This chat was already subscribed to the focused DeFi alert stream."
    }
    .to_owned();

    if broad_rule_count > 0 {
        reply.push_str(&format!(
            "\n\nThis chat still has {broad_rule_count} broader FireWatch rule{} from /watch all. Use /unwatch if you want only the focused DeFi stream.",
            if broad_rule_count == 1 { "" } else { "s" }
        ));
    }

    Ok(reply)
}

async fn ensure_watch_protocol(
    state: &SharedState,
    message: &TelegramMessage,
    slug_or_name: &str,
) -> Result<String> {
    let Some(protocol) = protocols::curated_protocol_descriptor(slug_or_name) else {
        return Ok("FireWatch does not know that protocol key yet. Use the website catalog to copy an exact /watch protocol command.".to_owned());
    };

    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let label_prefix = message
        .chat
        .title
        .as_deref()
        .unwrap_or("telegram chat")
        .to_owned();
    let required_tags = vec![format!("protocol:{}", protocol.slug)];

    let (_, created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch protocol - {}", protocol.name),
            &chat_id,
            thread_id,
            TelegramEventScope::DefiWatch,
            None,
            &required_tags,
        )
        .await?;

    Ok(if created {
        let mut scopes = Vec::new();
        if !protocol.programs.is_empty() {
            scopes.push(format!(
                "{} program{}",
                protocol.programs.len(),
                if protocol.programs.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        if !protocol.sol_domains.is_empty() {
            scopes.push(format!(
                "{} SNS domain{}",
                protocol.sol_domains.len(),
                if protocol.sol_domains.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        if !protocol.amount_triggers.is_empty() {
            scopes.push(format!(
                "{} amount trigger{}",
                protocol.amount_triggers.len(),
                if protocol.amount_triggers.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }
        let scope_summary = if scopes.is_empty() {
            "its curated alerts".to_owned()
        } else {
            scopes.join(", ")
        };
        format!(
            "This chat now tracks {} across {}.",
            protocol.name, scope_summary
        )
    } else {
        format!("This chat was already tracking {}.", protocol.name)
    })
}

async fn ensure_watch_program(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<String> {
    let normalized = normalize_pubkey(address).map_err(|error| anyhow!(error))?;
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let label_prefix = message
        .chat
        .title
        .as_deref()
        .unwrap_or("telegram chat")
        .to_owned();

    let (_, created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch program - {normalized}"),
            &chat_id,
            thread_id,
            TelegramEventScope::ProgramUpdates,
            Some(&normalized),
            &Vec::new(),
        )
        .await?;

    Ok(if created {
        format!("This chat now tracks program updates and IDL changes for {normalized}.")
    } else {
        format!("This chat was already tracking {normalized}.")
    })
}

async fn ensure_watch_multisig(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<String> {
    let normalized = normalize_pubkey(address).map_err(|error| anyhow!(error))?;
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let label_prefix = message
        .chat
        .title
        .as_deref()
        .unwrap_or("telegram chat")
        .to_owned();

    let (_, created) = state
        .db
        .ensure_telegram_subscription(
            &format!("{label_prefix} /watch multisig - {normalized}"),
            &chat_id,
            thread_id,
            TelegramEventScope::MultisigActivity,
            Some(&normalized),
            &Vec::new(),
        )
        .await?;

    Ok(if created {
        format!("This chat now tracks Squads activity for {normalized}.")
    } else {
        format!("This chat was already tracking {normalized}.")
    })
}

async fn ensure_unwatch_all(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let deleted = delete_chat_subscriptions(state, message, |_| true).await?;

    Ok(match deleted {
        0 => "This chat had no FireWatch rules to remove.".to_owned(),
        1 => "Removed 1 FireWatch rule from this chat.".to_owned(),
        count => format!("Removed {count} FireWatch rules from this chat."),
    })
}

async fn ensure_unwatch_defi(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let deleted = delete_chat_subscriptions(state, message, |subscription| {
        subscription.scope == TelegramEventScope::DefiWatch
    })
    .await?;

    Ok(match deleted {
        0 => "This chat had no focused DeFi rule to remove.".to_owned(),
        1 => "Removed the focused DeFi rule from this chat.".to_owned(),
        count => format!("Removed {count} focused DeFi rules from this chat."),
    })
}

async fn ensure_unwatch_protocol(
    state: &SharedState,
    message: &TelegramMessage,
    slug_or_name: &str,
) -> Result<String> {
    let Some(protocol) = protocols::curated_protocol_descriptor(slug_or_name) else {
        return Ok("FireWatch does not know that protocol key yet. Use the website catalog to copy an exact /unwatch protocol command.".to_owned());
    };

    let protocol_tag = format!("protocol:{}", protocol.slug);
    let deleted = delete_chat_subscriptions(state, message, |subscription| {
        subscription.scope == TelegramEventScope::DefiWatch
            && subscription
                .required_tags
                .iter()
                .any(|tag| tag == &protocol_tag)
    })
    .await?;

    Ok(match deleted {
        0 => format!(
            "This chat had no {} protocol rule to remove.",
            protocol.name
        ),
        1 => format!(
            "Removed the {} protocol rule from this chat.",
            protocol.name
        ),
        count => format!(
            "Removed {count} {} protocol rules from this chat.",
            protocol.name
        ),
    })
}

async fn ensure_unwatch_program(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<String> {
    let normalized = normalize_pubkey(address).map_err(|error| anyhow!(error))?;
    let deleted = delete_chat_subscriptions(state, message, |subscription| {
        subscription.scope == TelegramEventScope::ProgramUpdates
            && subscription.target_address.as_deref() == Some(normalized.as_str())
    })
    .await?;

    Ok(match deleted {
        0 => format!("This chat had no program rule for {normalized}."),
        1 => format!("Removed the program rule for {normalized}."),
        count => format!("Removed {count} program rules for {normalized}."),
    })
}

async fn ensure_unwatch_multisig(
    state: &SharedState,
    message: &TelegramMessage,
    address: &str,
) -> Result<String> {
    let normalized = normalize_pubkey(address).map_err(|error| anyhow!(error))?;
    let deleted = delete_chat_subscriptions(state, message, |subscription| {
        subscription.scope == TelegramEventScope::MultisigActivity
            && subscription.target_address.as_deref() == Some(normalized.as_str())
    })
    .await?;

    Ok(match deleted {
        0 => format!("This chat had no multisig rule for {normalized}."),
        1 => format!("Removed the multisig rule for {normalized}."),
        count => format!("Removed {count} multisig rules for {normalized}."),
    })
}

async fn build_info_message(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let summary = state.db.summary().await?;
    let subscriptions = state.db.list_telegram_subscriptions().await?;
    let current_chat_id = message.chat.id.to_string();
    let current_thread_id = message.message_thread_id;
    let current_chat_subscriptions = subscriptions
        .iter()
        .filter(|subscription| {
            subscription.chat_id == current_chat_id && subscription.thread_id == current_thread_id
        })
        .count();

    let mut lines = vec!["FireWatch status".to_owned()];
    lines.push(format!(
        "Last seen slot: {}",
        summary
            .last_processed_slot
            .map(|slot| slot.to_string())
            .unwrap_or_else(|| "n/a".to_owned())
    ));
    lines.push(format!(
        "Transactions parsed: {}",
        summary.processed_transaction_count
    ));
    lines.push(format!(
        "Transactions matched: {}",
        summary.matched_transaction_count
    ));
    lines.push(format!("Indexed events: {}", summary.total_count));
    lines.push(format!(
        "Pending retries: {}",
        summary.pending_delivery_count
    ));
    lines.push(format!(
        "Rules in this chat: {}",
        current_chat_subscriptions
    ));

    if let Some(profile) = bot_profile(state).await? {
        if let Some(username) = profile.username {
            lines.push(format!("Bot: {} (@{})", profile.first_name, username));
        } else {
            lines.push(format!("Bot: {}", profile.first_name));
        }
    }

    Ok(lines.join("\n"))
}

async fn build_list_message(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let mut subscriptions = state
        .db
        .list_telegram_subscriptions()
        .await?
        .into_iter()
        .filter(|subscription| {
            subscription.chat_id == chat_id && subscription.thread_id == thread_id
        })
        .collect::<Vec<_>>();

    if subscriptions.is_empty() {
        return Ok("This chat has no active FireWatch rules right now.".to_owned());
    }

    subscriptions.sort_by_key(|subscription| subscription.created_at);

    let mut lines = vec!["FireWatch rules in this chat".to_owned()];
    for (index, subscription) in subscriptions.iter().enumerate() {
        lines.push(format!(
            "{}. {}",
            index + 1,
            describe_subscription(subscription)
        ));
    }
    lines.push(
        "Use /unwatch to remove broad rules, or send a specific /unwatch command for one target."
            .to_owned(),
    );

    Ok(lines.join("\n"))
}

async fn build_cancel_message(state: &SharedState, message: &TelegramMessage) -> Result<String> {
    let active_rules = list_chat_scope_labels(state, message).await?;

    if active_rules.is_empty() {
        return Ok(
            "Closed the menu. This chat has no active FireWatch alerts right now.".to_owned(),
        );
    }

    let label_list = active_rules.join(", ");
    let rule_count = active_rules.len();

    Ok(format!(
        "Closed the menu. This chat still has {rule_count} active alert rule{}: {label_list}.\nUse /unwatch to remove them.",
        if rule_count == 1 { "" } else { "s" }
    ))
}

async fn register_bot_commands(state: &SharedState) -> Result<()> {
    let body = TelegramSetMyCommandsRequest {
        commands: vec![
            TelegramBotCommand {
                command: "help",
                description: "Show FireWatch commands",
            },
            TelegramBotCommand {
                command: "watch",
                description: "Open quick watch actions",
            },
            TelegramBotCommand {
                command: "list",
                description: "List FireWatch rules in this chat",
            },
            TelegramBotCommand {
                command: "info",
                description: "Show FireWatch live status",
            },
            TelegramBotCommand {
                command: "watch_all",
                description: "Subscribe this chat to broad coverage",
            },
            TelegramBotCommand {
                command: "watch_defi",
                description: "Subscribe this chat to curated DeFi watch",
            },
            TelegramBotCommand {
                command: "unwatch",
                description: "Open quick remove actions",
            },
            TelegramBotCommand {
                command: "cancel",
                description: "Close the current quick action",
            },
        ],
    };
    let response = state
        .http_client
        .post(telegram_method_url(state, "setMyCommands")?)
        .json(&body)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "telegram setMyCommands returned {status}: {}",
            text.trim()
        ));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<bool>>(&text)?;
    if parsed.ok {
        Ok(())
    } else {
        Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram setMyCommands failed".to_owned())
        ))
    }
}

fn telegram_method_url(state: &SharedState, method: &str) -> Result<String> {
    let bot_token = state
        .config
        .telegram_bot_token
        .as_deref()
        .ok_or_else(|| anyhow!("telegram bot token is not configured"))?;

    Ok(format!(
        "{}/bot{bot_token}/{method}",
        state.config.telegram_api_base.trim_end_matches('/')
    ))
}

fn normalize_command(raw: &str) -> String {
    raw.trim_start_matches('/')
        .split('@')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn normalize_pubkey(raw: &str) -> std::result::Result<String, &'static str> {
    let value = raw.trim();
    if value.is_empty() {
        return Err("A Solana address is required.");
    }

    Pubkey::from_str(value)
        .map(|pubkey| pubkey.to_string())
        .map_err(|_| "That is not a valid Solana address.")
}

fn became_active_member(old_status: &str, new_status: &str) -> bool {
    !is_active_member(old_status) && is_active_member(new_status)
}

fn is_active_member(status: &str) -> bool {
    matches!(status, "member" | "administrator" | "creator")
}

async fn send_help_menu(state: &SharedState, chat_id: &str, thread_id: Option<i64>) -> Result<()> {
    send_message(
        state,
        chat_id,
        thread_id,
        HELP_MESSAGE,
        Some(watch_menu_markup()),
    )
    .await
}

async fn send_watch_menu(state: &SharedState, chat_id: &str, thread_id: Option<i64>) -> Result<()> {
    send_watch_menu_with_note(state, chat_id, thread_id, WATCH_MENU_MESSAGE).await
}

async fn send_unwatch_menu(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
) -> Result<()> {
    let text = format!(
        "{UNWATCH_MENU_MESSAGE}\n\nUse this when you want to move from broad /watch all mode to a quieter focused setup."
    );
    send_message(
        state,
        chat_id,
        thread_id,
        &text,
        Some(unwatch_menu_markup()),
    )
    .await
}

async fn send_watch_menu_with_note(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    note: &str,
) -> Result<()> {
    let text = format!("{WATCH_MENU_MESSAGE}\n\n{note}");
    send_message(state, chat_id, thread_id, &text, Some(watch_menu_markup())).await
}

async fn send_message(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    text: &str,
    reply_markup: Option<TelegramInlineKeyboardMarkup>,
) -> Result<()> {
    let response = state
        .http_client
        .post(telegram_method_url(state, "sendMessage")?)
        .json(&TelegramSendMessageRequest {
            chat_id,
            text,
            disable_web_page_preview: true,
            message_thread_id: thread_id,
            reply_markup,
        })
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "telegram sendMessage returned {status}: {}",
            body.trim()
        ));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<serde_json::Value>>(&body)?;
    if parsed.ok {
        Ok(())
    } else {
        Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram sendMessage failed".to_owned())
        ))
    }
}

async fn answer_callback_query(
    state: &SharedState,
    callback_query_id: &str,
    text: Option<&str>,
) -> Result<()> {
    let response = state
        .http_client
        .post(telegram_method_url(state, "answerCallbackQuery")?)
        .json(&TelegramAnswerCallbackQueryRequest {
            callback_query_id,
            text,
        })
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "telegram answerCallbackQuery returned {status}: {}",
            body.trim()
        ));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<bool>>(&body)?;
    if parsed.ok {
        Ok(())
    } else {
        Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram answerCallbackQuery failed".to_owned())
        ))
    }
}

async fn close_menu_message(
    state: &SharedState,
    chat_id: &str,
    message_id: i64,
    text: &str,
) -> Result<()> {
    let response = state
        .http_client
        .post(telegram_method_url(state, "editMessageText")?)
        .json(&TelegramEditMessageTextRequest {
            chat_id,
            message_id,
            text,
            disable_web_page_preview: true,
        })
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        return Err(anyhow!(
            "telegram editMessageText returned {status}: {}",
            body.trim()
        ));
    }

    let parsed = serde_json::from_str::<TelegramApiEnvelope<serde_json::Value>>(&body)?;
    if parsed.ok {
        Ok(())
    } else {
        Err(anyhow!(
            "{}",
            parsed
                .description
                .unwrap_or_else(|| "telegram editMessageText failed".to_owned())
        ))
    }
}

fn watch_menu_markup() -> TelegramInlineKeyboardMarkup {
    TelegramInlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                TelegramInlineKeyboardButton {
                    text: "Watch all",
                    callback_data: "watch:all",
                },
                TelegramInlineKeyboardButton {
                    text: "Watch DeFi",
                    callback_data: "watch:defi",
                },
            ],
            vec![TelegramInlineKeyboardButton {
                text: "Remove alerts",
                callback_data: "unwatch:all",
            }],
            vec![TelegramInlineKeyboardButton {
                text: "Close menu",
                callback_data: "watch:cancel",
            }],
        ],
    }
}

fn unwatch_menu_markup() -> TelegramInlineKeyboardMarkup {
    TelegramInlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![
                TelegramInlineKeyboardButton {
                    text: "Unwatch all",
                    callback_data: "unwatch:all",
                },
                TelegramInlineKeyboardButton {
                    text: "Unwatch DeFi",
                    callback_data: "unwatch:defi",
                },
            ],
            vec![TelegramInlineKeyboardButton {
                text: "Close menu",
                callback_data: "unwatch:cancel",
            }],
        ],
    }
}

async fn delete_chat_subscriptions<F>(
    state: &SharedState,
    message: &TelegramMessage,
    mut predicate: F,
) -> Result<usize>
where
    F: FnMut(&crate::models::TelegramSubscription) -> bool,
{
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let subscriptions = state.db.list_telegram_subscriptions().await?;
    let matching_ids = subscriptions
        .into_iter()
        .filter(|subscription| {
            subscription.chat_id == chat_id
                && subscription.thread_id == thread_id
                && predicate(subscription)
        })
        .map(|subscription| subscription.id)
        .collect::<Vec<_>>();

    let mut deleted = 0_usize;
    for subscription_id in matching_ids {
        if state
            .db
            .delete_telegram_subscription(&subscription_id)
            .await?
        {
            deleted += 1;
        }
    }

    Ok(deleted)
}

async fn list_chat_scope_labels(
    state: &SharedState,
    message: &TelegramMessage,
) -> Result<Vec<&'static str>> {
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let subscriptions = state.db.list_telegram_subscriptions().await?;

    let mut labels = subscriptions
        .into_iter()
        .filter(|subscription| {
            subscription.chat_id == chat_id && subscription.thread_id == thread_id
        })
        .map(|subscription| scope_label(subscription.scope))
        .collect::<Vec<_>>();

    labels.sort_unstable();
    labels.dedup();

    Ok(labels)
}

async fn count_non_defi_subscriptions_for_chat(
    state: &SharedState,
    message: &TelegramMessage,
) -> Result<usize> {
    let chat_id = message.chat.id.to_string();
    let thread_id = message.message_thread_id;
    let subscriptions = state.db.list_telegram_subscriptions().await?;

    Ok(subscriptions
        .into_iter()
        .filter(|subscription| {
            subscription.chat_id == chat_id
                && subscription.thread_id == thread_id
                && subscription.scope != TelegramEventScope::DefiWatch
        })
        .count())
}

fn scope_label(scope: TelegramEventScope) -> &'static str {
    match scope {
        TelegramEventScope::ProgramUpdates => "program updates",
        TelegramEventScope::MultisigActivity => "multisig activity",
        TelegramEventScope::DefiWatch => "DeFi watch",
    }
}

fn describe_subscription(subscription: &crate::models::TelegramSubscription) -> String {
    match subscription.scope {
        TelegramEventScope::DefiWatch => {
            if let Some(protocol_slug) = subscription
                .required_tags
                .iter()
                .find_map(|tag| tag.strip_prefix("protocol:"))
            {
                if let Some(protocol) = protocols::curated_protocol_descriptor(protocol_slug) {
                    return format!("Protocol watch: {}", protocol.name);
                }

                return format!("Protocol watch: {protocol_slug}");
            }

            "Focused DeFi watch".to_owned()
        }
        TelegramEventScope::ProgramUpdates => {
            if let Some(address) = subscription.target_address.as_deref() {
                return format!("Program watch: {address}");
            }

            "All program updates".to_owned()
        }
        TelegramEventScope::MultisigActivity => {
            if let Some(address) = subscription.target_address.as_deref() {
                return format!("Multisig watch: {address}");
            }

            "All multisig activity".to_owned()
        }
    }
}
