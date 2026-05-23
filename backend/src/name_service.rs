use crate::{context::SharedState, database::AddressLabelRecord, models::now_ts};
use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::Deserialize;
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use std::{
    future::Future,
    pin::Pin,
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::{debug, warn};

const NAME_PROGRAM_ID: &str = "namesLPneVptA9Z5rqUDD9tMTWEJwofgaYwp8cawRkX";
const ROOT_DOMAIN_ACCOUNT: &str = "58PwtjSDuFHuUkYjH9BYnnQKHfwo9reZhC2zMJv9JPkx";
const REVERSE_LOOKUP_CLASS: &str = "33m47vH6Eav6jr5Ry86XjhRft2jRBLDnDgPSHoquXi2Z";
const PRIMARY_DOMAIN_PROGRAM_ID: &str = "85iDfUvr3HJyLM2zcq5BXSiDvUWfw6cSE1FfNBo8Ap29";
const PRIMARY_DOMAIN_SEED: &[u8] = b"favourite_domain";
const HASH_PREFIX: &str = "SPL Name Service";

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq)]
pub enum NameRegistryInstruction {
    Create {
        hashed_name: Vec<u8>,
        lamports: u64,
        space: u32,
    },
    Update {
        offset: u32,
        data: Vec<u8>,
    },
    Transfer {
        new_owner: Pubkey,
    },
    Delete,
    Realloc {
        space: u32,
    },
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize, PartialEq)]
pub struct NameRecordHeader {
    pub parent_name: Pubkey,
    pub owner: Pubkey,
    pub class: Pubkey,
}

impl NameRecordHeader {
    pub const LEN: usize = 96;
}

#[derive(Clone, Debug)]
pub enum DomainInstructionKind {
    Create { space: u32, lamports: u64 },
    Update { offset: u32, data_len: usize },
    Transfer { new_owner: String },
    Delete { refund_target: String },
    Realloc { space: u32 },
}

#[derive(Clone, Debug)]
pub struct DomainInstructionEvent {
    pub target_account: Pubkey,
    pub target_name: String,
    pub owner: String,
    pub class: String,
    pub parent: Option<String>,
    pub root_domain: bool,
    pub kind: DomainInstructionKind,
}

#[derive(Clone, Debug)]
pub struct DomainAccountResolution {
    pub name_account: Pubkey,
    pub display_name: String,
    pub owner: Pubkey,
    pub class: Pubkey,
    pub parent: Option<Pubkey>,
    pub root_domain: bool,
}

#[derive(Clone, Debug)]
pub enum AddressLabel {
    DomainAccount {
        label: String,
        domain_account: Pubkey,
    },
    PrimaryDomain {
        label: String,
        domain_account: Pubkey,
    },
}

impl AddressLabel {
    pub fn label(&self) -> &str {
        match self {
            Self::DomainAccount { label, .. } | Self::PrimaryDomain { label, .. } => label,
        }
    }

    pub fn source(&self) -> &'static str {
        match self {
            Self::DomainAccount { .. } => "sns_reverse_lookup",
            Self::PrimaryDomain { .. } => "sns_primary_domain",
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::DomainAccount { .. } => "domain_account",
            Self::PrimaryDomain { .. } => "wallet_primary_domain",
        }
    }

    pub fn reference_address(&self) -> Option<String> {
        match self {
            Self::DomainAccount { domain_account, .. }
            | Self::PrimaryDomain { domain_account, .. } => Some(domain_account.to_string()),
        }
    }
}

#[derive(Clone, Debug)]
struct AccountData {
    owner: Pubkey,
    data: Vec<u8>,
}

#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    result: T,
}

#[derive(Deserialize)]
struct AccountInfoResult {
    value: Option<AccountInfoValue>,
}

#[derive(Deserialize)]
struct AccountInfoValue {
    data: AccountDataValue,
    #[serde(rename = "lamports")]
    _lamports: u64,
    owner: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AccountDataValue {
    Binary((String, String)),
}

#[derive(BorshDeserialize)]
struct PrimaryDomainRecord {
    tag: u8,
    name_account: [u8; 32],
}

pub fn program_id() -> Pubkey {
    static KEY: OnceLock<Pubkey> = OnceLock::new();
    *KEY.get_or_init(|| Pubkey::from_str(NAME_PROGRAM_ID).expect("valid sns name program id"))
}

pub fn root_domain_account() -> Pubkey {
    static KEY: OnceLock<Pubkey> = OnceLock::new();
    *KEY.get_or_init(|| Pubkey::from_str(ROOT_DOMAIN_ACCOUNT).expect("valid sns root domain"))
}

pub fn reverse_lookup_class() -> Pubkey {
    static KEY: OnceLock<Pubkey> = OnceLock::new();
    *KEY.get_or_init(|| Pubkey::from_str(REVERSE_LOOKUP_CLASS).expect("valid reverse lookup class"))
}

pub fn primary_domain_program_id() -> Pubkey {
    static KEY: OnceLock<Pubkey> = OnceLock::new();
    *KEY.get_or_init(|| {
        Pubkey::from_str(PRIMARY_DOMAIN_PROGRAM_ID).expect("valid primary domain program id")
    })
}

pub async fn run_indexer_loop(state: SharedState) -> Result<()> {
    loop {
        let started = Instant::now();
        if let Err(error) = sync_recent_activity_labels(state.clone(), 96).await {
            warn!(error = %error, "failed to sync indexed SNS labels");
        }

        let elapsed = started.elapsed();
        debug!(
            refreshed_in_ms = elapsed.as_millis() as u64,
            "completed SNS index sync pass",
        );

        sleep(Duration::from_secs(state.config.sns_refresh_seconds)).await;
    }
}

pub async fn hydrate_event(
    state: &SharedState,
    mut event: crate::models::ChainEvent,
) -> crate::models::ChainEvent {
    if let Some(label) = cached_address_label(state, &event.target_address)
        .await
        .unwrap_or(None)
    {
        let payload = event
            .payload
            .as_object_mut()
            .expect("event payload is always an object");
        payload
            .entry("target_name".to_owned())
            .or_insert_with(|| json!(label.label()));
    }

    if let Some(actor) = event.actor.clone() {
        if let Some(label) = cached_address_label(state, &actor).await.unwrap_or(None) {
            let payload = event
                .payload
                .as_object_mut()
                .expect("event payload is always an object");
            payload
                .entry("actor_name".to_owned())
                .or_insert_with(|| json!(label.label()));
        }
    }

    for (address_key, label_key) in [
        ("member", "member_name"),
        ("multisig", "multisig_name"),
        ("decoded_program", "decoded_program_name"),
        ("owner", "owner_name"),
        ("new_owner", "new_owner_name"),
        ("refund_target", "refund_target_name"),
    ] {
        if let Some(address) = event
            .payload
            .get(address_key)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
        {
            if let Some(label) = cached_address_label(state, &address).await.unwrap_or(None) {
                let payload = event
                    .payload
                    .as_object_mut()
                    .expect("event payload is always an object");
                payload
                    .entry(label_key.to_owned())
                    .or_insert_with(|| json!(label.label()));
            }
        }
    }

    event
}

pub async fn hydrate_event_cached(
    state: &SharedState,
    mut event: crate::models::ChainEvent,
) -> crate::models::ChainEvent {
    if let Some(label) = cached_address_label(state, &event.target_address)
        .await
        .unwrap_or(None)
    {
        let payload = event
            .payload
            .as_object_mut()
            .expect("event payload is always an object");
        payload
            .entry("target_name".to_owned())
            .or_insert_with(|| json!(label.label()));
    }

    if let Some(actor) = event.actor.clone() {
        if let Some(label) = cached_address_label(state, &actor).await.unwrap_or(None) {
            let payload = event
                .payload
                .as_object_mut()
                .expect("event payload is always an object");
            payload
                .entry("actor_name".to_owned())
                .or_insert_with(|| json!(label.label()));
        }
    }

    for (address_key, label_key) in [
        ("member", "member_name"),
        ("multisig", "multisig_name"),
        ("decoded_program", "decoded_program_name"),
        ("owner", "owner_name"),
        ("new_owner", "new_owner_name"),
        ("refund_target", "refund_target_name"),
    ] {
        if let Some(address) = event
            .payload
            .get(address_key)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
        {
            if let Some(label) = cached_address_label(state, &address).await.unwrap_or(None) {
                let payload = event
                    .payload
                    .as_object_mut()
                    .expect("event payload is always an object");
                payload
                    .entry(label_key.to_owned())
                    .or_insert_with(|| json!(label.label()));
            }
        }
    }

    event
}

pub async fn cached_address_label(
    state: &SharedState,
    address: &str,
) -> Result<Option<AddressLabel>> {
    if let Some(record) = state.db.address_label(address).await? {
        return Ok(Some(label_record_to_value(record)));
    }

    if let Some(record) = state.db.sns_domain(address).await? {
        return Ok(Some(AddressLabel::DomainAccount {
            label: record.domain_name,
            domain_account: Pubkey::from_str(&record.name_account).unwrap_or_default(),
        }));
    }

    if let Some(record) = state.db.sns_primary_domain(address).await? {
        return Ok(Some(AddressLabel::PrimaryDomain {
            label: record.domain_name,
            domain_account: Pubkey::from_str(&record.name_account).unwrap_or_default(),
        }));
    }

    Ok(None)
}

pub async fn bootstrap_address_label(
    state: &SharedState,
    address: &str,
) -> Result<Option<AddressLabel>> {
    if let Some(cached) = state.db.address_label(address).await? {
        return Ok(Some(label_record_to_value(cached)));
    }

    let pubkey = match Pubkey::from_str(address) {
        Ok(pubkey) => pubkey,
        Err(_) => return Ok(None),
    };

    if let Some(domain) = resolve_domain_account(state, pubkey).await? {
        let label = AddressLabel::DomainAccount {
            label: domain.display_name.clone(),
            domain_account: domain.name_account,
        };
        persist_domain_resolution(state, &domain, None, None).await?;
        persist_label(state, address, &label).await?;
        return Ok(Some(label));
    }

    if let Some(label) = resolve_primary_domain(state, pubkey).await? {
        persist_label(state, address, &label).await?;
        return Ok(Some(label));
    }

    Ok(None)
}

pub async fn parse_domain_instruction_event(
    state: &SharedState,
    tx: &crate::solana::ResolvedTransaction,
    instruction: &crate::solana::ResolvedInstruction,
    slot: Option<u64>,
) -> Result<Option<DomainInstructionEvent>> {
    let Some(parsed) = parse_instruction(&instruction.data) else {
        return Ok(None);
    };

    let target_account = match parsed {
        NameRegistryInstruction::Create { .. } => {
            crate::solana::instruction_account(tx, instruction, 2)?
        }
        NameRegistryInstruction::Update { .. }
        | NameRegistryInstruction::Transfer { .. }
        | NameRegistryInstruction::Delete
        | NameRegistryInstruction::Realloc { .. } => {
            crate::solana::instruction_account(tx, instruction, 0)?
        }
    };

    let cached_label = state
        .db
        .address_label(&target_account.to_string())
        .await?
        .map(label_record_to_value);
    let resolved_domain = resolve_domain_account(state, target_account).await?;

    let (target_name, owner, class, parent, root_domain) = match resolved_domain {
        Some(domain) => {
            persist_domain_resolution(
                state,
                &domain,
                slot,
                Some(domain_event_kind_for_instruction(&parsed)),
            )
            .await?;
            if domain.class == reverse_lookup_class() {
                return Ok(None);
            }

            (
                domain.display_name,
                domain.owner.to_string(),
                domain.class.to_string(),
                domain.parent.map(|value| value.to_string()),
                domain.root_domain,
            )
        }
        None => match (&parsed, cached_label) {
            (NameRegistryInstruction::Delete, Some(label)) => {
                let target_name = label.label().to_owned();
                let root_domain =
                    target_name.ends_with(".sol") && target_name.matches('.').count() == 1;
                state
                    .db
                    .mark_sns_domain_deleted(
                        &target_account.to_string(),
                        slot.map(|value| value as i64),
                        now_ts(),
                    )
                    .await?;
                (target_name, String::new(), String::new(), None, root_domain)
            }
            _ => return Ok(None),
        },
    };

    let kind = match parsed {
        NameRegistryInstruction::Create {
            lamports, space, ..
        } => DomainInstructionKind::Create { space, lamports },
        NameRegistryInstruction::Update { offset, data } => DomainInstructionKind::Update {
            offset,
            data_len: data.len(),
        },
        NameRegistryInstruction::Transfer { new_owner } => DomainInstructionKind::Transfer {
            new_owner: new_owner.to_string(),
        },
        NameRegistryInstruction::Delete => DomainInstructionKind::Delete {
            refund_target: crate::solana::instruction_account(tx, instruction, 2)?.to_string(),
        },
        NameRegistryInstruction::Realloc { space } => DomainInstructionKind::Realloc { space },
    };

    Ok(Some(DomainInstructionEvent {
        target_account,
        target_name,
        owner,
        class,
        parent,
        root_domain,
        kind,
    }))
}

pub fn parse_instruction(data: &[u8]) -> Option<NameRegistryInstruction> {
    NameRegistryInstruction::try_from_slice(data).ok()
}

async fn refresh_recent_labels(state: SharedState, limit: i64) -> Result<()> {
    let recent_addresses = state.db.recent_unlabeled_addresses(limit).await?;
    for address in recent_addresses {
        let _ = bootstrap_address_label(&state, &address).await?;
    }

    Ok(())
}

async fn sync_recent_activity_labels(state: SharedState, limit: i64) -> Result<()> {
    refresh_recent_labels(state, limit).await
}

async fn persist_label(state: &SharedState, address: &str, label: &AddressLabel) -> Result<()> {
    state
        .db
        .upsert_address_label(
            address,
            label.label(),
            label.source(),
            label.kind(),
            label.reference_address().as_deref(),
            now_ts(),
        )
        .await
}

async fn persist_domain_resolution(
    state: &SharedState,
    domain: &DomainAccountResolution,
    last_seen_slot: Option<u64>,
    last_event_kind: Option<&str>,
) -> Result<()> {
    let updated_at = now_ts();
    state
        .db
        .upsert_sns_domain(
            &domain.name_account.to_string(),
            &domain.display_name,
            Some(&domain.owner.to_string()),
            Some(&domain.class.to_string()),
            domain.parent.as_ref().map(Pubkey::to_string).as_deref(),
            domain.root_domain,
            last_seen_slot.map(|value| value as i64),
            last_event_kind,
            updated_at,
            None,
        )
        .await?;

    state
        .db
        .upsert_address_label(
            &domain.name_account.to_string(),
            &domain.display_name,
            "sns_domain_index",
            "domain_account",
            Some(&domain.name_account.to_string()),
            updated_at,
        )
        .await?;

    Ok(())
}

async fn persist_primary_domain_resolution(
    state: &SharedState,
    owner: Pubkey,
    domain: &DomainAccountResolution,
) -> Result<()> {
    let updated_at = now_ts();
    let owner_address = owner.to_string();
    let name_account = domain.name_account.to_string();

    state
        .db
        .upsert_sns_primary_domain(
            &owner_address,
            &name_account,
            &domain.display_name,
            updated_at,
        )
        .await?;
    state
        .db
        .upsert_address_label(
            &owner_address,
            &domain.display_name,
            "sns_primary_domain_index",
            "wallet_primary_domain",
            Some(&name_account),
            updated_at,
        )
        .await?;

    Ok(())
}

async fn clear_primary_domain_resolution(state: &SharedState, owner: Pubkey) -> Result<()> {
    let owner_address = owner.to_string();
    state.db.remove_sns_primary_domain(&owner_address).await?;
    state
        .db
        .delete_address_label_if_kind(&owner_address, "wallet_primary_domain")
        .await?;
    Ok(())
}

fn label_record_to_value(record: AddressLabelRecord) -> AddressLabel {
    match record.kind.as_str() {
        "wallet_primary_domain" => AddressLabel::PrimaryDomain {
            label: record.label,
            domain_account: record
                .reference_address
                .and_then(|value| Pubkey::from_str(&value).ok())
                .unwrap_or_default(),
        },
        _ => AddressLabel::DomainAccount {
            label: record.label,
            domain_account: record
                .reference_address
                .and_then(|value| Pubkey::from_str(&value).ok())
                .unwrap_or_default(),
        },
    }
}

async fn resolve_domain_account(
    state: &SharedState,
    name_account: Pubkey,
) -> Result<Option<DomainAccountResolution>> {
    resolve_domain_account_inner(state, name_account).await
}

fn resolve_domain_account_inner<'a>(
    state: &'a SharedState,
    name_account: Pubkey,
) -> Pin<Box<dyn Future<Output = Result<Option<DomainAccountResolution>>> + Send + 'a>> {
    Box::pin(async move {
        let Some(account) = fetch_account_data(state, name_account).await? else {
            return Ok(None);
        };

        if account.owner != program_id() || account.data.len() < NameRecordHeader::LEN {
            return Ok(None);
        }

        let header = NameRecordHeader::try_from_slice(&account.data[..NameRecordHeader::LEN])
            .map_err(|error| anyhow!("failed to parse name registry header: {error}"))?;

        if header.class == reverse_lookup_class() {
            return Ok(None);
        }

        let root = root_domain_account();
        let parent = if header.parent_name == root || header.parent_name == Pubkey::default() {
            None
        } else {
            Some(header.parent_name)
        };

        let reverse_key = reverse_lookup_key(name_account, parent.as_ref());
        let Some(reverse_account) = fetch_account_data(state, reverse_key).await? else {
            return Ok(None);
        };
        if reverse_account.owner != program_id()
            || reverse_account.data.len() < NameRecordHeader::LEN
        {
            return Ok(None);
        }

        let label = deserialize_reverse(
            &reverse_account.data[NameRecordHeader::LEN..],
            parent.is_some(),
        )
        .context("failed to decode SNS reverse record")?;
        let display_name = if let Some(parent_name_account) = parent {
            let Some(parent_resolution) =
                resolve_domain_account_inner(state, parent_name_account).await?
            else {
                return Ok(None);
            };
            format!("{label}.{}", parent_resolution.display_name)
        } else {
            format!("{label}.sol")
        };

        Ok(Some(DomainAccountResolution {
            name_account,
            display_name,
            owner: header.owner,
            class: header.class,
            parent,
            root_domain: header.parent_name == root,
        }))
    })
}

async fn resolve_primary_domain(
    state: &SharedState,
    owner: Pubkey,
) -> Result<Option<AddressLabel>> {
    let primary_account = primary_domain_key(owner);
    let Some(account) = fetch_account_data(state, primary_account).await? else {
        clear_primary_domain_resolution(state, owner).await?;
        return Ok(None);
    };

    let record = PrimaryDomainRecord::try_from_slice(&account.data)
        .map_err(|error| anyhow!("failed to parse SNS primary domain record: {error}"))?;
    if record.tag == 0 {
        clear_primary_domain_resolution(state, owner).await?;
        return Ok(None);
    }

    let domain_account = Pubkey::new_from_array(record.name_account);
    let Some(domain) = resolve_domain_account(state, domain_account).await? else {
        clear_primary_domain_resolution(state, owner).await?;
        return Ok(None);
    };

    if domain.owner != owner {
        clear_primary_domain_resolution(state, owner).await?;
        return Ok(None);
    }

    persist_domain_resolution(state, &domain, None, None).await?;
    persist_primary_domain_resolution(state, owner, &domain).await?;

    Ok(Some(AddressLabel::PrimaryDomain {
        label: domain.display_name,
        domain_account,
    }))
}

fn domain_event_kind_for_instruction(parsed: &NameRegistryInstruction) -> &'static str {
    match parsed {
        NameRegistryInstruction::Create { .. } => "name_service_domain_created",
        NameRegistryInstruction::Update { .. } => "name_service_domain_updated",
        NameRegistryInstruction::Transfer { .. } => "name_service_domain_transferred",
        NameRegistryInstruction::Delete => "name_service_domain_deleted",
        NameRegistryInstruction::Realloc { .. } => "name_service_domain_reallocated",
    }
}

fn reverse_lookup_key(name_account: Pubkey, parent: Option<&Pubkey>) -> Pubkey {
    let hashed_name = hash_name(&name_account.to_string());
    get_name_account_key(hashed_name, Some(&reverse_lookup_class()), parent)
}

fn primary_domain_key(owner: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[PRIMARY_DOMAIN_SEED, owner.as_ref()],
        &primary_domain_program_id(),
    )
    .0
}

fn hash_name(name: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(HASH_PREFIX.as_bytes());
    hasher.update(name.as_bytes());
    hasher.finalize().to_vec()
}

fn get_name_account_key(
    hashed_name: Vec<u8>,
    name_class: Option<&Pubkey>,
    parent: Option<&Pubkey>,
) -> Pubkey {
    let mut seeds = vec![hashed_name];
    seeds.push(name_class.copied().unwrap_or_default().to_bytes().to_vec());
    seeds.push(parent.copied().unwrap_or_default().to_bytes().to_vec());
    let seed_slices = seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
    Pubkey::find_program_address(&seed_slices, &program_id()).0
}

fn deserialize_reverse(data: &[u8], with_parent: bool) -> Result<String> {
    if data.len() < 4 {
        return Err(anyhow!("SNS reverse record payload is too short"));
    }

    let length = u32::from_le_bytes(
        data[..4]
            .try_into()
            .map_err(|_| anyhow!("SNS reverse record length prefix is invalid"))?,
    ) as usize;

    let bytes = data
        .get(4..4 + length)
        .ok_or_else(|| anyhow!("SNS reverse record payload is truncated"))?;
    let value = std::str::from_utf8(bytes)
        .context("SNS reverse record is not valid UTF-8")?
        .to_owned();

    if with_parent {
        Ok(value.trim_start_matches('\0').to_owned())
    } else {
        Ok(value)
    }
}

async fn fetch_account_data(state: &SharedState, address: Pubkey) -> Result<Option<AccountData>> {
    let response = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "getAccountInfo",
            "params": [
                address.to_string(),
                {
                    "commitment": "finalized",
                    "encoding": "base64"
                }
            ]
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Solana RPC returned {} while fetching {}",
            response.status(),
            address
        ));
    }

    let payload = response
        .json::<JsonRpcResponse<AccountInfoResult>>()
        .await
        .context("failed to decode Solana RPC account response")?;
    let Some(value) = payload.result.value else {
        return Ok(None);
    };

    let AccountDataValue::Binary((data, encoding)) = value.data;
    if encoding != "base64" {
        return Err(anyhow!(
            "unsupported Solana RPC account encoding: {encoding}"
        ));
    }

    let owner = Pubkey::from_str(&value.owner)
        .with_context(|| format!("invalid owner pubkey returned for account {address}"))?;
    let data = BASE64
        .decode(data)
        .with_context(|| format!("invalid base64 account payload returned for {address}"))?;

    Ok(Some(AccountData { owner, data }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_domain_names_with_the_sns_prefix() {
        let hashed_name = hash_name("bonfida");

        assert_eq!(hashed_name.len(), 32);
        assert_ne!(hashed_name, hash_name("bonfida.sol"));
    }

    #[test]
    fn strips_reverse_subdomain_prefix_for_child_labels() {
        let mut data = Vec::new();
        data.extend_from_slice(&(5_u32).to_le_bytes());
        data.extend_from_slice(b"\0team");

        assert_eq!(deserialize_reverse(&data, true).unwrap(), "team");
    }
}
