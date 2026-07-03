use crate::{
    context::SharedState,
    database::ProtocolMultisigLinkWrite,
    protocols::{self, SQUADS_V4_PROGRAM_ID},
};
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::Duration,
};
use tokio::time::timeout;

const SOURCE_PROGRAM_UPGRADE_AUTHORITY: &str = "program_upgrade_authority";
const DEFAULT_MULTIPLE_ACCOUNT_CHUNK_SIZE: usize = 100;
const RPC_TIMEOUT_SECONDS: u64 = 90;
const BPF_LOADER_UPGRADEABLE_ID: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const PROGRAM_ACCOUNT_TAG: u32 = 2;
const PROGRAM_DATA_ACCOUNT_TAG: u32 = 3;

#[derive(Clone, Debug)]
struct ProgramSurface {
    protocol_slug: String,
    program_id: String,
    program_label: String,
}

#[derive(Clone, Debug)]
struct ProgramAuthority {
    surface: ProgramSurface,
    authority: String,
}

#[derive(Clone, Debug)]
struct SquadsAuthorityLink {
    multisig: String,
    vault_index: Option<i64>,
}

#[derive(Clone, Debug, Default)]
pub struct ProtocolAuthoritySyncReport {
    pub program_count: usize,
    pub upgradeable_program_count: usize,
    pub immutable_program_count: usize,
    pub squads_multisig_count: usize,
    pub discovered_all_squads: bool,
    pub checked_vault_index_count: u8,
    pub link_count: usize,
    pub linked_program_count: usize,
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
struct RpcMultipleAccounts {
    value: Vec<Option<RpcAccount>>,
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

#[derive(Debug, Deserialize)]
struct RpcKeyedAccount {
    pubkey: String,
}

pub async fn sync_protocol_upgrade_authority_multisigs(
    state: SharedState,
    vault_index_count: u8,
    discover_all_squads: bool,
) -> Result<ProtocolAuthoritySyncReport> {
    let program_surfaces = curated_program_surfaces();
    let program_authorities = fetch_program_authorities(&state, &program_surfaces).await?;
    let squads_multisigs = if discover_all_squads {
        fetch_squads_multisigs(&state).await?
    } else {
        state.db.known_multisig_addresses().await?
    };
    let authority_lookup = squads_authority_lookup(&squads_multisigs, vault_index_count)?;
    let links = protocol_multisig_links(&program_authorities, &authority_lookup);
    let linked_program_count = links
        .iter()
        .map(|link| link.program_id.as_str())
        .collect::<HashSet<_>>()
        .len();
    let inserted_count = state
        .db
        .replace_protocol_multisig_links(SOURCE_PROGRAM_UPGRADE_AUTHORITY, &links)
        .await?;
    let immutable_program_count = program_surfaces
        .len()
        .saturating_sub(program_authorities.len());

    Ok(ProtocolAuthoritySyncReport {
        program_count: program_surfaces.len(),
        upgradeable_program_count: program_authorities.len(),
        immutable_program_count,
        squads_multisig_count: squads_multisigs.len(),
        discovered_all_squads: discover_all_squads,
        checked_vault_index_count: vault_index_count,
        link_count: links.len(),
        linked_program_count,
        inserted_count,
    })
}

fn curated_program_surfaces() -> Vec<ProgramSurface> {
    let mut seen = HashSet::<(String, String)>::new();
    let mut surfaces = Vec::new();

    for protocol in protocols::curated_protocol_descriptors() {
        for program in protocol.programs {
            if !seen.insert((protocol.slug.clone(), program.program_id.clone())) {
                continue;
            }

            surfaces.push(ProgramSurface {
                protocol_slug: protocol.slug.clone(),
                program_id: program.program_id,
                program_label: program.label,
            });
        }
    }

    surfaces
}

async fn fetch_program_authorities(
    state: &SharedState,
    surfaces: &[ProgramSurface],
) -> Result<Vec<ProgramAuthority>> {
    let mut authorities = Vec::new();

    for chunk in surfaces.chunks(DEFAULT_MULTIPLE_ACCOUNT_CHUNK_SIZE) {
        let program_accounts = rpc_get_multiple_accounts(
            state,
            &chunk
                .iter()
                .map(|surface| surface.program_id.clone())
                .collect::<Vec<_>>(),
        )
        .await?;

        let mut data_account_requests = Vec::<(ProgramSurface, String)>::new();
        for (surface, account) in chunk.iter().cloned().zip(program_accounts) {
            let Some(account) = account else {
                continue;
            };
            let Some(program_data_address) = parse_program_data_account(&account)? else {
                continue;
            };
            data_account_requests.push((surface, program_data_address));
        }

        for data_chunk in data_account_requests.chunks(DEFAULT_MULTIPLE_ACCOUNT_CHUNK_SIZE) {
            let data_accounts = rpc_get_multiple_accounts(
                state,
                &data_chunk
                    .iter()
                    .map(|(_, address)| address.clone())
                    .collect::<Vec<_>>(),
            )
            .await?;

            for ((surface, _), account) in data_chunk.iter().cloned().zip(data_accounts) {
                let Some(account) = account else {
                    continue;
                };
                let Some(authority) = parse_program_upgrade_authority(&account)? else {
                    continue;
                };
                authorities.push(ProgramAuthority { surface, authority });
            }
        }
    }

    Ok(authorities)
}

async fn fetch_squads_multisigs(state: &SharedState) -> Result<Vec<String>> {
    let discriminator = bs58::encode(anchor_account_discriminator("Multisig")).into_string();
    let result = rpc_call::<Vec<RpcKeyedAccount>>(
        state,
        "getProgramAccounts",
        json!([
            SQUADS_V4_PROGRAM_ID,
            {
                "encoding": "base64",
                "dataSlice": { "offset": 0, "length": 0 },
                "filters": [
                    { "memcmp": { "offset": 0, "bytes": discriminator } }
                ]
            }
        ]),
    )
    .await?;

    Ok(result.into_iter().map(|account| account.pubkey).collect())
}

fn squads_authority_lookup(
    multisigs: &[String],
    vault_index_count: u8,
) -> Result<HashMap<String, SquadsAuthorityLink>> {
    let squads_program = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)?;
    let mut lookup = HashMap::new();

    for multisig in multisigs {
        let multisig_pubkey = Pubkey::from_str(multisig)
            .with_context(|| format!("parse Squads multisig {multisig}"))?;
        lookup.insert(
            multisig.clone(),
            SquadsAuthorityLink {
                multisig: multisig.clone(),
                vault_index: None,
            },
        );

        for vault_index in 0..vault_index_count {
            let (vault, _) = Pubkey::find_program_address(
                &[
                    b"multisig",
                    multisig_pubkey.as_ref(),
                    b"vault",
                    &[vault_index],
                ],
                &squads_program,
            );
            lookup.insert(
                vault.to_string(),
                SquadsAuthorityLink {
                    multisig: multisig.clone(),
                    vault_index: Some(i64::from(vault_index)),
                },
            );
        }
    }

    Ok(lookup)
}

fn protocol_multisig_links(
    authorities: &[ProgramAuthority],
    authority_lookup: &HashMap<String, SquadsAuthorityLink>,
) -> Vec<ProtocolMultisigLinkWrite> {
    let mut dedupe = HashSet::<(String, String, String, String)>::new();
    let mut links = Vec::new();

    for authority in authorities {
        let Some(link) = authority_lookup.get(&authority.authority) else {
            continue;
        };
        let key = (
            authority.surface.protocol_slug.clone(),
            authority.surface.program_id.clone(),
            link.multisig.clone(),
            authority.authority.clone(),
        );
        if !dedupe.insert(key) {
            continue;
        }

        links.push(ProtocolMultisigLinkWrite {
            protocol_slug: authority.surface.protocol_slug.clone(),
            multisig: link.multisig.clone(),
            authority: authority.authority.clone(),
            program_id: authority.surface.program_id.clone(),
            program_label: authority.surface.program_label.clone(),
            evidence: match link.vault_index {
                Some(index) => format!("Program upgrade authority is Squads vault {index}"),
                None => "Program upgrade authority is the Squads multisig account".to_owned(),
            },
            source: SOURCE_PROGRAM_UPGRADE_AUTHORITY.to_owned(),
            vault_index: link.vault_index,
        });
    }

    links.sort_by(|left, right| {
        left.protocol_slug
            .cmp(&right.protocol_slug)
            .then_with(|| left.program_label.cmp(&right.program_label))
            .then_with(|| left.multisig.cmp(&right.multisig))
    });

    links
}

async fn rpc_get_multiple_accounts(
    state: &SharedState,
    addresses: &[String],
) -> Result<Vec<Option<RpcAccount>>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }

    let response = rpc_call::<RpcMultipleAccounts>(
        state,
        "getMultipleAccounts",
        json!([addresses, { "encoding": "base64" }]),
    )
    .await?;

    Ok(response.value)
}

async fn rpc_call<T>(state: &SharedState, method: &str, params: Value) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let request = state
        .http_client
        .post(&state.config.solana_rpc_http_url)
        .json(&json!({
            "jsonrpc": "2.0",
            "id": "firewatch",
            "method": method,
            "params": params,
        }));
    let response = timeout(Duration::from_secs(RPC_TIMEOUT_SECONDS), request.send())
        .await
        .with_context(|| format!("RPC {method} timed out while sending"))?
        .with_context(|| format!("send RPC {method}"))?;
    if !response.status().is_success() {
        bail!("RPC {method} returned HTTP {}", response.status());
    }

    let payload = timeout(
        Duration::from_secs(RPC_TIMEOUT_SECONDS),
        response.json::<RpcResponse<T>>(),
    )
    .await
    .with_context(|| format!("RPC {method} timed out while decoding"))?
    .with_context(|| format!("decode RPC {method} response"))?;
    if let Some(error) = payload.error {
        bail!("RPC {method} error {}: {}", error.code, error.message);
    }

    payload
        .result
        .ok_or_else(|| anyhow!("RPC {method} response was missing result"))
}

fn parse_program_data_account(account: &RpcAccount) -> Result<Option<String>> {
    if account.owner != BPF_LOADER_UPGRADEABLE_ID {
        return Ok(None);
    }

    let data = account.data.bytes()?;
    if read_u32_le(&data, 0)? != PROGRAM_ACCOUNT_TAG {
        return Ok(None);
    }
    if data.len() < 36 {
        bail!(
            "upgradeable program account data too short: {} bytes",
            data.len()
        );
    }

    let program_data = Pubkey::new_from_array(data[4..36].try_into()?);
    Ok(Some(program_data.to_string()))
}

fn parse_program_upgrade_authority(account: &RpcAccount) -> Result<Option<String>> {
    if account.owner != BPF_LOADER_UPGRADEABLE_ID {
        return Ok(None);
    }

    let data = account.data.bytes()?;
    if read_u32_le(&data, 0)? != PROGRAM_DATA_ACCOUNT_TAG {
        return Ok(None);
    }
    if data.len() < 13 {
        bail!("program data account data too short: {} bytes", data.len());
    }

    match data[12] {
        0 => Ok(None),
        1 => {
            if data.len() < 45 {
                bail!(
                    "program data account authority missing: {} bytes",
                    data.len()
                );
            }
            let authority = Pubkey::new_from_array(data[13..45].try_into()?);
            Ok(Some(authority.to_string()))
        }
        tag => bail!("unsupported program data authority option tag {tag}"),
    }
}

fn read_u32_le(data: &[u8], offset: usize) -> Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("account data missing u32 at offset {offset}"))?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

fn anchor_account_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("account:{name}"));
    let digest = hasher.finalize();
    digest[..8].try_into().expect("slice is 8 bytes")
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
    fn squads_multisig_discriminator_matches_anchor() {
        assert_eq!(
            bs58::encode(anchor_account_discriminator("Multisig")).into_string(),
            "eYVgUYyJkvP"
        );
    }
}
