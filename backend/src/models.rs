use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;

pub mod event_kind {
    pub const NAME_SERVICE_DOMAIN_CREATED: &str = "name_service_domain_created";
    pub const NAME_SERVICE_DOMAIN_UPDATED: &str = "name_service_domain_updated";
    pub const NAME_SERVICE_DOMAIN_TRANSFERRED: &str = "name_service_domain_transferred";
    pub const NAME_SERVICE_DOMAIN_DELETED: &str = "name_service_domain_deleted";
    pub const NAME_SERVICE_DOMAIN_REALLOCATED: &str = "name_service_domain_reallocated";
    pub const PROGRAM_DEPLOYED: &str = "program_deployed";
    pub const PROGRAM_UPGRADED: &str = "program_upgraded";
    pub const PROGRAM_FINALIZED: &str = "program_finalized";
    pub const PROGRAM_MIGRATED_TO_LOADER_V4: &str = "program_migrated_to_loader_v4";
    pub const PROGRAM_IDL_INITIALIZED: &str = "program_idl_initialized";
    pub const PROGRAM_IDL_UPDATED: &str = "program_idl_updated";
    pub const PROGRAM_IDL_AUTHORITY_CHANGED: &str = "program_idl_authority_changed";
    pub const PROGRAM_IDL_IMMUTABLE: &str = "program_idl_immutable";
    pub const PROGRAM_IDL_RESIZED: &str = "program_idl_resized";
    pub const PROGRAM_IDL_CLOSED: &str = "program_idl_closed";
    pub const PROGRAM_ADMIN_INSTRUCTION: &str = "program_admin_instruction";
    pub const PROGRAM_METRIC_ALERT: &str = "program_metric_alert";
    pub const PROGRAM_UNKNOWN_INSTRUCTION: &str = "program_unknown_instruction";
    pub const MULTISIG_PROPOSAL_ACTIVATED: &str = "multisig_proposal_activated";
    pub const MULTISIG_PROPOSAL_APPROVED: &str = "multisig_proposal_approved";
    pub const MULTISIG_PROPOSAL_REJECTED: &str = "multisig_proposal_rejected";
    pub const MULTISIG_PROPOSAL_CANCELLED: &str = "multisig_proposal_cancelled";
    pub const MULTISIG_VAULT_EXECUTED: &str = "multisig_vault_executed";
    pub const MULTISIG_CONFIG_EXECUTED: &str = "multisig_config_executed";
}

pub mod target_type {
    pub const DOMAIN: &str = "domain";
    pub const PROGRAM: &str = "program";
    pub const MULTISIG: &str = "multisig";
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TelegramEventScope {
    ProgramUpdates,
    MultisigActivity,
    DefiWatch,
}

impl TelegramEventScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ProgramUpdates => "program_updates",
            Self::MultisigActivity => "multisig_activity",
            Self::DefiWatch => "defi_watch",
        }
    }

    pub fn matches_event(&self, event: &ChainEvent) -> bool {
        match self {
            Self::ProgramUpdates => {
                event.target_type == target_type::PROGRAM && is_program_update_kind(&event.kind)
            }
            Self::MultisigActivity => {
                event.target_type == target_type::MULTISIG && is_multisig_activity_kind(&event.kind)
            }
            Self::DefiWatch => is_default_defi_watch_event(event),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolCoverage {
    AddressOnly,
    AdminSignals,
    DeepDecode,
}

impl FromStr for TelegramEventScope {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "program_updates" => Ok(Self::ProgramUpdates),
            "multisig_activity" => Ok(Self::MultisigActivity),
            "defi_watch" => Ok(Self::DefiWatch),
            _ => Err("unsupported telegram event scope"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainEvent {
    pub id: String,
    pub signature: String,
    pub instruction_path: String,
    pub kind: String,
    pub target_type: String,
    pub target_address: String,
    pub actor: Option<String>,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub indexed_at: i64,
    pub summary: String,
    pub tags: Vec<String>,
    pub payload: Value,
    pub delivery_attempts: i64,
    pub delivered_at: Option<i64>,
    pub last_error: Option<String>,
    pub next_retry_at: i64,
}

impl ChainEvent {
    pub fn has_tag(&self, needle: &str) -> bool {
        self.tags.iter().any(|tag| tag == needle)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventsSummary {
    pub total_count: i64,
    pub delivered_count: i64,
    pub pending_delivery_count: i64,
    pub first_indexed_slot: Option<u64>,
    pub backfill_start_slot: Option<u64>,
    pub last_processed_slot: Option<u64>,
    pub processed_transaction_count: i64,
    pub matched_transaction_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventScopeCounts {
    pub all: i64,
    pub critical: i64,
    pub defi: i64,
    pub programs: i64,
    pub idl: i64,
    pub multisig: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventsResponse {
    pub items: Vec<ChainEvent>,
    pub summary: EventsSummary,
    pub total_count: i64,
    pub scope_counts: EventScopeCounts,
    pub next_offset: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelegramSubscription {
    pub id: String,
    pub label: String,
    pub chat_id: String,
    pub thread_id: Option<i64>,
    pub scope: TelegramEventScope,
    pub target_address: Option<String>,
    pub required_tags: Vec<String>,
    pub created_at: i64,
}

impl TelegramSubscription {
    pub fn matches_event(&self, event: &ChainEvent) -> bool {
        if self.scope == TelegramEventScope::MultisigActivity {
            if event.target_type != target_type::MULTISIG
                || !is_multisig_activity_kind(event.kind.as_str())
            {
                return false;
            }

            if self.target_address.is_none() && !is_relevant_multisig_activity_event(event) {
                return false;
            }
        } else if !self.scope.matches_event(event) {
            return false;
        }

        match self.target_address.as_deref() {
            Some(address) if address != event.target_address => return false,
            _ => {}
        }

        self.required_tags
            .iter()
            .all(|required_tag| event.has_tag(required_tag))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelegramSubscriptionsResponse {
    pub items: Vec<TelegramSubscription>,
    pub telegram_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelegramSubscriptionSummaryResponse {
    pub subscription_count: i64,
    pub telegram_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateTelegramSubscriptionRequest {
    pub label: String,
    pub chat_id: String,
    pub thread_id: Option<i64>,
    pub scope: TelegramEventScope,
    pub target_address: Option<String>,
    pub required_tags: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefiProtocolProgramDescriptor {
    pub label: String,
    pub program_id: String,
    pub idl_metadata_address: String,
    pub anchor_legacy_idl_address: Option<String>,
    #[serde(default)]
    pub risk_summary: ProgramRiskSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramRiskSummary {
    pub critical_total: i64,
    pub reviewed_critical: i64,
    pub unreviewed_critical: i64,
    pub score: i64,
    pub grade: String,
}

impl Default for ProgramRiskSummary {
    fn default() -> Self {
        Self::from_counts(0, 0)
    }
}

impl ProgramRiskSummary {
    pub fn from_counts(reviewed_critical: i64, unreviewed_critical: i64) -> Self {
        let score = (100 - (unreviewed_critical * 20)).clamp(0, 100);
        let grade = match score {
            90..=100 => "A",
            75..=89 => "B",
            60..=74 => "C",
            40..=59 => "D",
            _ => "F",
        };

        Self {
            critical_total: reviewed_critical + unreviewed_critical,
            reviewed_critical,
            unreviewed_critical,
            score,
            grade: grade.to_owned(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AmountTriggerDescriptor {
    pub label: String,
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefiProtocolDescriptor {
    pub slug: String,
    pub name: String,
    pub program_id: String,
    pub programs: Vec<DefiProtocolProgramDescriptor>,
    pub sol_domains: Vec<String>,
    pub coverage: ProtocolCoverage,
    pub catalog_instruction_count: usize,
    pub admin_instruction_filters: Vec<String>,
    pub high_impact_instruction_filters: Vec<String>,
    pub amount_triggers: Vec<AmountTriggerDescriptor>,
    pub unknown_alerts: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DefiProtocolsResponse {
    pub items: Vec<DefiProtocolDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramSignerStat {
    pub member: String,
    pub member_label: Option<String>,
    pub actions: i64,
    pub last_slot: i64,
    pub multisig_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramSignerStatsResponse {
    pub items: Vec<ProgramSignerStat>,
}

#[derive(Clone, Debug)]
pub struct ProgramDataWrite {
    pub id: String,
    pub signature: String,
    pub instruction_path: String,
    pub instruction_order: String,
    pub loader: String,
    pub write_account: String,
    pub authority: Option<String>,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub indexed_at: i64,
    pub offset: i64,
    pub bytes: Vec<u8>,
    pub bytes_sha256: String,
}

#[derive(Clone, Debug)]
pub struct ProgramIdlDataWrite {
    pub id: String,
    pub signature: String,
    pub instruction_path: String,
    pub instruction_order: String,
    pub origin: String,
    pub metadata_account: String,
    pub source_account: String,
    pub program_address: String,
    pub authority: Option<String>,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub indexed_at: i64,
    pub offset: Option<i64>,
    pub bytes: Vec<u8>,
    pub bytes_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProgramByteRange {
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramVersionSummary {
    pub id: String,
    pub event_id: String,
    pub program_address: String,
    pub loader: String,
    pub source_account: String,
    pub slot: i64,
    pub instruction_path: String,
    pub complete: bool,
    pub chunk_count: i64,
    pub program_len: Option<i64>,
    pub program_sha256: Option<String>,
    pub missing_ranges: Vec<ProgramByteRange>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramVersionRecord {
    pub id: String,
    pub event_id: String,
    pub signature: String,
    pub instruction_path: String,
    pub kind: String,
    pub program_address: String,
    pub loader: String,
    pub source_account: String,
    pub authority: Option<String>,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub indexed_at: i64,
    pub complete: bool,
    pub chunk_count: i64,
    pub program_len: Option<i64>,
    pub program_sha256: Option<String>,
    pub missing_ranges: Vec<ProgramByteRange>,
    pub previous_version_id: Option<String>,
    pub previous_program_len: Option<i64>,
    pub previous_program_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramVersionsResponse {
    pub items: Vec<ProgramVersionRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramIdlVersionRecord {
    pub id: String,
    pub event_id: String,
    pub signature: String,
    pub instruction_path: String,
    pub kind: String,
    pub origin: String,
    pub program_address: String,
    pub metadata_account: String,
    pub source_account: String,
    pub authority: Option<String>,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub indexed_at: i64,
    pub complete: bool,
    pub chunk_count: i64,
    pub idl_len: Option<i64>,
    pub idl_sha256: Option<String>,
    pub encoding: Option<String>,
    pub compression: Option<String>,
    pub format: Option<String>,
    pub data_source: Option<String>,
    pub missing_ranges: Vec<ProgramByteRange>,
    pub previous_idl_version_id: Option<String>,
    pub previous_idl_len: Option<i64>,
    pub previous_idl_sha256: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramIdlVersionsResponse {
    pub items: Vec<ProgramIdlVersionRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramDisassemblyRecord {
    pub version_id: String,
    pub program_address: String,
    pub program_sha256: String,
    pub format: String,
    pub instruction_count: i64,
    pub entrypoint_instruction: Option<i64>,
    pub asm_sha256: String,
    pub asm_text: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProgramDisassemblyResponse {
    pub item: ProgramDisassemblyRecord,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProtocolMetricObservation {
    pub id: String,
    pub signature: String,
    pub instruction_path: String,
    pub slot: i64,
    pub block_time: Option<i64>,
    pub protocol_slug: String,
    pub protocol_name: String,
    pub program_id: String,
    pub program_label: String,
    pub instruction_name: String,
    pub metric_key: String,
    pub metric_label: String,
    pub metric_value: f64,
    pub subject_address: Option<String>,
    pub subject_kind: Option<String>,
    pub subject_label: Option<String>,
    pub payload: Value,
    pub observed_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricAlertRule {
    pub id: String,
    pub label: String,
    pub protocol_slug: String,
    pub protocol_name: String,
    pub instruction_name: String,
    pub metric_key: String,
    pub metric_label: String,
    pub subject_address: Option<String>,
    pub subject_kind: Option<String>,
    pub subject_label: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateMetricAlertRuleRequest {
    pub label: String,
    pub protocol_slug: String,
    pub instruction_name: String,
    pub metric_key: String,
    pub subject_address: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricAlertPreset {
    pub protocol_slug: String,
    pub protocol_name: String,
    pub instruction_name: String,
    pub metric_key: String,
    pub metric_label: String,
    pub subject_address: Option<String>,
    pub subject_kind: Option<String>,
    pub subject_label: Option<String>,
    pub latest_value: f64,
    pub suggested_min_value: Option<f64>,
    pub suggested_max_value: Option<f64>,
    pub observation_count: usize,
    pub latest_slot: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetricAlertsResponse {
    pub presets: Vec<MetricAlertPreset>,
    pub rules: Vec<MetricAlertRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TelegramBotProfile {
    pub id: i64,
    pub first_name: String,
    pub username: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveStatusResponse {
    pub summary: EventsSummary,
    pub telegram_enabled: bool,
    pub telegram_bot: Option<TelegramBotProfile>,
    pub sampled_at_ms: i64,
    pub estimated_slot_time_ms: i64,
}

#[derive(Clone, Debug)]
pub struct PendingTelegramDelivery {
    pub id: String,
    pub event: ChainEvent,
    pub subscription: TelegramSubscription,
    pub delivery_attempts: i64,
}

pub fn now_ts() -> i64 {
    Utc::now().timestamp()
}

pub fn now_ts_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn is_program_update_kind(kind: &str) -> bool {
    matches!(
        kind,
        event_kind::PROGRAM_DEPLOYED
            | event_kind::PROGRAM_UPGRADED
            | event_kind::PROGRAM_FINALIZED
            | event_kind::PROGRAM_MIGRATED_TO_LOADER_V4
            | event_kind::PROGRAM_IDL_INITIALIZED
            | event_kind::PROGRAM_IDL_UPDATED
            | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
            | event_kind::PROGRAM_IDL_IMMUTABLE
            | event_kind::PROGRAM_IDL_RESIZED
            | event_kind::PROGRAM_IDL_CLOSED
    )
}

fn is_multisig_activity_kind(kind: &str) -> bool {
    matches!(
        kind,
        event_kind::MULTISIG_PROPOSAL_ACTIVATED
            | event_kind::MULTISIG_PROPOSAL_APPROVED
            | event_kind::MULTISIG_PROPOSAL_REJECTED
            | event_kind::MULTISIG_PROPOSAL_CANCELLED
            | event_kind::MULTISIG_VAULT_EXECUTED
            | event_kind::MULTISIG_CONFIG_EXECUTED
    )
}

fn is_relevant_multisig_activity_event(event: &ChainEvent) -> bool {
    event.has_tag("multisig:project_linked") || event.has_tag("severity:critical")
}

fn is_default_defi_watch_event(event: &ChainEvent) -> bool {
    if !event.has_tag("watch:defi") {
        return false;
    }

    match event.kind.as_str() {
        event_kind::PROGRAM_DEPLOYED
        | event_kind::PROGRAM_UPGRADED
        | event_kind::PROGRAM_FINALIZED
        | event_kind::PROGRAM_MIGRATED_TO_LOADER_V4
        | event_kind::PROGRAM_UNKNOWN_INSTRUCTION
        | event_kind::PROGRAM_METRIC_ALERT => true,
        event_kind::NAME_SERVICE_DOMAIN_CREATED
        | event_kind::NAME_SERVICE_DOMAIN_UPDATED
        | event_kind::NAME_SERVICE_DOMAIN_TRANSFERRED
        | event_kind::NAME_SERVICE_DOMAIN_DELETED
        | event_kind::NAME_SERVICE_DOMAIN_REALLOCATED => {
            event.has_tag("watch:critical_domain") || event.has_tag("domain:project_linked")
        }
        event_kind::PROGRAM_ADMIN_INSTRUCTION
        | event_kind::PROGRAM_IDL_INITIALIZED
        | event_kind::PROGRAM_IDL_UPDATED
        | event_kind::PROGRAM_IDL_AUTHORITY_CHANGED
        | event_kind::PROGRAM_IDL_IMMUTABLE
        | event_kind::PROGRAM_IDL_RESIZED
        | event_kind::PROGRAM_IDL_CLOSED => event.has_tag("impact:high"),
        event_kind::MULTISIG_PROPOSAL_ACTIVATED
        | event_kind::MULTISIG_PROPOSAL_APPROVED
        | event_kind::MULTISIG_PROPOSAL_REJECTED
        | event_kind::MULTISIG_PROPOSAL_CANCELLED
        | event_kind::MULTISIG_VAULT_EXECUTED
        | event_kind::MULTISIG_CONFIG_EXECUTED => {
            event.has_tag("multisig:project_linked") && has_decoded_multisig_action(event)
        }
        _ => false,
    }
}

fn has_decoded_multisig_action(event: &ChainEvent) -> bool {
    ["decoded_protocol", "decoded_program", "decoded_kind"]
        .iter()
        .any(|key| {
            event
                .payload
                .get(key)
                .and_then(|value| value.as_str())
                .is_some_and(|value| !value.trim().is_empty())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_multisig_event(kind: &str, tags: &[&str]) -> ChainEvent {
        ChainEvent {
            id: format!("test:{kind}"),
            signature: "test-signature".to_owned(),
            instruction_path: "0".to_owned(),
            kind: kind.to_owned(),
            target_type: target_type::MULTISIG.to_owned(),
            target_address: "test-multisig".to_owned(),
            actor: Some("test-member".to_owned()),
            slot: 1,
            block_time: None,
            indexed_at: 1,
            summary: "test event".to_owned(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            payload: serde_json::json!({}),
            delivery_attempts: 0,
            delivered_at: None,
            last_error: None,
            next_retry_at: 1,
        }
    }

    fn sample_multisig_subscription(target_address: Option<&str>) -> TelegramSubscription {
        TelegramSubscription {
            id: "test-subscription".to_owned(),
            label: "test".to_owned(),
            chat_id: "1".to_owned(),
            thread_id: None,
            scope: TelegramEventScope::MultisigActivity,
            target_address: target_address.map(str::to_owned),
            required_tags: Vec::new(),
            created_at: 1,
        }
    }

    #[test]
    fn defi_watch_ignores_project_linked_multisig_activity_without_decoded_action() {
        let event = sample_multisig_event(
            event_kind::MULTISIG_PROPOSAL_APPROVED,
            &[
                "protocol:squads",
                "protocol:ferno",
                "watch:defi",
                "multisig:project_linked",
            ],
        );

        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
    }

    #[test]
    fn defi_watch_keeps_project_linked_multisig_execution_with_decoded_action() {
        let mut event = sample_multisig_event(
            event_kind::MULTISIG_VAULT_EXECUTED,
            &[
                "protocol:squads",
                "protocol:ferno",
                "watch:defi",
                "multisig:project_linked",
            ],
        );
        event.payload = serde_json::json!({
            "decoded_program": "test-program",
            "decoded_kind": "program_upgraded",
        });

        assert!(TelegramEventScope::DefiWatch.matches_event(&event));
    }

    #[test]
    fn defi_watch_ignores_plain_squads_multisig_proposals() {
        let event = sample_multisig_event(
            event_kind::MULTISIG_PROPOSAL_APPROVED,
            &["protocol:squads", "signal:multisig", "source:squads"],
        );

        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
    }

    #[test]
    fn defi_watch_ignores_unlinked_squads_config_activity() {
        let event = sample_multisig_event(
            event_kind::MULTISIG_CONFIG_EXECUTED,
            &[
                "protocol:squads",
                "signal:multisig",
                "source:squads",
                "watch:defi",
                "impact:high",
            ],
        );

        assert!(!TelegramEventScope::DefiWatch.matches_event(&event));
        assert!(!sample_multisig_subscription(None).matches_event(&event));
    }

    #[test]
    fn broad_multisig_subscription_ignores_plain_squads_activity() {
        let event = sample_multisig_event(
            event_kind::MULTISIG_PROPOSAL_APPROVED,
            &["protocol:squads", "signal:multisig", "source:squads"],
        );

        assert!(!sample_multisig_subscription(None).matches_event(&event));
    }

    #[test]
    fn targeted_multisig_subscription_keeps_plain_squads_activity() {
        let event = sample_multisig_event(
            event_kind::MULTISIG_PROPOSAL_APPROVED,
            &["protocol:squads", "signal:multisig", "source:squads"],
        );

        assert!(sample_multisig_subscription(Some("test-multisig")).matches_event(&event));
    }
}
