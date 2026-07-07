use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::read::{GzDecoder, ZlibDecoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

const PROGRAM_METADATA_PROGRAM_ID: &str = "ProgM6JCCvbYkfKqJYHePx4xxSUSqJp7rh8Lyv7nk7S";
const CANONICAL_IDL_SEED: &str = "idl";
const LEGACY_ANCHOR_IDL_SEED: &str = "anchor:idl";
const SEED_LEN: usize = 16;
const PROGRAM_METADATA_HEADER_LEN: usize = 96;
const LEGACY_ANCHOR_IDL_HEADER_LEN: usize = 44;
const RPC_CHUNK_SIZE: usize = 10;
const RPC_RETRIES: usize = 3;

#[derive(Clone, Debug, Deserialize)]
struct RpcResponse {
    result: RpcResult,
}

#[derive(Clone, Debug, Deserialize)]
struct RpcResult {
    value: Vec<Option<RpcAccount>>,
}

#[derive(Clone, Debug, Deserialize)]
struct RpcAccount {
    owner: String,
    data: serde_json::Value,
    executable: bool,
    lamports: u64,
    space: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct IdlDownloadReport {
    rpc_url: String,
    program_count: usize,
    account_count: usize,
    present_account_count: usize,
    decoded_idl_count: usize,
    output_dir: String,
    programs: Vec<ProgramDownloadReport>,
}

#[derive(Clone, Debug, Serialize)]
struct ProgramDownloadReport {
    program_id: String,
    program_metadata: AccountDownloadReport,
    anchor_legacy: AccountDownloadReport,
}

#[derive(Clone, Debug, Serialize)]
struct AccountDownloadReport {
    origin: String,
    address: String,
    present: bool,
    owner: Option<String>,
    executable: Option<bool>,
    lamports: Option<u64>,
    space: Option<usize>,
    decoded: bool,
    encoding: Option<String>,
    compression: Option<String>,
    format: Option<String>,
    data_source: Option<String>,
    bytes_len: Option<usize>,
    text_len: Option<usize>,
    sha256: Option<String>,
    path: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug)]
struct DecodedIdl {
    encoding: Option<String>,
    compression: Option<String>,
    format: Option<String>,
    data_source: Option<String>,
    bytes: Vec<u8>,
    text: String,
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

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let output_dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/idls"));
    let rpc_url = std::env::var("SOLANA_RPC_URL")
        .or_else(|_| std::env::var("SOLANA_RPC_HTTP_URL"))
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_owned());

    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("read program ids from stdin")?;
    let program_ids: Vec<String> =
        serde_json::from_str(&input).context("stdin must be a JSON array of program ids")?;

    fs::create_dir_all(&output_dir).with_context(|| format!("create {}", output_dir.display()))?;

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

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("build HTTP client")?;
    let accounts = fetch_accounts(&client, &rpc_url, &addresses).await?;

    let mut programs = Vec::new();
    for (program_id, program_metadata, anchor_legacy) in probes {
        let program_metadata_report = account_report(
            &output_dir,
            &program_id,
            "program_metadata",
            program_metadata,
            accounts.get(&program_metadata).cloned().flatten(),
        )?;
        let anchor_legacy_report = account_report(
            &output_dir,
            &program_id,
            "anchor_legacy",
            anchor_legacy,
            accounts.get(&anchor_legacy).cloned().flatten(),
        )?;

        programs.push(ProgramDownloadReport {
            program_id,
            program_metadata: program_metadata_report,
            anchor_legacy: anchor_legacy_report,
        });
    }

    let present_account_count = programs
        .iter()
        .flat_map(|program| [&program.program_metadata, &program.anchor_legacy])
        .filter(|account| account.present)
        .count();
    let decoded_idl_count = programs
        .iter()
        .flat_map(|program| [&program.program_metadata, &program.anchor_legacy])
        .filter(|account| account.decoded)
        .count();
    let report = IdlDownloadReport {
        rpc_url: redacted_rpc_url(&rpc_url),
        program_count: programs.len(),
        account_count: programs.len() * 2,
        present_account_count,
        decoded_idl_count,
        output_dir: output_dir.display().to_string(),
        programs,
    };

    let index_path = output_dir.join("index.json");
    let index = serde_json::to_vec_pretty(&report)?;
    fs::write(&index_path, [index, b"\n".to_vec()].concat())
        .with_context(|| format!("write {}", index_path.display()))?;
    println!("{}", serde_json::to_string_pretty(&report)?);

    Ok(())
}

async fn fetch_accounts(
    client: &reqwest::Client,
    rpc_url: &str,
    addresses: &[Pubkey],
) -> Result<HashMap<Pubkey, Option<RpcAccount>>> {
    let mut accounts = HashMap::<Pubkey, Option<RpcAccount>>::new();

    for chunk in addresses.chunks(RPC_CHUNK_SIZE) {
        let address_strings = chunk
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<String>>();
        let mut last_error = None;

        for attempt in 1..=RPC_RETRIES {
            let result = client
                .post(rpc_url)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "getMultipleAccounts",
                    "params": [
                        address_strings,
                        { "encoding": "base64" }
                    ]
                }))
                .send()
                .await
                .context("send Solana RPC getMultipleAccounts")
                .and_then(|response| response.error_for_status().context("Solana RPC status"));

            match result {
                Ok(response) => {
                    let response = response
                        .json::<RpcResponse>()
                        .await
                        .context("decode Solana RPC response")?;
                    for (address, account) in chunk.iter().copied().zip(response.result.value) {
                        accounts.insert(address, account);
                    }
                    last_error = None;
                    break;
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt < RPC_RETRIES {
                        tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
                    }
                }
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }
    }

    Ok(accounts)
}

fn account_report(
    output_dir: &Path,
    program_id: &str,
    origin: &str,
    address: Pubkey,
    account: Option<RpcAccount>,
) -> Result<AccountDownloadReport> {
    let base_report = |present: bool,
                       account: Option<&RpcAccount>,
                       decoded: bool,
                       decoded_idl: Option<&DecodedIdl>,
                       path: Option<String>,
                       error: Option<String>| {
        AccountDownloadReport {
            origin: origin.to_owned(),
            address: address.to_string(),
            present,
            owner: account.map(|account| account.owner.clone()),
            executable: account.map(|account| account.executable),
            lamports: account.map(|account| account.lamports),
            space: account.and_then(|account| account.space),
            decoded,
            encoding: decoded_idl.and_then(|idl| idl.encoding.clone()),
            compression: decoded_idl.and_then(|idl| idl.compression.clone()),
            format: decoded_idl.and_then(|idl| idl.format.clone()),
            data_source: decoded_idl.and_then(|idl| idl.data_source.clone()),
            bytes_len: decoded_idl.map(|idl| idl.bytes.len()),
            text_len: decoded_idl.map(|idl| idl.text.len()),
            sha256: decoded_idl.map(|idl| sha256_hex(&idl.bytes)),
            path,
            error,
        }
    };

    let Some(account) = account else {
        return Ok(base_report(false, None, false, None, None, None));
    };
    let raw_data = match rpc_account_data(&account) {
        Ok(data) => data,
        Err(error) => {
            return Ok(base_report(
                true,
                Some(&account),
                false,
                None,
                None,
                Some(error.to_string()),
            ));
        }
    };
    let decoded = match origin {
        "program_metadata" => decode_program_metadata_idl(&raw_data),
        "anchor_legacy" => decode_anchor_legacy_idl(&raw_data),
        _ => Err(anyhow!("unsupported origin {origin}")),
    };

    match decoded {
        Ok(decoded) => {
            let extension = if decoded.format.as_deref() == Some("json")
                && serde_json::from_str::<serde_json::Value>(&decoded.text).is_ok()
            {
                "json"
            } else {
                "txt"
            };
            let filename = format!("{program_id}.{origin}.{extension}");
            let path = output_dir.join(filename);
            write_text(&path, &decoded.text)?;
            Ok(base_report(
                true,
                Some(&account),
                true,
                Some(&decoded),
                Some(path.display().to_string()),
                None,
            ))
        }
        Err(error) => Ok(base_report(
            true,
            Some(&account),
            false,
            None,
            None,
            Some(error.to_string()),
        )),
    }
}

fn rpc_account_data(account: &RpcAccount) -> Result<Vec<u8>> {
    let data = account
        .data
        .as_array()
        .and_then(|values| values.first())
        .and_then(|value| value.as_str())
        .ok_or_else(|| anyhow!("unexpected RPC account data shape"))?;
    STANDARD.decode(data).context("decode base64 account data")
}

fn decode_program_metadata_idl(data: &[u8]) -> Result<DecodedIdl> {
    if data.len() < PROGRAM_METADATA_HEADER_LEN {
        return Err(anyhow!("program metadata account too short"));
    }
    if data.first().copied() != Some(2) {
        return Err(anyhow!("program metadata header discriminator mismatch"));
    }
    let seed = seed_string(
        data.get(67..83)
            .ok_or_else(|| anyhow!("metadata seed missing"))?,
    );
    if seed != CANONICAL_IDL_SEED {
        return Err(anyhow!(
            "metadata seed is {seed:?}, not {CANONICAL_IDL_SEED:?}"
        ));
    }

    let encoding = metadata_encoding(*data.get(83).ok_or_else(|| anyhow!("encoding missing"))?)?;
    let compression =
        metadata_compression(*data.get(84).ok_or_else(|| anyhow!("compression missing"))?)?;
    let format = metadata_format(*data.get(85).ok_or_else(|| anyhow!("format missing"))?)?;
    let data_source =
        metadata_data_source(*data.get(86).ok_or_else(|| anyhow!("data source missing"))?)?;
    let data_len = u32::from_le_bytes(
        data.get(87..91)
            .ok_or_else(|| anyhow!("data length missing"))?
            .try_into()?,
    ) as usize;
    let payload_start = PROGRAM_METADATA_HEADER_LEN;
    let payload_end = payload_start
        .checked_add(data_len)
        .filter(|end| *end <= data.len())
        .unwrap_or(data.len());
    let bytes = decode_bytes(&data[payload_start..payload_end], &compression)?;
    let text = decode_text(&bytes, &encoding)?;

    Ok(DecodedIdl {
        encoding: Some(encoding),
        compression: Some(compression),
        format: Some(format),
        data_source: Some(data_source),
        bytes,
        text,
    })
}

fn decode_anchor_legacy_idl(data: &[u8]) -> Result<DecodedIdl> {
    let mut candidates = Vec::<&[u8]>::new();
    if data.len() > LEGACY_ANCHOR_IDL_HEADER_LEN {
        let declared_len = u32::from_le_bytes(
            data.get(40..44)
                .ok_or_else(|| anyhow!("legacy IDL data length missing"))?
                .try_into()?,
        ) as usize;
        let start = LEGACY_ANCHOR_IDL_HEADER_LEN;
        if start + declared_len <= data.len() {
            candidates.push(&data[start..start + declared_len]);
        }
        candidates.push(&data[start..]);
    }
    candidates.push(data);

    for candidate in candidates {
        let Ok(bytes) = decode_bytes(candidate, "zlib") else {
            continue;
        };
        let Ok(text) = decode_text(&bytes, "utf8") else {
            continue;
        };
        if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
            return Ok(DecodedIdl {
                encoding: Some("utf8".to_owned()),
                compression: Some("zlib".to_owned()),
                format: Some("json".to_owned()),
                data_source: Some("anchor_idl_account".to_owned()),
                bytes,
                text,
            });
        }
    }

    Err(anyhow!("could not zlib-decode a JSON legacy Anchor IDL"))
}

fn decode_bytes(bytes: &[u8], compression: &str) -> Result<Vec<u8>> {
    match compression {
        "none" => Ok(bytes.to_vec()),
        "zlib" => {
            let mut decoder = ZlibDecoder::new(bytes);
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            Ok(output)
        }
        "gzip" => {
            let mut decoder = GzDecoder::new(bytes);
            let mut output = Vec::new();
            decoder.read_to_end(&mut output)?;
            Ok(output)
        }
        unknown => Err(anyhow!("unsupported compression {unknown:?}")),
    }
}

fn decode_text(bytes: &[u8], encoding: &str) -> Result<String> {
    match encoding {
        "utf8" => String::from_utf8(bytes.to_vec()).context("decode UTF-8"),
        "none" => Ok(hex::encode(bytes)),
        "base58" => Ok(bs58::encode(bytes).into_string()),
        "base64" => Ok(STANDARD.encode(bytes)),
        unknown => Err(anyhow!("unsupported encoding {unknown:?}")),
    }
}

fn metadata_encoding(value: u8) -> Result<String> {
    Ok(match value {
        0 => "none",
        1 => "utf8",
        2 => "base58",
        3 => "base64",
        _ => return Err(anyhow!("unsupported metadata encoding {value}")),
    }
    .to_owned())
}

fn metadata_compression(value: u8) -> Result<String> {
    Ok(match value {
        0 => "none",
        1 => "gzip",
        2 => "zlib",
        _ => return Err(anyhow!("unsupported metadata compression {value}")),
    }
    .to_owned())
}

fn metadata_format(value: u8) -> Result<String> {
    Ok(match value {
        0 => "none",
        1 => "json",
        2 => "yaml",
        3 => "toml",
        _ => return Err(anyhow!("unsupported metadata format {value}")),
    }
    .to_owned())
}

fn metadata_data_source(value: u8) -> Result<String> {
    Ok(match value {
        0 => "direct",
        1 => "url",
        2 => "external",
        _ => return Err(anyhow!("unsupported metadata data source {value}")),
    }
    .to_owned())
}

fn seed_string(seed_bytes: &[u8]) -> String {
    let end = seed_bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(seed_bytes.len());
    String::from_utf8_lossy(&seed_bytes[..end]).to_string()
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write_text(path: &Path, text: &str) -> Result<()> {
    let mut file = fs::File::create(path).with_context(|| format!("create {}", path.display()))?;
    if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            let pretty = serde_json::to_vec_pretty(&value)?;
            file.write_all(&pretty)?;
            file.write_all(b"\n")?;
            return Ok(());
        }
    }
    file.write_all(text.as_bytes())?;
    if !text.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    Ok(())
}

fn redacted_rpc_url(url: &str) -> String {
    let Some((base, token)) = url.rsplit_once('/') else {
        return url.to_owned();
    };
    if token.len() < 12 {
        return url.to_owned();
    }
    format!("{base}/<redacted>")
}
