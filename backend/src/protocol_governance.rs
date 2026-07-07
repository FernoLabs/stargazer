use crate::{context::SharedState, database::ProtocolGovernanceLinkWrite, protocols};
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;
use serde_json::{Value, json};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::Duration,
};
use tokio::time::{sleep, timeout};

const SOURCE_REALMS_ACCOUNT_INDEX: &str = "realms_account_index";
const DEFAULT_RPC_TIMEOUT_SECONDS: u64 = 120;
const RPC_RETRY_ATTEMPTS: usize = 5;
const REALM_V1_TAG: u8 = 1;
const REALM_V2_TAG: u8 = 16;
const GOVERNANCE_TAGS: &[u8] = &[3, 4, 9, 10, 18, 19, 20, 21];

#[derive(Clone, Debug)]
struct RealmAccount {
    program_id: Pubkey,
    address: String,
    name: String,
    community_mint: String,
    council_mint: Option<String>,
    authority: Option<String>,
}

#[derive(Clone, Debug)]
struct GovernanceAccount {
    program_id: Pubkey,
    address: String,
    account_type: u8,
    realm: String,
    governed_account: String,
}

#[derive(Clone, Debug, Default)]
pub struct ProtocolGovernanceSyncReport {
    pub governance_program_count: usize,
    pub realm_count: usize,
    pub governance_count: usize,
    pub matched_realm_count: usize,
    pub link_count: usize,
    pub inserted_count: usize,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
struct RpcProgramAccount {
    pubkey: String,
    account: RpcAccount,
}

#[derive(Clone, Debug, Deserialize)]
struct RpcAccount {
    owner: String,
    data: RpcAccountData,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum RpcAccountData {
    Encoded(Vec<String>),
    Raw(String),
}

pub async fn sync_protocol_governance_realms(
    state: SharedState,
) -> Result<ProtocolGovernanceSyncReport> {
    let governance_program_ids = protocols::realms_governance_program_ids();
    let mut realms = Vec::new();
    let mut governances = Vec::new();

    for program_id in &governance_program_ids {
        realms.extend(fetch_realms_for_program(&state, *program_id).await?);
        governances.extend(fetch_governances_for_program(&state, *program_id).await?);
    }

    let links = protocol_governance_links(&realms, &governances);
    let matched_realm_count = links
        .iter()
        .map(|link| link.realm.as_str())
        .collect::<HashSet<_>>()
        .len();
    let inserted_count = state
        .db
        .replace_protocol_governance_links(SOURCE_REALMS_ACCOUNT_INDEX, &links)
        .await?;

    Ok(ProtocolGovernanceSyncReport {
        governance_program_count: governance_program_ids.len(),
        realm_count: realms.len(),
        governance_count: governances.len(),
        matched_realm_count,
        link_count: links.len(),
        inserted_count,
    })
}

async fn fetch_realms_for_program(
    state: &SharedState,
    program_id: Pubkey,
) -> Result<Vec<RealmAccount>> {
    let mut realms = Vec::new();
    for tag in [REALM_V1_TAG, REALM_V2_TAG] {
        for account in rpc_get_program_accounts_with_tag(state, program_id, tag).await? {
            let Some(realm) = parse_realm_account(program_id, account)? else {
                continue;
            };
            realms.push(realm);
        }
    }

    Ok(realms)
}

async fn fetch_governances_for_program(
    state: &SharedState,
    program_id: Pubkey,
) -> Result<Vec<GovernanceAccount>> {
    let mut governances = Vec::new();
    for tag in GOVERNANCE_TAGS {
        for account in rpc_get_program_accounts_with_tag(state, program_id, *tag).await? {
            let Some(governance) = parse_governance_account(program_id, account)? else {
                continue;
            };
            governances.push(governance);
        }
    }

    Ok(governances)
}

async fn rpc_get_program_accounts_with_tag(
    state: &SharedState,
    program_id: Pubkey,
    tag: u8,
) -> Result<Vec<RpcProgramAccount>> {
    rpc_call::<Vec<RpcProgramAccount>>(
        state,
        "getProgramAccounts",
        json!([
            program_id.to_string(),
            {
                "encoding": "base64",
                "filters": [
                    { "memcmp": { "offset": 0, "bytes": bs58::encode([tag]).into_string() } }
                ]
            }
        ]),
    )
    .await
    .with_context(|| format!("fetch Realms accounts for {program_id} tag {tag}"))
}

fn protocol_governance_links(
    realms: &[RealmAccount],
    governances: &[GovernanceAccount],
) -> Vec<ProtocolGovernanceLinkWrite> {
    let realms_by_address = realms
        .iter()
        .map(|realm| (realm.address.clone(), realm))
        .collect::<HashMap<_, _>>();
    let mut governances_by_realm = HashMap::<String, Vec<&GovernanceAccount>>::new();
    for governance in governances {
        governances_by_realm
            .entry(governance.realm.clone())
            .or_default()
            .push(governance);
    }

    let mut links = Vec::new();
    let mut dedupe = HashSet::<(String, String, String, String, String)>::new();

    for realm in realms {
        let governance_accounts = governances_by_realm
            .get(&realm.address)
            .cloned()
            .unwrap_or_default();
        let mut protocol_matches = protocol_matches_for_realm(realm);

        for governance in &governance_accounts {
            if let Ok(governed_account) = Pubkey::from_str(&governance.governed_account) {
                if let Some(protocol) = protocols::protocol_for_program(governed_account) {
                    protocol_matches.entry(protocol.slug).or_insert_with(|| {
                        format!(
                            "Realms governance controls tracked program {}",
                            protocol.program_label
                        )
                    });
                }
            }

            if let Some(protocol) =
                protocols::protocol_for_realms_governance_account(&governance.address)
            {
                protocol_matches
                    .entry(protocol.slug)
                    .or_insert_with(|| "Known Realms governance account".to_owned());
            }
        }

        if protocol_matches.is_empty() {
            continue;
        }

        if governance_accounts.is_empty() {
            for (protocol_slug, evidence) in protocol_matches {
                push_governance_link(
                    &mut links,
                    &mut dedupe,
                    &protocol_slug,
                    realm,
                    None,
                    evidence,
                );
            }
            continue;
        }

        for governance in governance_accounts {
            let evidence_for_governance =
                if let Ok(governed_account) = Pubkey::from_str(&governance.governed_account) {
                    protocols::protocol_for_program(governed_account).map(|protocol| {
                        (
                            protocol.slug,
                            format!(
                                "Realms governance controls tracked program {}",
                                protocol.program_label
                            ),
                        )
                    })
                } else {
                    None
                };

            for (protocol_slug, evidence) in &protocol_matches {
                let evidence = evidence_for_governance
                    .as_ref()
                    .filter(|(slug, _)| slug == protocol_slug)
                    .map(|(_, evidence)| evidence.clone())
                    .unwrap_or_else(|| evidence.clone());
                push_governance_link(
                    &mut links,
                    &mut dedupe,
                    protocol_slug,
                    realm,
                    Some(governance),
                    evidence,
                );
            }
        }
    }

    for governance in governances {
        if realms_by_address.contains_key(&governance.realm) {
            continue;
        }
        let Some(protocol) = protocols::protocol_for_realms_governance_account(&governance.address)
        else {
            continue;
        };
        let fallback_realm = RealmAccount {
            program_id: governance.program_id,
            address: governance.realm.clone(),
            name: protocol.name.clone(),
            community_mint: String::new(),
            council_mint: None,
            authority: None,
        };
        push_governance_link(
            &mut links,
            &mut dedupe,
            &protocol.slug,
            &fallback_realm,
            Some(governance),
            "Known Realms governance account".to_owned(),
        );
    }

    links.sort_by(|left, right| {
        left.protocol_slug
            .cmp(&right.protocol_slug)
            .then_with(|| left.realm_name.cmp(&right.realm_name))
            .then_with(|| left.governance_kind.cmp(&right.governance_kind))
            .then_with(|| left.governance.cmp(&right.governance))
    });

    links
}

fn protocol_matches_for_realm(realm: &RealmAccount) -> HashMap<String, String> {
    let mut matches = HashMap::new();

    if let Some(protocol_slug) = protocols::protocol_slug_for_realm_name(&realm.name) {
        matches.insert(protocol_slug, format!("Realm name matches {}", realm.name));
    }

    for account in [
        realm.address.as_str(),
        realm.community_mint.as_str(),
        realm.council_mint.as_deref().unwrap_or_default(),
        realm.authority.as_deref().unwrap_or_default(),
    ] {
        if let Some(protocol) = protocols::protocol_for_realms_governance_account(account) {
            matches
                .entry(protocol.slug)
                .or_insert_with(|| "Known Realms realm account".to_owned());
        }
    }

    matches
}

fn push_governance_link(
    links: &mut Vec<ProtocolGovernanceLinkWrite>,
    dedupe: &mut HashSet<(String, String, String, String, String)>,
    protocol_slug: &str,
    realm: &RealmAccount,
    governance: Option<&GovernanceAccount>,
    evidence: String,
) {
    let governance_address = governance
        .map(|account| account.address.clone())
        .unwrap_or_default();
    let governed_account = governance
        .map(|account| account.governed_account.clone())
        .unwrap_or_default();
    let governance_kind = governance
        .map(|account| governance_kind_label(account.account_type).to_owned())
        .unwrap_or_else(|| "realm".to_owned());
    let key = (
        protocol_slug.to_owned(),
        realm.program_id.to_string(),
        realm.address.clone(),
        governance_address.clone(),
        governed_account.clone(),
    );
    if !dedupe.insert(key) {
        return;
    }

    links.push(ProtocolGovernanceLinkWrite {
        protocol_slug: protocol_slug.to_owned(),
        governance_program_id: realm.program_id.to_string(),
        realm: realm.address.clone(),
        realm_name: realm.name.clone(),
        governance: governance_address,
        governed_account,
        governance_kind,
        evidence,
        source: SOURCE_REALMS_ACCOUNT_INDEX.to_owned(),
    });
}

fn parse_realm_account(
    program_id: Pubkey,
    account: RpcProgramAccount,
) -> Result<Option<RealmAccount>> {
    if account.account.owner != program_id.to_string() {
        return Ok(None);
    }
    let data = account.account.data.bytes()?;
    let Some(tag) = data.first().copied() else {
        return Ok(None);
    };
    if tag != REALM_V1_TAG && tag != REALM_V2_TAG {
        return Ok(None);
    }

    let community_mint = read_pubkey_string(&data, 1)?;
    let mut offset = 33_usize;
    offset += 2; // legacy voter-weight addin flags
    offset += 6; // reserved
    offset += 8; // min community weight to create governance
    offset = skip_mint_max_voter_weight_source(&data, offset)?;
    let (council_mint, next_offset) = read_option_pubkey(&data, offset)?;
    offset = next_offset;
    offset += 6; // reserved
    offset += 2; // legacy voting proposal count
    let (authority, next_offset) = read_option_pubkey(&data, offset)?;
    offset = next_offset;
    let (name, _) = read_borsh_string(&data, offset)?;

    Ok(Some(RealmAccount {
        program_id,
        address: account.pubkey,
        name,
        community_mint,
        council_mint,
        authority,
    }))
}

fn parse_governance_account(
    program_id: Pubkey,
    account: RpcProgramAccount,
) -> Result<Option<GovernanceAccount>> {
    if account.account.owner != program_id.to_string() {
        return Ok(None);
    }
    let data = account.account.data.bytes()?;
    let Some(account_type) = data.first().copied() else {
        return Ok(None);
    };
    if !GOVERNANCE_TAGS.contains(&account_type) {
        return Ok(None);
    }

    Ok(Some(GovernanceAccount {
        program_id,
        address: account.pubkey,
        account_type,
        realm: read_pubkey_string(&data, 1)?,
        governed_account: read_pubkey_string(&data, 33)?,
    }))
}

fn skip_mint_max_voter_weight_source(data: &[u8], offset: usize) -> Result<usize> {
    let tag = *data
        .get(offset)
        .ok_or_else(|| anyhow!("missing mint max voter weight source tag"))?;
    match tag {
        0 | 1 => Ok(offset + 1 + 8),
        value => bail!("unsupported mint max voter weight source tag {value}"),
    }
}

fn read_option_pubkey(data: &[u8], offset: usize) -> Result<(Option<String>, usize)> {
    let tag = *data
        .get(offset)
        .ok_or_else(|| anyhow!("missing option pubkey tag at {offset}"))?;
    match tag {
        0 => Ok((None, offset + 1)),
        1 => Ok((Some(read_pubkey_string(data, offset + 1)?), offset + 33)),
        value => bail!("unsupported option pubkey tag {value}"),
    }
}

fn read_pubkey_string(data: &[u8], offset: usize) -> Result<String> {
    let bytes = data
        .get(offset..offset + 32)
        .ok_or_else(|| anyhow!("missing pubkey at offset {offset}"))?;
    Ok(Pubkey::new_from_array(bytes.try_into()?).to_string())
}

fn read_borsh_string(data: &[u8], offset: usize) -> Result<(String, usize)> {
    let len_bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("missing string length at offset {offset}"))?;
    let len = u32::from_le_bytes(len_bytes.try_into()?) as usize;
    let start = offset + 4;
    let end = start
        .checked_add(len)
        .ok_or_else(|| anyhow!("string length overflow"))?;
    let bytes = data
        .get(start..end)
        .ok_or_else(|| anyhow!("string data exceeds account length"))?;
    let value = std::str::from_utf8(bytes)
        .context("realm name is not valid UTF-8")?
        .to_owned();
    Ok((value, end))
}

fn governance_kind_label(account_type: u8) -> &'static str {
    match account_type {
        3 | 18 => "governance",
        4 | 19 => "program_governance",
        9 | 20 => "mint_governance",
        10 | 21 => "token_governance",
        _ => "governance",
    }
}

async fn rpc_call<T>(state: &SharedState, method: &str, params: Value) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let request_body = json!({
        "jsonrpc": "2.0",
        "id": "firewatch",
        "method": method,
        "params": params,
    });
    let mut last_error = None;

    for attempt in 0..RPC_RETRY_ATTEMPTS {
        let request = state
            .http_client
            .post(&state.config.solana_rpc_http_url)
            .json(&request_body);
        let response = timeout(
            Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECONDS),
            request.send(),
        )
        .await
        .with_context(|| format!("RPC {method} timed out while sending"))?
        .with_context(|| format!("send RPC {method}"))?;

        let status = response.status();
        if !status.is_success() {
            if status.as_u16() == 429 || status.is_server_error() {
                last_error = Some(format!("RPC {method} returned HTTP {status}"));
                retry_rpc_after(attempt).await;
                continue;
            }
            bail!("RPC {method} returned HTTP {status}");
        }

        let payload = timeout(
            Duration::from_secs(DEFAULT_RPC_TIMEOUT_SECONDS),
            response.json::<RpcResponse<T>>(),
        )
        .await
        .with_context(|| format!("RPC {method} timed out while decoding"))?
        .with_context(|| format!("decode RPC {method} response"))?;
        if let Some(error) = payload.error {
            if is_retryable_rpc_error(&error) {
                last_error = Some(format!(
                    "RPC {method} error {}: {}",
                    error.code, error.message
                ));
                retry_rpc_after(attempt).await;
                continue;
            }
            bail!("RPC {method} error {}: {}", error.code, error.message);
        }

        return payload
            .result
            .ok_or_else(|| anyhow!("RPC {method} response was missing result"));
    }

    Err(anyhow!(
        "{} after {RPC_RETRY_ATTEMPTS} attempts",
        last_error.unwrap_or_else(|| format!("RPC {method} failed"))
    ))
}

fn is_retryable_rpc_error(error: &RpcError) -> bool {
    let message = error.message.to_ascii_lowercase();
    error.code == 429
        || error.code == -32005
        || message.contains("too many request")
        || message.contains("rate limit")
}

async fn retry_rpc_after(attempt: usize) {
    let delay_ms = 750_u64.saturating_mul(1_u64 << attempt.min(4));
    sleep(Duration::from_millis(delay_ms)).await;
}

impl RpcAccountData {
    fn bytes(&self) -> Result<Vec<u8>> {
        let encoded = match self {
            Self::Encoded(values) => values
                .first()
                .ok_or_else(|| anyhow!("RPC account data array was empty"))?,
            Self::Raw(value) => value,
        };

        BASE64
            .decode(encoded)
            .context("decode base64 RPC account data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governance_kind_labels_cover_v1_and_v2_types() {
        assert_eq!(governance_kind_label(3), "governance");
        assert_eq!(governance_kind_label(4), "program_governance");
        assert_eq!(governance_kind_label(18), "governance");
        assert_eq!(governance_kind_label(19), "program_governance");
    }

    #[test]
    fn parses_borsh_string() {
        let mut data = Vec::new();
        data.extend_from_slice(&(4_u32).to_le_bytes());
        data.extend_from_slice(b"Bonk");

        let (value, offset) = read_borsh_string(&data, 0).unwrap();
        assert_eq!(value, "Bonk");
        assert_eq!(offset, data.len());
    }
}
