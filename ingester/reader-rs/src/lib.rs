pub mod api;
pub mod archive;
pub mod compact;
pub mod decode;
pub mod governance;
pub mod incidents;
pub mod indexer;

pub use archive::{discover_files, parse_archive_filename, ArchiveFile, ArchiveWalker};
pub use compact::{CompactIndexDatabase, CompactIndexStats};
pub use decode::{
    decode_ledger, AccountInteraction, ContractCall, ContractTouch, ContractVersionObservation,
    DecodedLedger, IndexedTransaction, LedgerMetrics, WalletContractLink, WasmCodeObservation,
    DEFAULT_MAX_COMPRESSED_BYTES, DEFAULT_MAX_XDR_BYTES, DEFAULT_ZSTD_WINDOW_LOG_MAX,
    PUBLIC_NETWORK_PASSPHRASE,
};
pub use indexer::{
    IndexDatabase, IndexOptions, IndexRunStats, WalletProtocolSummary, WalletTransactionSummary,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Debug, Serialize)]
pub struct LedgerSummary {
    pub path: String,
    pub sequence: u32,
    pub batch_start_sequence: u32,
    pub batch_end_sequence: u32,
    pub ledger_count: usize,
    pub meta_version: i32,
    pub compressed_bytes: u64,
    pub xdr_bytes: usize,
    pub xdr_sha256: String,
}

/// Full validation mode, including a digest of the decompressed XDR. It uses
/// two bounded streaming passes to keep memory independent of ledger size.
pub fn read_summary(file: &ArchiveFile) -> Result<LedgerSummary> {
    let metrics = read_metrics(file)?;
    let input = BufReader::new(
        File::open(&file.path).with_context(|| format!("opening {}", file.path.display()))?,
    );
    let mut decoder = zstd::Decoder::new(input)?;
    decoder.window_log_max(DEFAULT_ZSTD_WINDOW_LOG_MAX)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut xdr_bytes = 0_usize;
    loop {
        let read = decoder.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        xdr_bytes = xdr_bytes
            .checked_add(read)
            .context("decompressed XDR size overflow")?;
        if xdr_bytes > DEFAULT_MAX_XDR_BYTES {
            bail!(
                "decompressed XDR exceeds {} bytes in {}",
                DEFAULT_MAX_XDR_BYTES,
                file.path.display()
            );
        }
        hasher.update(&buffer[..read]);
    }
    Ok(LedgerSummary {
        path: file.path.display().to_string(),
        sequence: file.sequence,
        batch_start_sequence: file.sequence,
        batch_end_sequence: file.sequence,
        ledger_count: 1,
        meta_version: metrics.meta_version,
        compressed_bytes: metrics.compressed_bytes,
        xdr_bytes,
        xdr_sha256: hex::encode(hasher.finalize()),
    })
}

pub fn read_metrics(file: &ArchiveFile) -> Result<LedgerMetrics> {
    Ok(decode_ledger(file, PUBLIC_NETWORK_PASSPHRASE, DEFAULT_MAX_XDR_BYTES)?.metrics)
}
