//! Bounded reader for the NAS-side Classic controller watcher.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

const CURRENT_FILENAME: &str = "current.json";
const ALERTS_FILENAME: &str = "alerts.json";
const MAX_CURRENT_BYTES: u64 = 1_048_576;
const MAX_ALERT_BYTES: u64 = 8_388_608;
const MAX_CONTROLLERS: usize = 128;
const MAX_ALERTS: usize = 1_000;
const MAX_SIGNERS: usize = 32;
const MAX_ACTIVITY_OPERATIONS: usize = 100;
const MAX_OPERATION_PAYLOAD_BYTES: usize = 32_768;
const MAX_OPERATION_PAYLOAD_DEPTH: usize = 8;
const STALE_AFTER: Duration = Duration::minutes(20);
const MAX_CLOCK_SKEW: Duration = Duration::minutes(5);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceMonitorResponse {
    pub available: bool,
    pub reason: String,
    pub observed_at: Option<String>,
    pub stale: Option<bool>,
    pub controller_count: usize,
    pub controllers: Vec<GovernanceController>,
    pub alerts: Vec<GovernanceAlert>,
}

impl GovernanceMonitorResponse {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            reason: reason.into(),
            observed_at: None,
            stale: None,
            controller_count: 0,
            controllers: Vec::new(),
            alerts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceController {
    pub account_id: String,
    pub protocol: String,
    pub role: String,
    pub sources: Vec<String>,
    pub last_modified_ledger: u32,
    pub policy: GovernancePolicy,
    #[serde(default, skip_serializing)]
    pub operation_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernancePolicy {
    pub thresholds: GovernanceThresholds,
    pub master_weight: u32,
    pub signers: Vec<GovernanceSigner>,
    pub classification: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct GovernanceThresholds {
    pub low: u32,
    pub medium: u32,
    pub high: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct GovernanceSigner {
    pub key: String,
    #[serde(rename = "type")]
    pub signer_type: String,
    pub weight: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernancePolicyAlert {
    pub id: String,
    #[serde(rename = "type")]
    pub alert_type: String,
    pub severity: String,
    pub detected_at: String,
    pub account_id: String,
    pub protocol: String,
    pub role: String,
    pub before_observed_at: String,
    pub after_last_modified_ledger: u32,
    pub before: GovernancePolicy,
    pub after: GovernancePolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceActivityAlert {
    pub id: String,
    #[serde(rename = "type")]
    pub alert_type: String,
    pub severity: String,
    pub detected_at: String,
    pub occurred_at: String,
    pub account_id: String,
    pub protocol: String,
    pub role: String,
    pub transaction_hash: String,
    pub operations: Vec<GovernanceOperationPayload>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceOperationPayload {
    pub id: String,
    #[serde(rename = "type")]
    pub operation_type: String,
    pub type_code: u32,
    pub source_account: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum GovernanceAlert {
    Policy(GovernancePolicyAlert),
    Activity(GovernanceActivityAlert),
}

impl GovernanceAlert {
    fn id(&self) -> &str {
        match self {
            Self::Policy(alert) => &alert.id,
            Self::Activity(alert) => &alert.id,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurrentDocument {
    schema_version: u32,
    network: String,
    observed_at: String,
    accounts: Vec<GovernanceController>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AlertsDocument {
    schema_version: u32,
    alerts: Vec<GovernanceAlert>,
}

pub fn load_governance_monitor(directory: Option<&Path>) -> Result<GovernanceMonitorResponse> {
    load_governance_monitor_at(directory, OffsetDateTime::now_utc())
}

fn load_governance_monitor_at(
    directory: Option<&Path>,
    now: OffsetDateTime,
) -> Result<GovernanceMonitorResponse> {
    let Some(directory) = directory else {
        return Ok(GovernanceMonitorResponse::unavailable(
            "Classic controller monitoring is not configured on this read service.",
        ));
    };
    let current_path = directory.join(CURRENT_FILENAME);
    let alerts_path = directory.join(ALERTS_FILENAME);
    if !current_path.exists() && !alerts_path.exists() {
        return Ok(GovernanceMonitorResponse::unavailable(
            "The Classic controller watcher has not published its first complete baseline yet.",
        ));
    }
    if !current_path.exists() || !alerts_path.exists() {
        bail!("governance watcher output is incomplete");
    }

    let current: CurrentDocument = read_bounded_json(&current_path, MAX_CURRENT_BYTES)?;
    let alerts: AlertsDocument = read_bounded_json(&alerts_path, MAX_ALERT_BYTES)?;
    if current.schema_version != 1 || alerts.schema_version != 1 {
        bail!("unsupported governance watcher schema version");
    }
    if current.network != "public" {
        bail!("governance watcher output is not bound to Stellar public network");
    }
    if current.accounts.is_empty() || current.accounts.len() > MAX_CONTROLLERS {
        bail!("governance controller count is outside the supported range");
    }
    if alerts.alerts.len() > MAX_ALERTS {
        bail!("governance alert count exceeds {MAX_ALERTS}");
    }

    let observed = parse_timestamp(&current.observed_at, "governance observedAt")?;
    if observed > now + MAX_CLOCK_SKEW {
        bail!("governance watcher timestamp is too far in the future");
    }
    let stale = now - observed > STALE_AFTER;
    let mut account_ids = BTreeSet::new();
    for controller in &current.accounts {
        validate_controller(controller)?;
        if !account_ids.insert(controller.account_id.as_str()) {
            bail!("duplicate governance controller {}", controller.account_id);
        }
    }

    let mut alert_ids = BTreeSet::new();
    for alert in &alerts.alerts {
        validate_alert(alert)?;
        if !alert_ids.insert(alert.id()) {
            bail!("duplicate governance alert {}", alert.id());
        }
    }

    Ok(GovernanceMonitorResponse {
        available: true,
        reason: if stale {
            "The last complete controller cycle is stale; do not treat it as current.".to_owned()
        } else {
            "Classic signer and threshold policies plus successful controller-sourced operations are monitored from complete Horizon cycles."
                .to_owned()
        },
        observed_at: Some(current.observed_at),
        stale: Some(stale),
        controller_count: current.accounts.len(),
        controllers: current.accounts,
        alerts: alerts.alerts,
    })
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &Path, max_bytes: u64) -> Result<T> {
    let metadata =
        fs::metadata(path).with_context(|| format!("reading {} metadata", path.display()))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        bail!("{} is not a bounded regular file", path.display());
    }
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

fn validate_controller(controller: &GovernanceController) -> Result<()> {
    validate_g_address(&controller.account_id)?;
    validate_label(&controller.protocol, "controller protocol")?;
    validate_label(&controller.role, "controller role")?;
    if controller.sources.is_empty() || controller.sources.len() > 16 {
        bail!("controller sources are outside the supported range");
    }
    for source in &controller.sources {
        if !source.starts_with("https://") || source.len() > 2_048 {
            bail!("controller source must be a bounded HTTPS URL");
        }
    }
    if controller
        .operation_cursor
        .as_deref()
        .is_some_and(|cursor| {
            cursor.is_empty()
                || cursor.len() > 64
                || !cursor.bytes().all(|value| value.is_ascii_digit())
        })
    {
        bail!("controller operation cursor must be a bounded decimal string");
    }
    validate_policy(&controller.policy, Some(&controller.account_id))
}

fn validate_alert(alert: &GovernanceAlert) -> Result<()> {
    match alert {
        GovernanceAlert::Policy(alert) => validate_policy_alert(alert),
        GovernanceAlert::Activity(alert) => validate_activity_alert(alert),
    }
}

fn validate_policy_alert(alert: &GovernancePolicyAlert) -> Result<()> {
    validate_digest(&alert.id, "governance policy alert id")?;
    if alert.alert_type != "classic-controller-policy-changed" || alert.severity != "critical" {
        bail!("unsupported governance alert type or severity");
    }
    parse_timestamp(&alert.detected_at, "alert detectedAt")?;
    parse_timestamp(&alert.before_observed_at, "alert beforeObservedAt")?;
    validate_g_address(&alert.account_id)?;
    validate_label(&alert.protocol, "alert protocol")?;
    validate_label(&alert.role, "alert role")?;
    validate_policy(&alert.before, Some(&alert.account_id))?;
    validate_policy(&alert.after, Some(&alert.account_id))?;
    if alert.before == alert.after {
        bail!("governance alert before and after policies are identical");
    }
    Ok(())
}

fn validate_activity_alert(alert: &GovernanceActivityAlert) -> Result<()> {
    validate_digest(&alert.id, "governance activity alert id")?;
    if alert.alert_type != "classic-controller-transaction-executed" || alert.severity != "high" {
        bail!("unsupported governance activity alert type or severity");
    }
    parse_timestamp(&alert.detected_at, "activity alert detectedAt")?;
    parse_timestamp(&alert.occurred_at, "activity alert occurredAt")?;
    validate_g_address(&alert.account_id)?;
    validate_label(&alert.protocol, "activity alert protocol")?;
    validate_label(&alert.role, "activity alert role")?;
    validate_digest(&alert.transaction_hash, "activity alert transaction hash")?;
    if alert.operations.is_empty() || alert.operations.len() > MAX_ACTIVITY_OPERATIONS {
        bail!("activity alert operation count is outside the supported range");
    }
    let mut operation_ids = BTreeSet::new();
    for operation in &alert.operations {
        if operation.id.is_empty()
            || operation.id.len() > 64
            || !operation.id.bytes().all(|value| value.is_ascii_digit())
        {
            bail!("activity operation id must be a bounded decimal string");
        }
        if !operation_ids.insert(operation.id.as_str()) {
            bail!("duplicate activity operation id");
        }
        validate_label(&operation.operation_type, "activity operation type")?;
        if operation.type_code > 1_024 {
            bail!("activity operation type code is outside the supported range");
        }
        validate_g_address(&operation.source_account)?;
        if operation.source_account != alert.account_id {
            bail!("activity operation source does not match the tracked controller");
        }
        if !operation.payload.is_object() {
            bail!("activity operation payload must be an object");
        }
        if serde_json::to_vec(&operation.payload)?.len() > MAX_OPERATION_PAYLOAD_BYTES {
            bail!("activity operation payload exceeds the supported size");
        }
        validate_json_payload(&operation.payload, 0)?;
    }
    Ok(())
}

fn validate_policy(policy: &GovernancePolicy, account_id: Option<&str>) -> Result<()> {
    for value in [
        policy.thresholds.low,
        policy.thresholds.medium,
        policy.thresholds.high,
        policy.master_weight,
    ] {
        if value > u32::from(u8::MAX) {
            bail!("governance threshold or weight exceeds one byte");
        }
    }
    if policy.signers.is_empty() || policy.signers.len() > MAX_SIGNERS {
        bail!("governance signer count is outside the supported range");
    }
    let mut previous: Option<(&str, &str)> = None;
    let mut identities = BTreeSet::new();
    let mut master_weight = None;
    for signer in &policy.signers {
        if signer.weight > u32::from(u8::MAX) {
            bail!("governance signer weight exceeds one byte");
        }
        match signer.signer_type.as_str() {
            "ed25519_public_key" => validate_g_address(&signer.key)?,
            "sha256_hash" | "preauth_tx" => {
                if signer.key.len() != 64
                    || !signer.key.bytes().all(|value| value.is_ascii_hexdigit())
                {
                    bail!("non-Ed25519 signer key must be a 32-byte hex digest");
                }
            }
            _ => bail!("unsupported governance signer type"),
        }
        let identity = (signer.signer_type.as_str(), signer.key.as_str());
        if previous.is_some_and(|value| value >= identity) {
            bail!("governance signers must be uniquely sorted by type and key");
        }
        previous = Some(identity);
        if !identities.insert(identity) {
            bail!("duplicate governance signer");
        }
        if account_id == Some(signer.key.as_str()) && signer.signer_type == "ed25519_public_key" {
            master_weight = Some(signer.weight);
        }
    }
    if master_weight != Some(policy.master_weight) {
        bail!("governance master weight does not match the master signer");
    }
    let expected = classify_policy(policy);
    if policy.classification != expected {
        bail!(
            "governance classification {} does not match derived {expected}",
            policy.classification
        );
    }
    Ok(())
}

fn classify_policy(policy: &GovernancePolicy) -> &'static str {
    let positive = policy
        .signers
        .iter()
        .filter(|signer| signer.weight > 0)
        .collect::<Vec<_>>();
    if positive.is_empty() {
        return "locked";
    }
    if positive.len() == 1 {
        return "single-key";
    }
    let weights = positive
        .iter()
        .map(|signer| signer.weight)
        .collect::<BTreeSet<_>>();
    let thresholds = [
        policy.thresholds.low,
        policy.thresholds.medium,
        policy.thresholds.high,
    ];
    if weights.len() == 1 && thresholds.iter().all(|value| *value == thresholds[0]) {
        let signer_weight = positive[0].weight;
        let effective_threshold = thresholds[0].max(1);
        let required = effective_threshold.div_ceil(signer_weight);
        if required > 1 && usize::try_from(required).is_ok_and(|value| value <= positive.len()) {
            return "strict";
        }
    }
    "weighted"
}

fn validate_g_address(value: &str) -> Result<()> {
    let parsed: stellar_xdr::MuxedAccount = value
        .parse()
        .with_context(|| format!("invalid Stellar account address {value}"))?;
    if !matches!(parsed, stellar_xdr::MuxedAccount::Ed25519(_)) || parsed.to_string() != value {
        bail!("governance controller must be a canonical G-address");
    }
    Ok(())
}

fn validate_digest(value: &str, field: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{field} must be a lowercase 32-byte hex digest");
    }
    Ok(())
}

fn validate_json_payload(value: &serde_json::Value, depth: usize) -> Result<()> {
    if depth > MAX_OPERATION_PAYLOAD_DEPTH {
        bail!("activity operation payload exceeds the supported depth");
    }
    match value {
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
        serde_json::Value::String(value) => {
            if value.len() > 8_192 {
                bail!("activity operation payload string is too long");
            }
        }
        serde_json::Value::Array(values) => {
            if values.len() > 256 {
                bail!("activity operation payload array is too large");
            }
            for value in values {
                validate_json_payload(value, depth + 1)?;
            }
        }
        serde_json::Value::Object(values) => {
            if values.len() > 256 {
                bail!("activity operation payload object is too large");
            }
            for (key, value) in values {
                if key.is_empty() || key.len() > 128 {
                    bail!("activity operation payload key is invalid");
                }
                validate_json_payload(value, depth + 1)?;
            }
        }
    }
    Ok(())
}

fn validate_label(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 128 {
        bail!("{field} must be a bounded non-empty string");
    }
    Ok(())
}

fn parse_timestamp(value: &str, field: &str) -> Result<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).with_context(|| format!("invalid {field}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    const ACCOUNT: &str = "GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS";
    const SIGNER_A: &str = "GA2YKNLNRAMQ6WXF6D2PD6FKTHFADDYFLJ5N3QGSYG752YFSCGV4I5N7";
    const SIGNER_B: &str = "GDDILLHDLKEB4A7NASCSAJ65V3XAIIEZQK5I6MAJQ3FTVYQYFQGBHXJH";

    #[test]
    fn missing_baseline_is_explicitly_unavailable() {
        let directory = TempDir::new().unwrap();
        let response = load_governance_monitor_at(
            Some(directory.path()),
            OffsetDateTime::parse("2026-07-27T20:10:00Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        assert!(!response.available);
        assert_eq!(response.controller_count, 0);
    }

    #[test]
    fn valid_snapshot_is_loaded_and_becomes_stale_after_twenty_minutes() {
        let directory = TempDir::new().unwrap();
        let policy = strict_policy();
        fs::write(
            directory.path().join(CURRENT_FILENAME),
            serde_json::to_vec(&json!({
                "schemaVersion": 1,
                "network": "public",
                "observedAt": "2026-07-27T20:00:00Z",
                "registry": {"observedAt": "2026-07-27T19:00:00Z", "observationLedger": 1},
                "accounts": [{
                    "accountId": ACCOUNT,
                    "protocol": "Aquarius",
                    "role": "root-admin",
                    "sources": ["https://example.test/source"],
                    "lastModifiedLedger": 63_676_000,
                    "policy": policy
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            directory.path().join(ALERTS_FILENAME),
            b"{\"schemaVersion\":1,\"alerts\":[]}",
        )
        .unwrap();

        let fresh = load_governance_monitor_at(
            Some(directory.path()),
            OffsetDateTime::parse("2026-07-27T20:19:59Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        assert!(fresh.available);
        assert_eq!(fresh.stale, Some(false));
        assert_eq!(fresh.controller_count, 1);

        let stale = load_governance_monitor_at(
            Some(directory.path()),
            OffsetDateTime::parse("2026-07-27T20:20:01Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        assert_eq!(stale.stale, Some(true));
    }

    #[test]
    fn controller_activity_payload_is_served_and_source_mismatches_fail_closed() {
        let directory = TempDir::new().unwrap();
        let policy = strict_policy();
        fs::write(
            directory.path().join(CURRENT_FILENAME),
            serde_json::to_vec(&json!({
                "schemaVersion": 1,
                "network": "public",
                "observedAt": "2026-07-27T20:00:00Z",
                "registry": {"observedAt": "2026-07-27T19:00:00Z", "observationLedger": 1},
                "accounts": [{
                    "accountId": ACCOUNT,
                    "protocol": "Aquarius",
                    "role": "root-admin",
                    "sources": ["https://example.test/source"],
                    "lastModifiedLedger": 63_676_000,
                    "operationCursor": "273796412704575582",
                    "policy": policy
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        let activity = |source_account: &str| {
            json!({
                "schemaVersion": 1,
                "alerts": [{
                    "id": "44".repeat(32),
                    "type": "classic-controller-transaction-executed",
                    "severity": "high",
                    "detectedAt": "2026-07-27T20:00:00Z",
                    "occurredAt": "2026-07-27T19:59:30Z",
                    "accountId": ACCOUNT,
                    "protocol": "Aquarius",
                    "role": "root-admin",
                    "transactionHash": "55".repeat(32),
                    "operations": [{
                        "id": "273796412704575582",
                        "type": "manage_data",
                        "typeCode": 10,
                        "sourceAccount": source_account,
                        "payload": {"name": "emergency-mode", "value": "enabled"}
                    }]
                }]
            })
        };
        fs::write(
            directory.path().join(ALERTS_FILENAME),
            serde_json::to_vec(&activity(ACCOUNT)).unwrap(),
        )
        .unwrap();

        let response = load_governance_monitor_at(
            Some(directory.path()),
            OffsetDateTime::parse("2026-07-27T20:01:00Z", &Rfc3339).unwrap(),
        )
        .unwrap();
        assert_eq!(response.alerts.len(), 1);
        assert!(matches!(
            &response.alerts[0],
            GovernanceAlert::Activity(alert)
                if alert.transaction_hash == "55".repeat(32)
                    && alert.operations[0].payload["name"] == "emergency-mode"
        ));

        fs::write(
            directory.path().join(ALERTS_FILENAME),
            serde_json::to_vec(&activity(SIGNER_A)).unwrap(),
        )
        .unwrap();
        let error = load_governance_monitor_at(
            Some(directory.path()),
            OffsetDateTime::parse("2026-07-27T20:01:00Z", &Rfc3339).unwrap(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("source does not match"));
    }

    fn strict_policy() -> serde_json::Value {
        let mut signers = vec![SIGNER_A, ACCOUNT, SIGNER_B];
        signers.sort_unstable();
        json!({
            "thresholds": {"low": 2, "medium": 2, "high": 2},
            "masterWeight": 1,
            "signers": signers.into_iter().map(|key| json!({
                "key": key,
                "type": "ed25519_public_key",
                "weight": 1
            })).collect::<Vec<_>>(),
            "classification": "strict"
        })
    }
}
