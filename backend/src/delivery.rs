use crate::{
    config::Config,
    context::SharedState,
    models::{
        ChainEvent, PendingTelegramDailySummary, PendingTelegramDelivery, TelegramDeliveryMode,
        TelegramEventScope, event_kind, now_ts,
    },
    name_service, protocols,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::Duration,
};
use tokio::time::sleep;
use tracing::{debug, warn};

const TELEGRAM_API_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Deserialize)]
struct TelegramApiResponse {
    ok: bool,
    description: Option<String>,
}

#[derive(Serialize)]
struct TelegramSendMessageRequest<'a> {
    chat_id: &'a str,
    text: &'a str,
    disable_web_page_preview: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link_preview_options: Option<TelegramLinkPreviewOptions<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
}

#[derive(Serialize)]
struct TelegramLinkPreviewOptions<'a> {
    is_disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    prefer_large_media: bool,
}

const TELEGRAM_DIGEST_LINE_LIMIT: usize = 6;

pub async fn run_retry_loop(state: SharedState) -> Result<()> {
    loop {
        let pending = state.db.pending_telegram_deliveries(now_ts(), 50).await?;
        if !pending.is_empty() {
            debug!(
                count = pending.len(),
                "retrying pending telegram deliveries"
            );
        }

        for delivery in pending {
            attempt_telegram_delivery(state.clone(), delivery).await;
        }

        sleep(Duration::from_secs(state.config.delivery_poll_seconds)).await;
    }
}

pub async fn run_daily_summary_loop(state: SharedState) -> Result<()> {
    loop {
        let now = now_ts();
        let latest_summary_day =
            latest_due_summary_day(now, state.config.telegram_daily_summary_hour_utc);
        let pending = state
            .db
            .pending_telegram_daily_summaries(now, latest_summary_day, 20, 12)
            .await?;
        if !pending.is_empty() {
            debug!(
                count = pending.len(),
                latest_summary_day, "sending pending telegram daily summaries"
            );
        }

        for summary in pending {
            attempt_telegram_daily_summary_delivery(state.clone(), summary).await;
        }

        sleep(Duration::from_secs(
            state.config.telegram_daily_summary_poll_seconds,
        ))
        .await;
    }
}

pub async fn enqueue_telegram_deliveries(state: SharedState, event: &ChainEvent) -> Result<()> {
    let mut matching_subscriptions = state.db.matching_telegram_subscriptions(event).await?;
    let auto_enrolled = auto_enroll_paid_wallet_protocol_subscriptions(
        state.clone(),
        event,
        &matching_subscriptions,
    )
    .await?;
    if auto_enrolled > 0 {
        matching_subscriptions = state.db.matching_telegram_subscriptions(event).await?;
    }
    let initial_next_retry_at = now_ts() + state.config.delivery_poll_seconds as i64;

    for subscription in matching_subscriptions {
        match subscription.delivery_mode {
            TelegramDeliveryMode::Live => {
                if let Some(delivery) = state
                    .db
                    .enqueue_telegram_delivery(event, &subscription, initial_next_retry_at)
                    .await?
                {
                    tokio::spawn(attempt_telegram_delivery(state.clone(), delivery));
                }
            }
            TelegramDeliveryMode::DailySummary => {
                state
                    .db
                    .enqueue_telegram_daily_summary_entry(
                        event,
                        &subscription,
                        initial_next_retry_at,
                    )
                    .await?;
            }
        }
    }

    Ok(())
}

async fn auto_enroll_paid_wallet_protocol_subscriptions(
    state: SharedState,
    event: &ChainEvent,
    matching_subscriptions: &[crate::models::TelegramSubscription],
) -> Result<usize> {
    let protocol_slugs = supported_protocol_slugs(event);
    if protocol_slugs.is_empty() {
        return Ok(0);
    }

    let mut entitlement_cache = HashMap::<String, bool>::new();
    let mut attempted_keys = HashSet::<(String, Option<i64>, String)>::new();
    let mut created_count = 0usize;

    for subscription in matching_subscriptions {
        if subscription.scope != TelegramEventScope::WalletActivity
            || subscription.status != "active"
        {
            continue;
        }

        let Some(user_id) = subscription.user_id.as_deref() else {
            continue;
        };

        let has_entitlement = if let Some(cached) = entitlement_cache.get(user_id) {
            *cached
        } else {
            let value = state
                .db
                .user_has_active_custom_entitlement(user_id, now_ts())
                .await?;
            entitlement_cache.insert(user_id.to_owned(), value);
            value
        };
        if !has_entitlement {
            continue;
        }

        for protocol_slug in &protocol_slugs {
            let dedupe_key = (
                subscription.chat_id.clone(),
                subscription.thread_id,
                protocol_slug.clone(),
            );
            if !attempted_keys.insert(dedupe_key) {
                continue;
            }

            let Some(protocol) = protocols::curated_protocol_descriptor(protocol_slug) else {
                continue;
            };

            let (created_subscription, created) = state
                .db
                .ensure_telegram_subscription(
                    &format!("Auto /watch protocol - {}", protocol.name),
                    Some(user_id),
                    &subscription.chat_id,
                    subscription.thread_id,
                    TelegramEventScope::DefiWatch,
                    None,
                    &[format!("protocol:{}", protocol.slug)],
                    TelegramDeliveryMode::Live,
                )
                .await?;

            if created {
                created_count += 1;
                debug!(
                    subscription_id = %created_subscription.id,
                    user_id,
                    chat_id = %subscription.chat_id,
                    thread_id = ?subscription.thread_id,
                    protocol = %protocol.slug,
                    event_id = %event.id,
                    "auto-enrolled paid wallet watch into protocol watch",
                );
            }
        }
    }

    Ok(created_count)
}

fn supported_protocol_slugs(event: &ChainEvent) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut slugs = Vec::new();
    let mut has_squads = false;

    let mut collect = |slug: &str| {
        let slug = slug.trim();
        if slug.is_empty() {
            return;
        }
        if slug == protocols::SQUADS_PROTOCOL_SLUG {
            has_squads = true;
            return;
        }
        if seen.insert(slug.to_owned()) && protocols::curated_protocol_descriptor(slug).is_some() {
            slugs.push(slug.to_owned());
        }
    };

    if let Some(linked_protocols) = event
        .payload
        .get("linked_protocols")
        .and_then(|value| value.as_array())
    {
        for linked_protocol in linked_protocols {
            if let Some(slug) = linked_protocol
                .get("protocol")
                .and_then(|value| value.as_str())
            {
                collect(slug);
            }
        }
    }

    for key in ["decoded_protocol", "protocol_slug", "protocol"] {
        if let Some(slug) = payload_str(event, key) {
            collect(slug);
        }
    }

    for tag in &event.tags {
        if let Some(slug) = tag.strip_prefix("protocol:") {
            collect(slug);
        }
    }

    if slugs.is_empty()
        && has_squads
        && protocols::curated_protocol_descriptor(protocols::SQUADS_PROTOCOL_SLUG).is_some()
    {
        slugs.push(protocols::SQUADS_PROTOCOL_SLUG.to_owned());
    }

    slugs
}

pub async fn attempt_telegram_delivery(state: SharedState, delivery: PendingTelegramDelivery) {
    if let Err(error) = deliver_telegram_event(&state, &delivery).await {
        warn!(
            delivery_id = delivery.id,
            subscription_id = delivery.subscription.id,
            error = %error,
            "failed to complete telegram delivery attempt",
        );
    }
}

pub async fn attempt_telegram_daily_summary_delivery(
    state: SharedState,
    summary: PendingTelegramDailySummary,
) {
    if let Err(error) = deliver_telegram_daily_summary(&state, &summary).await {
        warn!(
            subscription_id = summary.subscription.id,
            summary_day = summary.summary_day,
            error = %error,
            "failed to complete telegram daily summary delivery attempt",
        );
    }
}

async fn deliver_telegram_event(
    state: &SharedState,
    delivery: &PendingTelegramDelivery,
) -> Result<()> {
    let hydrated_event = name_service::hydrate_event(state, delivery.event.clone()).await;
    let preview_url = firewatch_event_url(&state.config, &hydrated_event);
    let message = format_telegram_message(&state.config, &hydrated_event);
    match send_telegram_html(
        state,
        &delivery.subscription.chat_id,
        delivery.subscription.thread_id,
        &message,
        &preview_url,
    )
    .await
    {
        Ok(()) => {
            state
                .db
                .mark_telegram_delivery_success(&delivery.id, now_ts())
                .await?;
        }
        Err(error) => {
            record_telegram_failure(state, delivery, error.to_string()).await?;
        }
    }

    Ok(())
}

async fn deliver_telegram_daily_summary(
    state: &SharedState,
    summary: &PendingTelegramDailySummary,
) -> Result<()> {
    let mut hydrated_events = Vec::with_capacity(summary.events.len());
    for event in &summary.events {
        hydrated_events.push(name_service::hydrate_event(state, event.clone()).await);
    }

    let preview_url = firewatch_events_url(&state.config);
    let message = format_telegram_daily_summary(&state.config, summary, &hydrated_events);
    match send_telegram_html(
        state,
        &summary.subscription.chat_id,
        summary.subscription.thread_id,
        &message,
        &preview_url,
    )
    .await
    {
        Ok(()) => {
            state
                .db
                .mark_telegram_daily_summary_success(
                    &summary.subscription.id,
                    summary.summary_day,
                    now_ts(),
                )
                .await?;
        }
        Err(error) => {
            record_telegram_daily_summary_failure(state, summary, error.to_string()).await?;
        }
    }

    Ok(())
}

pub async fn send_telegram_text(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    text: &str,
) -> Result<()> {
    send_telegram_message(state, chat_id, thread_id, text, None, true, None).await
}

async fn send_telegram_html(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    text: &str,
    preview_url: &str,
) -> Result<()> {
    send_telegram_message(
        state,
        chat_id,
        thread_id,
        text,
        Some("HTML"),
        false,
        Some(preview_url),
    )
    .await
}

async fn send_telegram_message(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    text: &str,
    parse_mode: Option<&str>,
    disable_web_page_preview: bool,
    link_preview_url: Option<&str>,
) -> Result<()> {
    let bot_token = state
        .config
        .telegram_bot_token
        .as_deref()
        .ok_or_else(|| anyhow!("telegram bot token is not configured"))?;
    let endpoint = format!(
        "{}/bot{}/sendMessage",
        state.config.telegram_api_base.trim_end_matches('/'),
        bot_token
    );
    let body = TelegramSendMessageRequest {
        chat_id,
        text,
        disable_web_page_preview,
        parse_mode,
        link_preview_options: link_preview_url.map(|url| TelegramLinkPreviewOptions {
            is_disabled: false,
            url: Some(url),
            prefer_large_media: true,
        }),
        message_thread_id: thread_id,
    };
    let response = state
        .http_client
        .post(endpoint)
        .json(&body)
        .timeout(TELEGRAM_API_TIMEOUT)
        .send()
        .await?;
    let status = response.status();
    let text = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(anyhow!("telegram returned {status}: {}", text.trim()));
    }

    let parsed =
        serde_json::from_str::<TelegramApiResponse>(&text).unwrap_or(TelegramApiResponse {
            ok: true,
            description: None,
        });

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

async fn record_telegram_failure(
    state: &SharedState,
    delivery: &PendingTelegramDelivery,
    message: String,
) -> Result<()> {
    let attempts = delivery.delivery_attempts + 1;
    let next_retry_at = now_ts() + retry_delay_seconds(attempts);

    state
        .db
        .mark_telegram_delivery_failure(&delivery.id, attempts, &message, next_retry_at)
        .await?;

    Ok(())
}

async fn record_telegram_daily_summary_failure(
    state: &SharedState,
    summary: &PendingTelegramDailySummary,
    message: String,
) -> Result<()> {
    let attempts = summary.delivery_attempts + 1;
    let next_retry_at = now_ts() + retry_delay_seconds(attempts);

    state
        .db
        .mark_telegram_daily_summary_failure(
            &summary.subscription.id,
            summary.summary_day,
            attempts,
            &message,
            next_retry_at,
        )
        .await?;

    Ok(())
}

fn retry_delay_seconds(attempts: i64) -> i64 {
    match attempts {
        0 | 1 => 5,
        2 => 15,
        3 => 30,
        4 => 60,
        5 => 120,
        _ => 300,
    }
}

fn format_telegram_message(config: &Config, event: &ChainEvent) -> String {
    let mut lines = vec![format!("<b>{}</b>", html_escape(event_title(event)))];

    let (digest_lines, preferred_prefixes): (Vec<String>, &[&str]) = match event.kind.as_str() {
        event_kind::NAME_SERVICE_DOMAIN_CREATED
        | event_kind::NAME_SERVICE_DOMAIN_UPDATED
        | event_kind::NAME_SERVICE_DOMAIN_TRANSFERRED
        | event_kind::NAME_SERVICE_DOMAIN_DELETED
        | event_kind::NAME_SERVICE_DOMAIN_REALLOCATED => {
            (domain_digest_lines(config, event), DOMAIN_DIGEST_PREFIXES)
        }
        event_kind::PROGRAM_DEPLOYED
        | event_kind::PROGRAM_UPGRADED
        | event_kind::PROGRAM_FINALIZED
        | event_kind::PROGRAM_MIGRATED_TO_LOADER_V4 => {
            (program_digest_lines(event), PROGRAM_DIGEST_PREFIXES)
        }
        event_kind::PROGRAM_IDL_INITIALIZED
        | event_kind::PROGRAM_IDL_UPDATED
        | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
        | event_kind::PROGRAM_IDL_IMMUTABLE
        | event_kind::PROGRAM_IDL_RESIZED
        | event_kind::PROGRAM_IDL_CLOSED => (idl_digest_lines(config, event), IDL_DIGEST_PREFIXES),
        event_kind::PROGRAM_ADMIN_INSTRUCTION
        | event_kind::PROGRAM_UNKNOWN_INSTRUCTION
        | event_kind::PROGRAM_METRIC_ALERT => (
            protocol_digest_lines(config, event),
            PROTOCOL_DIGEST_PREFIXES,
        ),
        event_kind::TREASURY_TOKEN_OUTFLOW => (
            treasury_outflow_digest_lines(event),
            TREASURY_OUTFLOW_DIGEST_PREFIXES,
        ),
        event_kind::TOKEN_LIQUIDITY_LOW => (
            token_liquidity_digest_lines(event),
            TOKEN_LIQUIDITY_DIGEST_PREFIXES,
        ),
        event_kind::MULTISIG_PROPOSAL_ACTIVATED
        | event_kind::MULTISIG_PROPOSAL_APPROVED
        | event_kind::MULTISIG_PROPOSAL_REJECTED
        | event_kind::MULTISIG_PROPOSAL_CANCELLED
        | event_kind::MULTISIG_VAULT_EXECUTED
        | event_kind::MULTISIG_CONFIG_EXECUTED => {
            (multisig_digest_lines(event), MULTISIG_DIGEST_PREFIXES)
        }
        _ => {
            let mut default_lines = vec![
                format!("Summary: {}", event.summary),
                format!("Target: {} ({})", event.target_address, event.target_type),
            ];
            if let Some(actor) = &event.actor {
                default_lines.push(format!("Actor: {actor}"));
            }
            (default_lines, DEFAULT_DIGEST_PREFIXES)
        }
    };

    lines.extend(
        selected_digest_lines(digest_lines, preferred_prefixes, TELEGRAM_DIGEST_LINE_LIMIT)
            .into_iter()
            .map(|line| render_telegram_line(&line)),
    );

    let mut slot_line = format!("Slot: <code>{}</code>", event.slot);
    if let Some(block_time) = format_block_time(event.block_time) {
        slot_line.push_str(" - ");
        slot_line.push_str(&html_escape(&block_time));
    }
    lines.push(slot_line);
    lines.push(format!(
        "Tx: <a href=\"{}\">{}</a>",
        html_attr_escape(&explorer_tx_url(config, &event.signature)),
        html_escape(&compact_identifier(&event.signature, 6, 6)),
    ));
    lines.push(format!(
        "<a href=\"{}\">Open in FireWatch</a>",
        html_attr_escape(&firewatch_event_url(config, event)),
    ));

    lines.join("\n")
}

fn format_telegram_daily_summary(
    config: &Config,
    summary: &PendingTelegramDailySummary,
    events: &[ChainEvent],
) -> String {
    let summary_date = DateTime::<Utc>::from_timestamp(summary.summary_day, 0)
        .map(|datetime| datetime.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown day".to_owned());
    let alert_label = if summary.event_count == 1 {
        "alert"
    } else {
        "alerts"
    };
    let mut lines = vec![
        "<b>FireWatch daily summary</b>".to_owned(),
        format!("Date: <code>{}</code>", html_escape(&summary_date)),
        format!("Rule: {}", html_escape(&summary.subscription.label)),
        format!(
            "Total: <b>{}</b> {} ({} critical, {} high, {} standard)",
            summary.event_count,
            alert_label,
            summary.critical_count,
            summary.high_count,
            summary.standard_count
        ),
    ];

    for event in events {
        lines.push(format_daily_summary_event_line(config, event));
    }

    let omitted_count = (summary.event_count as usize).saturating_sub(events.len());
    if omitted_count > 0 {
        let omitted_label = if omitted_count == 1 {
            "more alert"
        } else {
            "more alerts"
        };
        lines.push(format!(
            "... and {omitted_count} {omitted_label} in FireWatch."
        ));
    }

    lines.push(format!(
        "<a href=\"{}\">Open FireWatch events</a>",
        html_attr_escape(&firewatch_events_url(config)),
    ));

    lines.join("\n")
}

fn format_daily_summary_event_line(config: &Config, event: &ChainEvent) -> String {
    let severity = event_severity(event);
    let context = event_context_label(event)
        .map(|value| {
            format!(
                "{}: ",
                truncate_for_telegram(&compact_long_tokens(&value), 42)
            )
        })
        .unwrap_or_default();
    let summary = truncate_for_telegram(&compact_long_tokens(&event.summary), 120);
    let block_time = event
        .block_time
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .map(|datetime| datetime.format("%H:%M").to_string())
        .unwrap_or_else(|| format!("slot {}", event.slot));
    let url = firewatch_event_url(config, event);

    format!(
        "- [{}] {} {}<a href=\"{}\">{}</a>",
        html_escape(severity),
        html_escape(&block_time),
        html_escape(&context),
        html_attr_escape(&url),
        html_escape(&summary),
    )
}

fn event_context_label(event: &ChainEvent) -> Option<String> {
    payload_str(event, "protocol_name")
        .or_else(|| payload_str(event, "domain_name"))
        .or_else(|| payload_str(event, "program_label"))
        .or_else(|| payload_str(event, "token_symbol"))
        .map(str::to_owned)
        .or_else(|| derived_protocol_name(&event.target_address))
}

fn event_severity(event: &ChainEvent) -> &'static str {
    if event.has_tag("severity:critical") || event.has_tag("signal:unknown_instruction") {
        "critical"
    } else if event.has_tag("severity:high")
        || payload_str(event, "severity").is_some_and(|severity| severity == "high")
        || payload_bool(event, "high_impact").unwrap_or(false)
    {
        "high"
    } else {
        "standard"
    }
}

const DOMAIN_DIGEST_PREFIXES: &[&str] = &[
    "Domain:",
    "Display owner:",
    "Owner:",
    "New owner name:",
    "New owner:",
    "Review:",
    "Updated bytes:",
    "Space:",
    "Level:",
];

const PROGRAM_DIGEST_PREFIXES: &[&str] = &[
    "Protocol:",
    "Surface:",
    "Program:",
    "Authority:",
    "Program bytes:",
    "Program SHA-256:",
    "Loader:",
    "Source:",
    "Buffer:",
];

const IDL_DIGEST_PREFIXES: &[&str] = &[
    "Protocol:",
    "Surface:",
    "Program:",
    "IDL action:",
    "Impact:",
    "Actor:",
    "New authority:",
    "Via multisig:",
    "Format:",
];

const PROTOCOL_DIGEST_PREFIXES: &[&str] = &[
    "Protocol:",
    "Surface:",
    "Program:",
    "Instruction:",
    "Metric:",
    "Signal:",
    "Severity:",
    "Impact:",
    "Actor:",
    "Review:",
    "Rule:",
    "Discriminator:",
];

const TOKEN_LIQUIDITY_DIGEST_PREFIXES: &[&str] = &[
    "Token:",
    "Pool:",
    "DEX:",
    "Reserve:",
    "Reserve USD:",
    "Threshold:",
    "Critical:",
    "Drop:",
    "Low since:",
    "Wallet balance:",
];

const TREASURY_OUTFLOW_DIGEST_PREFIXES: &[&str] = &[
    "Protocol:",
    "Token:",
    "Amount:",
    "Wallet:",
    "Destination:",
    "Threshold:",
];

const MULTISIG_DIGEST_PREFIXES: &[&str] = &[
    "Decoded protocol:",
    "Decoded action:",
    "Multisig:",
    "Member name:",
    "Member:",
    "Proposal:",
    "Transaction account:",
];

const DEFAULT_DIGEST_PREFIXES: &[&str] = &["Summary:", "Target:", "Actor:"];

fn event_title(event: &ChainEvent) -> &'static str {
    match event.kind.as_str() {
        event_kind::NAME_SERVICE_DOMAIN_CREATED => "SNS domain created",
        event_kind::NAME_SERVICE_DOMAIN_UPDATED => "SNS domain updated",
        event_kind::NAME_SERVICE_DOMAIN_TRANSFERRED => "SNS domain ownership changed",
        event_kind::NAME_SERVICE_DOMAIN_DELETED => "SNS domain deleted",
        event_kind::NAME_SERVICE_DOMAIN_REALLOCATED => "SNS domain resized",
        event_kind::PROGRAM_DEPLOYED => "Program deployed",
        event_kind::PROGRAM_UPGRADED => "Program upgraded",
        event_kind::PROGRAM_FINALIZED => "Program finalized",
        event_kind::PROGRAM_MIGRATED_TO_LOADER_V4 => "Program migrated to loader v4",
        event_kind::PROGRAM_IDL_INITIALIZED => "Program IDL initialized",
        event_kind::PROGRAM_IDL_UPDATED => "Program IDL updated",
        event_kind::PROGRAM_IDL_AUTHORITY_CHANGED => "Program IDL authority changed",
        event_kind::PROGRAM_IDL_IMMUTABLE => "Program IDL marked immutable",
        event_kind::PROGRAM_IDL_RESIZED => "Program IDL resized",
        event_kind::PROGRAM_IDL_CLOSED => "Program IDL closed",
        event_kind::PROGRAM_ADMIN_INSTRUCTION => "Protocol admin instruction",
        event_kind::PROGRAM_METRIC_ALERT => "Protocol metric anomaly",
        event_kind::PROGRAM_UNKNOWN_INSTRUCTION => "Critical unknown protocol instruction",
        event_kind::TOKEN_LIQUIDITY_LOW => "Token liquidity low",
        event_kind::TREASURY_TOKEN_OUTFLOW => "Treasury token outflow",
        event_kind::MULTISIG_PROPOSAL_ACTIVATED => "Multisig proposal activated",
        event_kind::MULTISIG_PROPOSAL_APPROVED => "Multisig approval",
        event_kind::MULTISIG_PROPOSAL_REJECTED => "Multisig rejection",
        event_kind::MULTISIG_PROPOSAL_CANCELLED => "Multisig cancellation",
        event_kind::MULTISIG_VAULT_EXECUTED => "Multisig vault execution",
        event_kind::MULTISIG_CONFIG_EXECUTED => "Multisig config execution",
        _ => "Chain event",
    }
}

fn domain_digest_lines(config: &Config, event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(domain_name) =
        payload_str(event, "domain_name").or(payload_str(event, "target_name"))
    {
        lines.push(format!("Domain: {domain_name}"));
    }

    lines.push(format!(
        "Domain account: {}",
        payload_str(event, "domain_account").unwrap_or(&event.target_address)
    ));

    if let Some(owner_name) = payload_str(event, "actor_name").or(payload_str(event, "owner_name"))
    {
        lines.push(format!("Display owner: {owner_name}"));
    }

    if let Some(owner) = payload_str(event, "owner") {
        lines.push(format!("Owner: {owner}"));
    }

    if let Some(new_owner_name) = payload_str(event, "new_owner_name") {
        lines.push(format!("New owner name: {new_owner_name}"));
    }

    if let Some(new_owner) = payload_str(event, "new_owner") {
        lines.push(format!("New owner: {new_owner}"));
        lines.push(format!(
            "New owner page: {}",
            explorer_account_url(config, new_owner)
        ));
    }

    if let Some(refund_target_name) = payload_str(event, "refund_target_name") {
        lines.push(format!("Refund target name: {refund_target_name}"));
    }

    if let Some(refund_target) = payload_str(event, "refund_target") {
        lines.push(format!("Refund target: {refund_target}"));
        lines.push(format!(
            "Refund target page: {}",
            explorer_account_url(config, refund_target)
        ));
    }

    if let Some(space) = payload_u64(event, "space") {
        lines.push(format!("Space: {space} bytes"));
    }

    if let Some(offset) = payload_u64(event, "offset") {
        lines.push(format!("Offset: {offset}"));
    }

    if let Some(data_len) = payload_u64(event, "data_len") {
        lines.push(format!("Updated bytes: {data_len}"));
    }

    if payload_bool(event, "root_domain").unwrap_or(false) {
        lines.push("Level: root .sol domain".to_owned());
    } else {
        lines.push("Level: subdomain".to_owned());
    }

    if payload_bool(event, "critical_domain").unwrap_or(false) {
        lines.push("Review: sensitive tracked domain".to_owned());
    }

    if let Some(linked_protocols) = event
        .payload
        .get("linked_protocols")
        .and_then(|value| value.as_array())
        .filter(|items| !items.is_empty())
    {
        let protocols = linked_protocols
            .iter()
            .filter_map(|item| item.get("name").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        if !protocols.is_empty() {
            lines.push(format!("Project links: {}", protocols.join(", ")));
        }
    }

    lines
}

fn program_digest_lines(event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    let protocol_name = payload_str(event, "protocol_name")
        .map(str::to_owned)
        .or_else(|| derived_protocol_name(&event.target_address));
    if let Some(protocol_name) = protocol_name {
        lines.push(format!("Protocol: {protocol_name}"));
    }

    lines.push(format!(
        "Program: {}",
        payload_str(event, "program").unwrap_or(&event.target_address)
    ));

    let program_label = payload_str(event, "program_label")
        .map(str::to_owned)
        .or_else(|| derived_program_label(&event.target_address));
    if let Some(program_label) = program_label {
        lines.push(format!("Surface: {program_label}"));
    }

    if let Some(authority_name) = payload_str(event, "actor_name") {
        lines.push(format!("Authority name: {authority_name}"));
    }

    if let Some(authority) = payload_str(event, "authority").or(event.actor.as_deref()) {
        lines.push(format!("Authority: {authority}"));
    }

    if let Some(loader) = payload_str(event, "loader") {
        lines.push(format!("Loader: {loader}"));
    }

    if let Some(parent_program) = payload_str(event, "parent_program") {
        lines.push(format!("Source: {parent_program}"));
    }

    if let Some(buffer) = payload_str(event, "buffer_account") {
        lines.push(format!("Buffer: {buffer}"));
    }

    if let Some(program_data) = payload_str(event, "program_data_account") {
        lines.push(format!("Program data: {program_data}"));
    }

    if let Some(max_data_len) = payload_u64(event, "max_data_len") {
        lines.push(format!("Max data len: {max_data_len}"));
    }

    if let Some(program_len) = payload_u64(event, "program_bytes_len") {
        lines.push(format!("Program bytes: {program_len}"));
    }

    if let Some(sha256) = payload_str(event, "program_bytes_sha256") {
        lines.push(format!("Program SHA-256: {}", short_digest(sha256)));
    } else if payload_bool(event, "program_bytes_available") == Some(false) {
        lines.push("Program bytes: pending upload chunks".to_owned());
    }

    lines
}

fn multisig_digest_lines(event: &ChainEvent) -> Vec<String> {
    let mut lines = vec![format!(
        "Multisig: {}",
        payload_str(event, "multisig").unwrap_or(&event.target_address)
    )];

    if let Some(decoded_protocol) = payload_str(event, "decoded_protocol") {
        lines.push(format!("Decoded protocol: {decoded_protocol}"));
    } else if let Some(decoded_program) = payload_str(event, "decoded_program") {
        if let Some(protocol_name) = derived_protocol_name(decoded_program) {
            lines.push(format!("Decoded protocol: {protocol_name}"));
        }
    }

    if let Some(decoded_summary) = payload_str(event, "decoded_summary") {
        lines.push(format!("Decoded action: {decoded_summary}"));
    }

    if let Some(decoded_program) = payload_str(event, "decoded_program") {
        lines.push(format!("Decoded program: {decoded_program}"));
    }

    if let Some(member_name) =
        payload_str(event, "member_name").or(payload_str(event, "actor_name"))
    {
        lines.push(format!("Member name: {member_name}"));
    }

    if let Some(member) = payload_str(event, "member").or(event.actor.as_deref()) {
        lines.push(format!("Member: {member}"));
    }

    if let Some(top_signer) = payload_str(event, "program_multisig_top_signer") {
        if let Some(top_count) = payload_u64(event, "program_multisig_top_signer_actions") {
            lines.push(format!(
                "Usual signer: {top_signer} ({top_count} prior actions)"
            ));
        }
    }

    if let Some(proposal) = payload_str(event, "proposal") {
        lines.push(format!("Proposal: {proposal}"));
    }

    if let Some(transaction) = payload_str(event, "transaction") {
        lines.push(format!("Transaction account: {transaction}"));
    }

    if !event.kind.contains("executed")
        && payload_str(event, "decoded_program").is_none()
        && payload_str(event, "decoded_protocol").is_none()
    {
        lines.push("Target program: not known yet on approval-only events".to_owned());
    }

    lines
}

fn idl_digest_lines(config: &Config, event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(protocol_name) = payload_str(event, "protocol_name") {
        lines.push(format!("Protocol: {protocol_name}"));
    }

    lines.push(format!(
        "Program: {}",
        payload_str(event, "program").unwrap_or(&event.target_address)
    ));

    if let Some(program_label) = payload_str(event, "program_label") {
        lines.push(format!("Surface: {program_label}"));
    }

    if let Some(instruction_name) = payload_str(event, "instruction_name") {
        lines.push(format!("IDL action: {instruction_name}"));
    }

    if let Some(metadata_account) = payload_str(event, "metadata_account") {
        lines.push(format!("IDL metadata: {metadata_account}"));
        lines.push(format!(
            "IDL metadata page: {}",
            explorer_account_url(config, metadata_account)
        ));
    }

    if let Some(format) = payload_str(event, "format") {
        lines.push(format!("Format: {format}"));
    }

    if let Some(encoding) = payload_str(event, "encoding") {
        lines.push(format!("Encoding: {encoding}"));
    }

    if let Some(compression) = payload_str(event, "compression") {
        lines.push(format!("Compression: {compression}"));
    }

    if let Some(data_source) = payload_str(event, "data_source") {
        lines.push(format!("Data source: {data_source}"));
    }

    if let Some(inline_data_len) = payload_u64(event, "inline_data_len") {
        lines.push(format!("Inline payload bytes: {inline_data_len}"));
    }

    if let Some(length) = payload_u64(event, "length") {
        lines.push(format!("Resize delta: {length} bytes"));
    }

    if let Some(new_authority) = payload_str(event, "new_authority") {
        lines.push(format!("New authority: {new_authority}"));
    }

    if payload_bool(event, "high_impact").unwrap_or(false) {
        lines.push("Impact: high".to_owned());
    }

    if let Some(actor) = payload_str(event, "member").or(event.actor.as_deref()) {
        lines.push(format!("Actor: {actor}"));
    }

    if payload_bool(event, "via_multisig").unwrap_or(false) {
        if let Some(multisig) = payload_str(event, "multisig") {
            lines.push(format!("Via multisig: {multisig}"));
            lines.push(format!(
                "Multisig page: {}",
                explorer_account_url(config, multisig)
            ));
        } else {
            lines.push("Via multisig: yes".to_owned());
        }
    }

    if let Some(call_origin) = payload_str(event, "call_origin") {
        lines.push(format!("Call origin: {call_origin}"));
    }

    lines
}

fn protocol_digest_lines(config: &Config, event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(protocol_name) = payload_str(event, "protocol_name") {
        lines.push(format!("Protocol: {protocol_name}"));
    }

    lines.push(format!(
        "Program: {}",
        payload_str(event, "program").unwrap_or(&event.target_address)
    ));

    if let Some(program_label) = payload_str(event, "program_label") {
        lines.push(format!("Surface: {program_label}"));
    }

    if let Some(instruction_name) = payload_str(event, "instruction_name") {
        lines.push(format!("Instruction: {instruction_name}"));
    }

    if let Some(metric_label) = payload_str(event, "metric_label") {
        if let Some(metric_value) = payload_f64(event, "metric_value") {
            lines.push(format!("Metric: {metric_label} = {metric_value}"));
        } else {
            lines.push(format!("Metric: {metric_label}"));
        }
    }

    if let Some(classification) = payload_str(event, "classification") {
        lines.push(format!("Signal: {classification}"));
    }

    append_authority_digest_lines(&mut lines, event);

    if let Some(severity) = payload_str(event, "severity") {
        lines.push(format!("Severity: {severity}"));
    }

    if payload_bool(event, "high_impact").unwrap_or(false) {
        lines.push("Impact: high".to_owned());
    }

    if let Some(actor) = payload_str(event, "member").or(event.actor.as_deref()) {
        lines.push(format!("Actor: {actor}"));
    }

    if payload_bool(event, "via_multisig").unwrap_or(false) {
        if let Some(multisig) = payload_str(event, "multisig") {
            lines.push(format!("Via multisig: {multisig}"));
            lines.push(format!(
                "Multisig page: {}",
                explorer_account_url(config, multisig)
            ));
        } else {
            lines.push("Via multisig: yes".to_owned());
        }
    }

    if let Some(call_origin) = payload_str(event, "call_origin") {
        lines.push(format!("Call origin: {call_origin}"));
    }

    if let Some(parent_program) = payload_str(event, "parent_program") {
        lines.push(format!("Top-level caller: {parent_program}"));
    }

    if let Some(immediate_parent_program) = payload_str(event, "immediate_parent_program") {
        lines.push(format!("Immediate caller: {immediate_parent_program}"));
    }

    if let Some(proposal) = payload_str(event, "proposal") {
        lines.push(format!("Proposal: {proposal}"));
    }

    if let Some(subject_address) = payload_str(event, "subject_address") {
        let subject_label = payload_str(event, "subject_label").unwrap_or("Subject");
        lines.push(format!("{subject_label}: {subject_address}"));
    }

    if let Some(rule_label) = payload_str(event, "rule_label") {
        lines.push(format!("Rule: {rule_label}"));
    }

    if let Some(previous_metric_value) = payload_f64(event, "previous_metric_value") {
        lines.push(format!("Previous value: {previous_metric_value}"));
    }

    if let Some(transaction) = payload_str(event, "transaction") {
        lines.push(format!("Transaction account: {transaction}"));
    }

    if matches!(event.kind.as_str(), event_kind::PROGRAM_UNKNOWN_INSTRUCTION) {
        lines.push("Review: immediate".to_owned());
        if let Some(discriminator) = payload_str(event, "discriminator") {
            lines.push(format!("Discriminator: {discriminator}"));
        }
    }

    lines
}

fn treasury_outflow_digest_lines(event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(protocol) = payload_str(event, "protocol_name") {
        lines.push(format!("Protocol: {protocol}"));
    }

    if let Some(symbol) = payload_str(event, "token_symbol") {
        lines.push(format!("Token: {symbol}"));
    }

    if let Some(amount) = payload_str(event, "display_amount") {
        let symbol = payload_str(event, "token_symbol").unwrap_or("token");
        lines.push(format!("Amount: {amount} {symbol}"));
    }

    if let Some(wallet) = payload_str(event, "wallet") {
        lines.push(format!("Wallet: {wallet}"));
    }

    if let Some(destination) = payload_str(event, "destination_token_account") {
        lines.push(format!("Destination: {destination}"));
    }

    if let Some(threshold) = payload_str(event, "critical_threshold_display_amount") {
        let symbol = payload_str(event, "token_symbol").unwrap_or("token");
        lines.push(format!("Threshold: {threshold} {symbol}"));
    }

    lines
}

fn token_liquidity_digest_lines(event: &ChainEvent) -> Vec<String> {
    let mut lines = Vec::new();

    if let Some(symbol) = payload_str(event, "token_symbol") {
        lines.push(format!("Token: {symbol}"));
    }

    if let Some(pool_address) = payload_str(event, "pool_address") {
        lines.push(format!("Pool: {pool_address}"));
    }

    if let Some(dex_id) = payload_str(event, "dex_id") {
        lines.push(format!("DEX: {dex_id}"));
    }

    if let Some(reserve) = payload_f64(event, "watched_token_reserve") {
        let symbol = payload_str(event, "token_symbol").unwrap_or("token");
        lines.push(format!("Reserve: {reserve:.8} {symbol}"));
    }

    if let Some(reserve_usd) = payload_f64(event, "watched_token_reserve_usd") {
        lines.push(format!("Reserve USD: {reserve_usd:.2}"));
    }

    if let Some(threshold) = payload_f64(event, "min_token_reserve") {
        let symbol = payload_str(event, "token_symbol").unwrap_or("token");
        lines.push(format!("Threshold: {threshold:.8} {symbol}"));
    } else if let Some(threshold_usd) = payload_f64(event, "min_token_reserve_usd") {
        lines.push(format!("Threshold: {threshold_usd:.2} USD"));
    }

    if let Some(reason) = payload_str(event, "critical_reason") {
        lines.push(format!("Critical: {reason}"));
    }

    if let Some(delta) = payload_f64(event, "watched_token_reserve_delta") {
        if delta < 0.0 {
            let symbol = payload_str(event, "token_symbol").unwrap_or("token");
            let ratio = payload_f64(event, "watched_token_reserve_delta_ratio")
                .map(|value| format!(" ({:.2}%)", value * 100.0))
                .unwrap_or_default();
            lines.push(format!("Drop: {:.8} {symbol}{ratio}", delta.abs()));
        }
    }

    if let Some(low_since) = event.payload.get("low_since") {
        if let Some(slot) = low_since.get("slot").and_then(serde_json::Value::as_i64) {
            lines.push(format!("Low since: slot {slot}"));
        }
    }

    if let Some(wallet_balance) = payload_f64(event, "wallet_token_balance") {
        let symbol = payload_str(event, "token_symbol").unwrap_or("token");
        lines.push(format!("Wallet balance: {wallet_balance:.8} {symbol}"));
    }

    lines
}

fn append_authority_digest_lines(lines: &mut Vec<String>, event: &ChainEvent) {
    if let Some(authority) = payload_str(event, "authority") {
        let authority_label = payload_str(event, "authority_label").unwrap_or("Authority");
        lines.push(format!("{authority_label}: {authority}"));
    }

    if let Some(new_authority) = payload_str(event, "new_authority") {
        let new_authority_label =
            payload_str(event, "new_authority_label").unwrap_or("New authority");
        lines.push(format!("{new_authority_label}: {new_authority}"));
    }

    if let Some(whirlpools_config) = payload_str(event, "whirlpools_config") {
        lines.push(format!("Whirlpools config: {whirlpools_config}"));
    }

    if let Some(whirlpools_config_extension) = payload_str(event, "whirlpools_config_extension") {
        lines.push(format!(
            "Whirlpools config extension: {whirlpools_config_extension}"
        ));
    }
}

fn payload_str<'a>(event: &'a ChainEvent, key: &str) -> Option<&'a str> {
    event.payload.get(key)?.as_str()
}

fn payload_bool(event: &ChainEvent, key: &str) -> Option<bool> {
    event.payload.get(key)?.as_bool()
}

fn payload_u64(event: &ChainEvent, key: &str) -> Option<u64> {
    event.payload.get(key)?.as_u64()
}

fn payload_f64(event: &ChainEvent, key: &str) -> Option<f64> {
    event.payload.get(key)?.as_f64()
}

fn short_digest(value: &str) -> String {
    if value.len() <= 16 {
        value.to_owned()
    } else {
        format!("{}...{}", &value[..8], &value[value.len() - 8..])
    }
}

fn selected_digest_lines(
    lines: Vec<String>,
    preferred_prefixes: &[&str],
    limit: usize,
) -> Vec<String> {
    let mut candidates = lines
        .into_iter()
        .filter(|line| !line.contains("://"))
        .collect::<Vec<_>>();
    let mut selected = Vec::new();

    for prefix in preferred_prefixes {
        if selected.len() >= limit {
            break;
        }
        if let Some(position) = candidates.iter().position(|line| line.starts_with(prefix)) {
            selected.push(candidates.remove(position));
        }
    }

    for line in candidates {
        if selected.len() >= limit {
            break;
        }
        selected.push(line);
    }

    selected
}

fn render_telegram_line(line: &str) -> String {
    let compact = compact_long_tokens(line);
    html_escape(&compact)
}

fn compact_long_tokens(input: &str) -> String {
    input
        .split_whitespace()
        .map(compact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn compact_token(token: &str) -> String {
    if !token.is_ascii() {
        return token.to_owned();
    }

    let bytes = token.as_bytes();
    let start = bytes
        .iter()
        .position(|byte| byte.is_ascii_alphanumeric())
        .unwrap_or(0);
    let end = bytes
        .iter()
        .rposition(|byte| byte.is_ascii_alphanumeric())
        .map(|position| position + 1)
        .unwrap_or(token.len());
    let core = &token[start..end];

    if core.len() < 32 || !core.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return token.to_owned();
    }

    format!(
        "{}{}{}",
        &token[..start],
        compact_identifier(core, 6, 6),
        &token[end..]
    )
}

fn compact_identifier(value: &str, prefix_len: usize, suffix_len: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= prefix_len + suffix_len + 3 {
        return value.to_owned();
    }

    let prefix = value.chars().take(prefix_len).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(suffix_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn truncate_for_telegram(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

fn latest_due_summary_day(now: i64, send_hour_utc: i64) -> i64 {
    let shifted_now = now - send_hour_utc.clamp(0, 23) * 60 * 60;
    day_start_utc(shifted_now) - 86_400
}

fn day_start_utc(timestamp: i64) -> i64 {
    timestamp.div_euclid(86_400) * 86_400
}

fn firewatch_events_url(config: &Config) -> String {
    let base_url = config.firewatch_web_base_url.trim().trim_end_matches('/');
    format!("{base_url}/events")
}

fn firewatch_event_url(config: &Config, event: &ChainEvent) -> String {
    let base_url = config.firewatch_web_base_url.trim().trim_end_matches('/');
    format!("{base_url}/events/{}", percent_encode(&event.id))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_attr_escape(value: &str) -> String {
    html_escape(value).replace('"', "&quot;")
}

fn format_block_time(block_time: Option<i64>) -> Option<String> {
    let timestamp = block_time?;
    let datetime = DateTime::<Utc>::from_timestamp(timestamp, 0)?;
    Some(datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string())
}

fn explorer_query(config: &Config) -> Option<String> {
    (config.solana_explorer_cluster != "mainnet-beta")
        .then(|| format!("?cluster={}", config.solana_explorer_cluster))
}

fn explorer_tx_url(config: &Config, signature: &str) -> String {
    format!(
        "{}/tx/{}{}",
        config.solana_explorer_base_url.trim_end_matches('/'),
        signature,
        explorer_query(config).unwrap_or_default(),
    )
}

fn explorer_account_url(config: &Config, address: &str) -> String {
    format!(
        "{}/address/{}{}",
        config.solana_explorer_base_url.trim_end_matches('/'),
        address,
        explorer_query(config).unwrap_or_default(),
    )
}

fn derived_protocol_name(program_address: &str) -> Option<String> {
    let program_id = Pubkey::from_str(program_address).ok()?;
    Some(protocols::protocol_for_program(program_id)?.name)
}

fn derived_program_label(program_address: &str) -> Option<String> {
    let program_id = Pubkey::from_str(program_address).ok()?;
    Some(protocols::protocol_for_program(program_id)?.program_label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::target_type;
    use serde_json::json;
    use std::path::PathBuf;

    const PROGRAM: &str = "6EF8rrecthR5DkxxxxxxxxxxxxxxxxxxxxxxxxxxwF6P";
    const AUTHORITY: &str = "Authority111111111111111111111111111111111999999";
    const SIGNATURE: &str =
        "5u3QJ9mZx4V2P1aBcDeFgHiJkLmNoPqRsTuVwXyZ123456789ABCDEFGHJKLMNPQRSTUVWXYZabc";

    fn test_config() -> Config {
        Config {
            http_bind: "0.0.0.0:8080".to_owned(),
            database_url: "sqlite::memory:".to_owned(),
            blob_store_dir: None::<PathBuf>,
            blob_store_s3: None,
            solana_grpc_endpoint: "https://example.invalid".to_owned(),
            solana_grpc_x_token: None,
            solana_rpc_http_url: "https://example.invalid".to_owned(),
            solana_pay_rpc_http_url: "https://jenifer-6iwpyb-fast-mainnet.helius-rpc.com"
                .to_owned(),
            solana_explorer_base_url: "https://explorer.solana.com".to_owned(),
            solana_explorer_cluster: "mainnet-beta".to_owned(),
            telegram_bot_token: None,
            telegram_api_base: "https://api.telegram.org".to_owned(),
            firewatch_web_base_url: "https://watch.ferno.ag".to_owned(),
            origin_api_token: None,
            cors_allowed_origin: "http://127.0.0.1:5173".to_owned(),
            delivery_poll_seconds: 5,
            telegram_daily_summary_poll_seconds: 300,
            telegram_daily_summary_hour_utc: 9,
            reconnect_delay_seconds: 5,
            sns_bootstrap_label_limit: 96,
            critical_sol_domains: Vec::new(),
            ferno_block_base_url: "https://block.ferno.ag/block".to_owned(),
            solana_pay_recipient: None,
            solana_pay_custom_plan_amount: None,
            solana_pay_custom_plan_yearly_amount: None,
            solana_pay_spl_token: None,
            solana_pay_label: "FireWatch".to_owned(),
            solana_pay_link_ttl_hours: 72,
            liquidity_pool_snapshot_retention_days: 3,
            watched_liquidity_pool_snapshot_retention_days: 7,
            liquidity_pool_snapshot_prune_interval_seconds: 6 * 60 * 60,
        }
    }

    fn sample_program_event() -> ChainEvent {
        ChainEvent {
            id: format!("{SIGNATURE}:0.1:program_upgraded:{PROGRAM}"),
            signature: SIGNATURE.to_owned(),
            instruction_path: "0.1".to_owned(),
            kind: event_kind::PROGRAM_UPGRADED.to_owned(),
            target_type: target_type::PROGRAM.to_owned(),
            target_address: PROGRAM.to_owned(),
            actor: Some(AUTHORITY.to_owned()),
            slot: 401_000_123,
            block_time: Some(1_716_000_000),
            indexed_at: 1_716_000_001,
            summary: "Program upgraded".to_owned(),
            tags: vec!["watch:defi".to_owned()],
            payload: json!({
                "protocol_name": "Pump.fun",
                "program_label": "Launchpad",
                "program": PROGRAM,
                "authority": AUTHORITY,
                "loader": "upgradeable",
                "program_bytes_len": 201856,
                "program_bytes_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "program_version_id": "sig:0:program_version"
            }),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 0,
        }
    }

    #[test]
    fn telegram_program_message_is_short_and_links_to_firewatch() {
        let message = format_telegram_message(&test_config(), &sample_program_event());

        assert!(message.contains("<b>Program upgraded</b>"));
        assert!(message.contains("Protocol: Pump.fun"));
        assert!(message.contains("Surface: Launchpad"));
        assert!(message.contains("Program: 6EF8rr...xxwF6P"));
        assert!(message.contains("Authority: Author...999999"));
        assert!(message.contains("Program bytes: 201856"));
        assert!(message.contains("Program SHA-256: 01234567...89abcdef"));
        assert!(message.contains(">Open in FireWatch</a>"));
        assert!(message.contains(
            "https://watch.ferno.ag/events/5u3QJ9mZx4V2P1aBcDeFgHiJkLmNoPqRsTuVwXyZ123456789ABCDEFGHJKLMNPQRSTUVWXYZabc%3A0.1%3Aprogram_upgraded%3A6EF8rrecthR5DkxxxxxxxxxxxxxxxxxxxxxxxxxxwF6P"
        ));
        assert!(!message.contains(&format!("Program: {PROGRAM}")));
        assert!(!message.contains(&format!("Authority: {AUTHORITY}")));
        assert!(!message.contains("Program page:"));
        assert!(!message.contains("/address/"));
        assert!(message.lines().count() <= 10);
    }

    #[test]
    fn percent_encode_handles_route_params() {
        assert_eq!(
            percent_encode("sig:0:program_version"),
            "sig%3A0%3Aprogram_version"
        );
    }

    #[test]
    fn supported_protocol_slugs_only_keeps_supported_protocol_tags() {
        let mut event = sample_program_event();
        event.tags = vec![
            "watch:defi".to_owned(),
            "protocol:squads".to_owned(),
            "protocol:unknown_protocol".to_owned(),
            "protocol:squads".to_owned(),
        ];

        assert_eq!(supported_protocol_slugs(&event), vec!["squads".to_owned()]);
    }

    #[test]
    fn supported_protocol_slugs_prefers_linked_protocols_over_squads() {
        let mut event = sample_program_event();
        event.tags = vec![
            "watch:defi".to_owned(),
            "source:squads".to_owned(),
            "protocol:squads".to_owned(),
            "protocol:pumpfun".to_owned(),
        ];
        event.payload = json!({
            "protocol": "squads",
            "decoded_protocol": "pumpfun",
            "linked_protocols": [
                {
                    "protocol": "pumpfun",
                    "protocol_name": "Pump.fun",
                    "evidence": "decoded_execution"
                }
            ],
        });

        assert_eq!(supported_protocol_slugs(&event), vec!["pumpfun".to_owned()]);
    }
}
