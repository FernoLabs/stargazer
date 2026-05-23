use crate::{
    config::Config,
    context::SharedState,
    models::{ChainEvent, PendingTelegramDelivery, event_kind, now_ts},
    name_service, protocols,
};
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{str::FromStr, time::Duration};
use tokio::time::sleep;
use tracing::{debug, warn};

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
    message_thread_id: Option<i64>,
}

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

pub async fn enqueue_telegram_deliveries(state: SharedState, event: &ChainEvent) -> Result<()> {
    let matching_subscriptions = state.db.matching_telegram_subscriptions(event).await?;
    let initial_next_retry_at = now_ts() + state.config.delivery_poll_seconds as i64;

    for subscription in matching_subscriptions {
        if let Some(delivery) = state
            .db
            .enqueue_telegram_delivery(event, &subscription, initial_next_retry_at)
            .await?
        {
            tokio::spawn(attempt_telegram_delivery(state.clone(), delivery));
        }
    }

    Ok(())
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

async fn deliver_telegram_event(
    state: &SharedState,
    delivery: &PendingTelegramDelivery,
) -> Result<()> {
    let hydrated_event = name_service::hydrate_event(state, delivery.event.clone()).await;
    let message = format_telegram_message(&state.config, &hydrated_event);
    match send_telegram_text(
        state,
        &delivery.subscription.chat_id,
        delivery.subscription.thread_id,
        &message,
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

pub async fn send_telegram_text(
    state: &SharedState,
    chat_id: &str,
    thread_id: Option<i64>,
    text: &str,
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
        disable_web_page_preview: true,
        message_thread_id: thread_id,
    };
    let response = state.http_client.post(endpoint).json(&body).send().await?;
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
    let transaction_url = explorer_tx_url(config, &event.signature);
    let target_url = explorer_account_url(config, &event.target_address);
    let mut lines = vec![format!("FireWatch alert: {}", event_title(event))];

    match event.kind.as_str() {
        event_kind::NAME_SERVICE_DOMAIN_CREATED
        | event_kind::NAME_SERVICE_DOMAIN_UPDATED
        | event_kind::NAME_SERVICE_DOMAIN_TRANSFERRED
        | event_kind::NAME_SERVICE_DOMAIN_DELETED
        | event_kind::NAME_SERVICE_DOMAIN_REALLOCATED => {
            lines.extend(domain_digest_lines(config, event));
            lines.push(format!("Domain account: {target_url}"));
        }
        event_kind::PROGRAM_DEPLOYED
        | event_kind::PROGRAM_UPGRADED
        | event_kind::PROGRAM_FINALIZED
        | event_kind::PROGRAM_MIGRATED_TO_LOADER_V4 => {
            lines.extend(program_digest_lines(event));
            lines.push(format!("Program page: {target_url}"));
        }
        event_kind::PROGRAM_IDL_INITIALIZED
        | event_kind::PROGRAM_IDL_UPDATED
        | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
        | event_kind::PROGRAM_IDL_IMMUTABLE
        | event_kind::PROGRAM_IDL_RESIZED
        | event_kind::PROGRAM_IDL_CLOSED => {
            lines.extend(idl_digest_lines(config, event));
            lines.push(format!("Program page: {target_url}"));
        }
        event_kind::PROGRAM_ADMIN_INSTRUCTION
        | event_kind::PROGRAM_UNKNOWN_INSTRUCTION
        | event_kind::PROGRAM_METRIC_ALERT => {
            lines.extend(protocol_digest_lines(config, event));
            lines.push(format!("Program page: {target_url}"));
        }
        event_kind::MULTISIG_PROPOSAL_ACTIVATED
        | event_kind::MULTISIG_PROPOSAL_APPROVED
        | event_kind::MULTISIG_PROPOSAL_REJECTED
        | event_kind::MULTISIG_PROPOSAL_CANCELLED
        | event_kind::MULTISIG_VAULT_EXECUTED
        | event_kind::MULTISIG_CONFIG_EXECUTED => {
            lines.extend(multisig_digest_lines(event));
            lines.push(format!("Multisig page: {target_url}"));
        }
        _ => {
            lines.push(format!("Summary: {}", event.summary));
            lines.push(format!(
                "Target: {} ({})",
                event.target_address, event.target_type
            ));
            if let Some(actor) = &event.actor {
                lines.push(format!("Actor: {actor}"));
            }
            lines.push(format!("Target page: {target_url}"));
        }
    }

    lines.push(format!("Slot: {}", event.slot));
    lines.push(format!("Path: {}", event.instruction_path));
    if let Some(block_time) = format_block_time(event.block_time) {
        lines.push(format!("Time: {block_time}"));
    }
    lines.push(format!("Tx: {transaction_url}"));

    lines.join("\n")
}

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
