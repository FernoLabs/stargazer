use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::{collections::HashMap, io::Read, str::FromStr};

const PROGRAM_METADATA_PROGRAM_ID: &str = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
const CANONICAL_IDL_SEED: &str = "idl";
const LEGACY_ANCHOR_IDL_SEED: &str = "anchor:idl";
const SEED_LEN: usize = 16;

#[derive(Clone, Debug, Deserialize)]
struct RpcResponse {
    result: RpcResult,
}

#[derive(Clone, Debug, Deserialize)]
struct RpcResult {
    value: Vec<Option<RpcAccount>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RpcAccount {
    executable: bool,
    lamports: u64,
    owner: String,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
    space: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct IdlAccountProbe {
    address: String,
    present: bool,
    owner: Option<String>,
    executable: Option<bool>,
    lamports: Option<u64>,
    space: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct ProgramIdlProbe {
    program_id: String,
    program_metadata: IdlAccountProbe,
    anchor_legacy: IdlAccountProbe,
}

fn seed_bytes(seed: &str) -> [u8; SEED_LEN] {
    let mut output = [0_u8; SEED_LEN];
    let bytes = seed.as_bytes();
    let length = bytes.len().min(SEED_LEN);
    output[..length].copy_from_slice(&bytes[..length]);
    output
}

fn program_metadata_program_id() -> Pubkey {
    Pubkey::from_str(PROGRAM_METADATA_PROGRAM_ID).expect("valid program metadata program id")
}

fn canonical_idl_metadata_address(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[program.as_ref(), &seed_bytes(CANONICAL_IDL_SEED)],
        &program_metadata_program_id(),
    )
    .0
}

fn legacy_anchor_idl_address(program: Pubkey) -> Option<Pubkey> {
    let program_signer = Pubkey::find_program_address(&[], &program).0;
    Pubkey::create_with_seed(&program_signer, LEGACY_ANCHOR_IDL_SEED, &program).ok()
}

fn account_probe(address: Pubkey, account: Option<RpcAccount>) -> IdlAccountProbe {
    IdlAccountProbe {
        address: address.to_string(),
        present: account.is_some(),
        owner: account.as_ref().map(|account| account.owner.clone()),
        executable: account.as_ref().map(|account| account.executable),
        lamports: account.as_ref().map(|account| account.lamports),
        space: account.and_then(|account| account.space),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_owned());
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read program ids from stdin")?;
    let program_ids: Vec<String> =
        serde_json::from_str(&input).context("stdin must be a JSON array of program ids")?;

    let mut probes = Vec::new();
    let mut addresses = Vec::new();
    for program_id in program_ids {
        let Ok(program) = Pubkey::from_str(&program_id) else {
            continue;
        };
        let program_metadata = canonical_idl_metadata_address(program);
        let Some(anchor_legacy) = legacy_anchor_idl_address(program) else {
            continue;
        };
        addresses.push(program_metadata);
        addresses.push(anchor_legacy);
        probes.push((program_id, program_metadata, anchor_legacy));
    }

    let client = reqwest::Client::new();
    let mut accounts = HashMap::<Pubkey, Option<RpcAccount>>::new();
    for chunk in addresses.chunks(100) {
        let address_strings = chunk
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>();
        let response = client
            .post(&rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "getMultipleAccounts",
                "params": [
                    address_strings,
                    {
                        "encoding": "base64",
                        "dataSlice": { "offset": 0, "length": 0 }
                    }
                ]
            }))
            .send()
            .await
            .context("send Solana RPC getMultipleAccounts")?
            .error_for_status()
            .context("Solana RPC status")?
            .json::<RpcResponse>()
            .await
            .context("decode Solana RPC response")?;

        for (address, account) in chunk.iter().copied().zip(response.result.value) {
            accounts.insert(address, account);
        }
    }

    let output = probes
        .into_iter()
        .map(
            |(program_id, program_metadata, anchor_legacy)| ProgramIdlProbe {
                program_id,
                program_metadata: account_probe(
                    program_metadata,
                    accounts.remove(&program_metadata).flatten(),
                ),
                anchor_legacy: account_probe(
                    anchor_legacy,
                    accounts.remove(&anchor_legacy).flatten(),
                ),
            },
        )
        .collect::<Vec<_>>();

    println!("{}", serde_json::to_string_pretty(&output)?);

    Ok(())
}
