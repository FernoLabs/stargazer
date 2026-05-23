use crate::models::ProtocolMetricObservation;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub struct MetricDecoderInput<'a> {
    pub signature: &'a str,
    pub instruction_path: &'a str,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub protocol_slug: &'a str,
    pub protocol_name: &'a str,
    pub program_id: &'a str,
    pub program_label: &'a str,
    pub instruction_name: Option<&'a str>,
    pub account_addresses: &'a [String],
    pub log_messages: &'a [String],
}

pub fn decode_metric_observations(
    input: &MetricDecoderInput<'_>,
) -> Vec<ProtocolMetricObservation> {
    match (input.protocol_slug, input.instruction_name) {
        ("jupiter_lend", Some("update_rate")) => decode_jupiter_lend_update_rate(input),
        _ => Vec::new(),
    }
}

fn decode_jupiter_lend_update_rate(
    input: &MetricDecoderInput<'_>,
) -> Vec<ProtocolMetricObservation> {
    let Some((token_exchange_price, liquidity_exchange_price)) =
        first_anchor_event(input.log_messages, "LogUpdateRates").and_then(|bytes| {
            let payload = bytes.get(8..24)?;
            let token_exchange_price = u64::from_le_bytes(payload.get(..8)?.try_into().ok()?);
            let liquidity_exchange_price = u64::from_le_bytes(payload.get(8..16)?.try_into().ok()?);
            Some((token_exchange_price, liquidity_exchange_price))
        })
    else {
        return Vec::new();
    };

    let lending_account = input.account_addresses.first().cloned();
    let mint_address = input.account_addresses.get(1).cloned();
    let f_token_mint = input.account_addresses.get(2).cloned();

    let base_payload = json!({
        "decoder": "anchor_event_log",
        "event_name": "LogUpdateRates",
        "lending_account": lending_account,
        "mint": mint_address,
        "f_token_mint": f_token_mint,
    });

    vec![
        ProtocolMetricObservation {
            id: format!(
                "metric:{}:{}:{}",
                input.signature, input.instruction_path, "token_exchange_price"
            ),
            signature: input.signature.to_owned(),
            instruction_path: input.instruction_path.to_owned(),
            slot: input.slot,
            block_time: input.block_time,
            protocol_slug: input.protocol_slug.to_owned(),
            protocol_name: input.protocol_name.to_owned(),
            program_id: input.program_id.to_owned(),
            program_label: input.program_label.to_owned(),
            instruction_name: "update_rate".to_owned(),
            metric_key: "token_exchange_price".to_owned(),
            metric_label: "Token exchange price".to_owned(),
            metric_value: token_exchange_price as f64,
            subject_address: mint_address.clone(),
            subject_kind: Some("mint".to_owned()),
            subject_label: Some("Lend mint".to_owned()),
            payload: base_payload.clone(),
            observed_at: input.slot,
        },
        ProtocolMetricObservation {
            id: format!(
                "metric:{}:{}:{}",
                input.signature, input.instruction_path, "liquidity_exchange_price"
            ),
            signature: input.signature.to_owned(),
            instruction_path: input.instruction_path.to_owned(),
            slot: input.slot,
            block_time: input.block_time,
            protocol_slug: input.protocol_slug.to_owned(),
            protocol_name: input.protocol_name.to_owned(),
            program_id: input.program_id.to_owned(),
            program_label: input.program_label.to_owned(),
            instruction_name: "update_rate".to_owned(),
            metric_key: "liquidity_exchange_price".to_owned(),
            metric_label: "Liquidity exchange price".to_owned(),
            metric_value: liquidity_exchange_price as f64,
            subject_address: mint_address,
            subject_kind: Some("mint".to_owned()),
            subject_label: Some("Lend mint".to_owned()),
            payload: base_payload,
            observed_at: input.slot,
        },
    ]
}

fn first_anchor_event(log_messages: &[String], event_name: &str) -> Option<Vec<u8>> {
    let discriminator = anchor_event_discriminator(event_name);
    for line in log_messages {
        let Some(encoded) = line.strip_prefix("Program data: ") else {
            continue;
        };
        let Ok(decoded) = STANDARD.decode(encoded) else {
            continue;
        };
        if decoded.starts_with(discriminator) {
            return Some(decoded);
        }
    }

    None
}

fn anchor_event_discriminator(event_name: &str) -> &'static [u8; 8] {
    static UPDATE_RATE_EVENT: OnceLock<[u8; 8]> = OnceLock::new();

    match event_name {
        "LogUpdateRates" => UPDATE_RATE_EVENT.get_or_init(|| {
            let mut hasher = Sha256::new();
            hasher.update(b"event:LogUpdateRates");
            let hash = hasher.finalize();
            let mut discriminator = [0_u8; 8];
            discriminator.copy_from_slice(&hash[..8]);
            discriminator
        }),
        _ => panic!("unsupported anchor event discriminator request"),
    }
}
