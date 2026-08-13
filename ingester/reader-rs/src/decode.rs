use crate::archive::ArchiveFile;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufReader, Read};
use stellar_xdr::{
    FeeBumpTransactionInnerTx, GeneralizedTransactionSet, HostFunction, LedgerCloseMeta,
    LedgerCloseMetaBatch, Limited, Limits, MuxedAccount, Operation, OperationBody, ReadXdr,
    SorobanAuthorizedFunction, SorobanAuthorizedInvocation, TransactionEnvelope, TransactionPhase,
    TransactionResult, TransactionResultResult, TxSetComponent,
};

pub const PUBLIC_NETWORK_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
pub const DEFAULT_MAX_XDR_BYTES: usize = 256 * 1024 * 1024;
pub const DEFAULT_MAX_COMPRESSED_BYTES: u64 = 128 * 1024 * 1024;
pub const DEFAULT_ZSTD_WINDOW_LOG_MAX: u32 = 27;

#[derive(Debug, Clone, Serialize)]
pub struct LedgerMetrics {
    pub sequence: u32,
    pub path: String,
    #[serde(serialize_with = "serialize_hash")]
    pub ledger_hash: [u8; 32],
    #[serde(serialize_with = "serialize_hash")]
    pub previous_ledger_hash: [u8; 32],
    pub close_time: u64,
    pub meta_version: i32,
    pub transaction_count: usize,
    pub successful_transaction_count: usize,
    pub operation_count: usize,
    pub direct_contract_call_count: usize,
    pub contract_touch_count: usize,
    pub soroban_state_bytes: u64,
    pub evicted_keys: usize,
    pub compressed_bytes: u64,
    pub xdr_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountInteraction {
    pub account_id: String,
    pub muxed_id: Option<u64>,
    pub role: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractCall {
    pub operation_index: usize,
    #[serde(serialize_with = "serialize_contract_id")]
    pub contract_id: [u8; 32],
    pub function_name: String,
    pub call_kind: &'static str,
    pub call_depth: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContractTouch {
    #[serde(serialize_with = "serialize_contract_id")]
    pub contract_id: [u8; 32],
    pub touch_kind: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WalletContractLink {
    pub account_id: String,
    #[serde(serialize_with = "serialize_contract_id")]
    pub contract_id: [u8; 32],
    pub role: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ContractVersionObservation {
    #[serde(serialize_with = "serialize_contract_id")]
    pub contract_id: [u8; 32],
    pub executable_kind: &'static str,
    #[serde(serialize_with = "serialize_optional_hash")]
    pub wasm_hash: Option<[u8; 32]>,
    pub change_kind: &'static str,
    pub exact: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WasmCodeObservation {
    #[serde(serialize_with = "serialize_hash")]
    pub wasm_hash: [u8; 32],
    pub code_size: usize,
    pub change_kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedTransaction {
    #[serde(serialize_with = "serialize_hash")]
    pub hash: [u8; 32],
    pub apply_order: usize,
    pub source_account: String,
    pub source_muxed_id: Option<u64>,
    pub fee_source_account: Option<String>,
    pub fee_source_muxed_id: Option<u64>,
    pub successful: bool,
    pub fee_charged: i64,
    pub operation_count: usize,
    pub accounts: Vec<AccountInteraction>,
    pub authorized_accounts: Vec<String>,
    pub wallet_contract_links: Vec<WalletContractLink>,
    pub contract_calls: Vec<ContractCall>,
    pub contract_touches: Vec<ContractTouch>,
    pub contract_versions: Vec<ContractVersionObservation>,
    pub wasm_codes: Vec<WasmCodeObservation>,
}

impl IndexedTransaction {
    pub fn is_contract_interacting(&self) -> bool {
        !self.contract_calls.is_empty()
            || !self.contract_touches.is_empty()
            || !self.contract_versions.is_empty()
            || !self.wasm_codes.is_empty()
    }
}

fn serialize_hash<S>(hash: &[u8; 32], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&hex::encode(hash))
}

fn serialize_optional_hash<S>(
    hash: &Option<[u8; 32]>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match hash {
        Some(hash) => serializer.serialize_some(&hex::encode(hash)),
        None => serializer.serialize_none(),
    }
}

fn serialize_contract_id<S>(id: &[u8; 32], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&stellar_xdr::ContractId(stellar_xdr::Hash(*id)).to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct DecodedLedger {
    pub metrics: LedgerMetrics,
    pub transactions: Vec<IndexedTransaction>,
}

struct CountingReader<R> {
    inner: R,
    bytes_read: usize,
}

impl<R> CountingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            bytes_read: 0,
        }
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes_read += read;
        Ok(read)
    }
}

fn network_id(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}

fn transaction_success(result: &TransactionResult) -> bool {
    matches!(
        result.result,
        TransactionResultResult::TxSuccess(_) | TransactionResultResult::TxFeeBumpInnerSuccess(_)
    )
}

fn generalized_envelopes(tx_set: &GeneralizedTransactionSet) -> Vec<&TransactionEnvelope> {
    let mut envelopes = Vec::new();
    let GeneralizedTransactionSet::V1(set) = tx_set;
    for phase in set.phases.iter() {
        match phase {
            TransactionPhase::V0(components) => {
                for component in components.iter() {
                    let TxSetComponent::TxsetCompTxsMaybeDiscountedFee(component) = component;
                    envelopes.extend(component.txs.iter());
                }
            }
            TransactionPhase::V1(parallel) => {
                for stage in parallel.execution_stages.iter() {
                    for cluster in stage.iter() {
                        envelopes.extend(cluster.iter());
                    }
                }
            }
        }
    }
    envelopes
}

fn envelopes(meta: &LedgerCloseMeta) -> Vec<&TransactionEnvelope> {
    match meta {
        LedgerCloseMeta::V0(value) => value.tx_set.txs.iter().collect(),
        LedgerCloseMeta::V1(value) => generalized_envelopes(&value.tx_set),
        LedgerCloseMeta::V2(value) => generalized_envelopes(&value.tx_set),
    }
}

struct EnvelopeView<'a> {
    source: String,
    source_muxed_id: Option<u64>,
    fee_source: Option<String>,
    fee_source_muxed_id: Option<u64>,
    operations: &'a [Operation],
}

fn account_identity(account: &MuxedAccount) -> (String, Option<u64>) {
    match account {
        MuxedAccount::Ed25519(id) => (MuxedAccount::Ed25519(id.clone()).to_string(), None),
        MuxedAccount::MuxedEd25519(muxed) => (
            MuxedAccount::Ed25519(muxed.ed25519.clone()).to_string(),
            Some(muxed.id),
        ),
    }
}

fn envelope_view(envelope: &TransactionEnvelope) -> EnvelopeView<'_> {
    match envelope {
        TransactionEnvelope::TxV0(envelope) => EnvelopeView {
            source: MuxedAccount::Ed25519(envelope.tx.source_account_ed25519.clone()).to_string(),
            source_muxed_id: None,
            fee_source: None,
            fee_source_muxed_id: None,
            operations: envelope.tx.operations.as_ref(),
        },
        TransactionEnvelope::Tx(envelope) => {
            let (source, source_muxed_id) = account_identity(&envelope.tx.source_account);
            EnvelopeView {
                source,
                source_muxed_id,
                fee_source: None,
                fee_source_muxed_id: None,
                operations: envelope.tx.operations.as_ref(),
            }
        }
        TransactionEnvelope::TxFeeBump(envelope) => {
            let FeeBumpTransactionInnerTx::Tx(inner) = &envelope.tx.inner_tx;
            let (source, source_muxed_id) = account_identity(&inner.tx.source_account);
            let (fee_source, fee_source_muxed_id) = account_identity(&envelope.tx.fee_source);
            EnvelopeView {
                source,
                source_muxed_id,
                fee_source: Some(fee_source),
                fee_source_muxed_id,
                operations: inner.tx.operations.as_ref(),
            }
        }
    }
}

fn operation_details(
    operations: &[Operation],
    transaction_source: &str,
    transaction_source_muxed_id: Option<u64>,
) -> (
    Vec<AccountInteraction>,
    Vec<String>,
    Vec<WalletContractLink>,
    Vec<ContractCall>,
) {
    let mut accounts = BTreeSet::new();
    let mut authorized_accounts = BTreeSet::new();
    let mut wallet_contract_links = BTreeSet::new();
    accounts.insert((
        transaction_source.to_owned(),
        transaction_source_muxed_id,
        "transaction_source",
    ));
    let mut calls = Vec::new();

    for (operation_index, operation) in operations.iter().enumerate() {
        let (effective_source, effective_source_role) =
            if let Some(source) = &operation.source_account {
                let (account_id, muxed_id) = account_identity(source);
                accounts.insert((account_id.clone(), muxed_id, "operation_source"));
                (account_id, "operation_source")
            } else {
                (transaction_source.to_owned(), "transaction_source")
            };
        if let OperationBody::InvokeHostFunction(invoke) = &operation.body {
            if let HostFunction::InvokeContract(args) = &invoke.host_function {
                if let stellar_xdr::ScAddress::Contract(contract_id) = &args.contract_address {
                    wallet_contract_links.insert((
                        effective_source.clone(),
                        contract_id.0 .0,
                        effective_source_role,
                    ));
                    calls.push(ContractCall {
                        operation_index,
                        contract_id: contract_id.0 .0,
                        function_name: args.function_name.0.to_string(),
                        call_kind: "direct",
                        call_depth: 0,
                    });
                }
            }
            for authorization in invoke.auth.iter() {
                let link_source = match &authorization.credentials {
                    stellar_xdr::SorobanCredentials::SourceAccount => {
                        Some((effective_source.clone(), effective_source_role))
                    }
                    stellar_xdr::SorobanCredentials::Address(credentials)
                    | stellar_xdr::SorobanCredentials::AddressV2(credentials) => {
                        match &credentials.address {
                            stellar_xdr::ScAddress::Account(account) => {
                                let account = account.to_string();
                                authorized_accounts.insert(account.clone());
                                Some((account, "authorized_account"))
                            }
                            _ => None,
                        }
                    }
                    stellar_xdr::SorobanCredentials::AddressWithDelegates(credentials) => {
                        match &credentials.address_credentials.address {
                            stellar_xdr::ScAddress::Account(account) => {
                                let account = account.to_string();
                                authorized_accounts.insert(account.clone());
                                Some((account, "authorized_account"))
                            }
                            _ => None,
                        }
                    }
                };
                let mut authorization_calls = Vec::new();
                collect_authorized_calls(
                    &authorization.root_invocation,
                    operation_index,
                    0,
                    &mut authorization_calls,
                );
                if let Some((account_id, role)) = link_source {
                    for call in &authorization_calls {
                        wallet_contract_links.insert((account_id.clone(), call.contract_id, role));
                    }
                }
                calls.extend(authorization_calls);
            }
        }
    }

    (
        accounts
            .into_iter()
            .map(|(account_id, muxed_id, role)| AccountInteraction {
                account_id,
                muxed_id,
                role,
            })
            .collect(),
        authorized_accounts.into_iter().collect(),
        wallet_contract_links
            .into_iter()
            .map(|(account_id, contract_id, role)| WalletContractLink {
                account_id,
                contract_id,
                role,
            })
            .collect(),
        calls,
    )
}

fn collect_authorized_calls(
    invocation: &SorobanAuthorizedInvocation,
    operation_index: usize,
    depth: usize,
    calls: &mut Vec<ContractCall>,
) {
    if let SorobanAuthorizedFunction::ContractFn(args) = &invocation.function {
        if let stellar_xdr::ScAddress::Contract(contract_id) = &args.contract_address {
            calls.push(ContractCall {
                operation_index,
                contract_id: contract_id.0 .0,
                function_name: args.function_name.0.to_string(),
                call_kind: "authorized",
                call_depth: depth,
            });
        }
    }
    for child in invocation.sub_invocations.iter() {
        collect_authorized_calls(child, operation_index, depth + 1, calls);
    }
}

type HeaderParts<'a> = (&'a stellar_xdr::LedgerHeaderHistoryEntry, i32, u64, usize);

fn meta_parts(meta: &LedgerCloseMeta) -> HeaderParts<'_> {
    match meta {
        LedgerCloseMeta::V0(value) => (&value.ledger_header, 0, 0, 0),
        LedgerCloseMeta::V1(value) => (
            &value.ledger_header,
            1,
            value.total_byte_size_of_live_soroban_state,
            value.evicted_keys.len(),
        ),
        LedgerCloseMeta::V2(value) => (
            &value.ledger_header,
            2,
            value.total_byte_size_of_live_soroban_state,
            value.evicted_keys.len(),
        ),
    }
}

struct ResultFacts {
    hash: [u8; 32],
    successful: bool,
    fee_charged: i64,
    contract_touches: Vec<ContractTouch>,
    contract_versions: Vec<ContractVersionObservation>,
    wasm_codes: Vec<WasmCodeObservation>,
}

fn event_contract(
    event: &stellar_xdr::ContractEvent,
    kind: &'static str,
    touches: &mut BTreeSet<([u8; 32], &'static str)>,
) {
    if let Some(contract_id) = &event.contract_id {
        touches.insert((contract_id.0 .0, kind));
    }
}

#[derive(Default)]
struct TransactionMetaFacts {
    contract_touches: Vec<ContractTouch>,
    contract_versions: Vec<ContractVersionObservation>,
    wasm_codes: Vec<WasmCodeObservation>,
}

fn observe_entry(
    entry: &stellar_xdr::LedgerEntry,
    change_kind: &'static str,
    exact: bool,
    versions: &mut Vec<ContractVersionObservation>,
    wasm_codes: &mut Vec<WasmCodeObservation>,
) {
    match &entry.data {
        stellar_xdr::LedgerEntryData::ContractData(data)
            if matches!(data.key, stellar_xdr::ScVal::LedgerKeyContractInstance) =>
        {
            let (
                stellar_xdr::ScAddress::Contract(contract_id),
                stellar_xdr::ScVal::ContractInstance(instance),
            ) = (&data.contract, &data.val)
            else {
                return;
            };
            let (executable_kind, wasm_hash) = match &instance.executable {
                stellar_xdr::ContractExecutable::Wasm(hash) => ("wasm", Some(hash.0)),
                stellar_xdr::ContractExecutable::StellarAsset => ("stellar_asset", None),
            };
            versions.push(ContractVersionObservation {
                contract_id: contract_id.0 .0,
                executable_kind,
                wasm_hash,
                change_kind,
                exact,
            });
        }
        stellar_xdr::LedgerEntryData::ContractCode(code) => {
            wasm_codes.push(WasmCodeObservation {
                wasm_hash: code.hash.0,
                code_size: code.code.len(),
                change_kind,
            });
        }
        _ => {}
    }
}

fn observe_removed(
    key: &stellar_xdr::LedgerKey,
    versions: &mut Vec<ContractVersionObservation>,
    wasm_codes: &mut Vec<WasmCodeObservation>,
) {
    match key {
        stellar_xdr::LedgerKey::ContractData(data)
            if matches!(data.key, stellar_xdr::ScVal::LedgerKeyContractInstance) =>
        {
            if let stellar_xdr::ScAddress::Contract(contract_id) = &data.contract {
                versions.push(ContractVersionObservation {
                    contract_id: contract_id.0 .0,
                    executable_kind: "removed",
                    wasm_hash: None,
                    change_kind: "removed",
                    exact: true,
                });
            }
        }
        stellar_xdr::LedgerKey::ContractCode(code) => {
            wasm_codes.push(WasmCodeObservation {
                wasm_hash: code.hash.0,
                code_size: 0,
                change_kind: "removed",
            });
        }
        _ => {}
    }
}

fn observe_changes(
    changes: &stellar_xdr::LedgerEntryChanges,
    versions: &mut Vec<ContractVersionObservation>,
    wasm_codes: &mut Vec<WasmCodeObservation>,
) {
    for change in changes.iter() {
        match change {
            stellar_xdr::LedgerEntryChange::Created(entry) => {
                observe_entry(entry, "created", true, versions, wasm_codes)
            }
            stellar_xdr::LedgerEntryChange::Updated(entry) => {
                observe_entry(entry, "updated", true, versions, wasm_codes)
            }
            stellar_xdr::LedgerEntryChange::Restored(entry) => {
                observe_entry(entry, "restored", true, versions, wasm_codes)
            }
            stellar_xdr::LedgerEntryChange::State(entry) => {
                observe_entry(entry, "observed_state", false, versions, wasm_codes)
            }
            stellar_xdr::LedgerEntryChange::Removed(key) => {
                observe_removed(key, versions, wasm_codes)
            }
        }
    }
}

fn transaction_meta_facts(meta: &stellar_xdr::TransactionMeta) -> TransactionMetaFacts {
    let mut touches = BTreeSet::new();
    let mut contract_versions = Vec::new();
    let mut wasm_codes = Vec::new();
    match meta {
        stellar_xdr::TransactionMeta::V3(meta) => {
            observe_changes(
                &meta.tx_changes_before,
                &mut contract_versions,
                &mut wasm_codes,
            );
            for operation in meta.operations.iter() {
                observe_changes(&operation.changes, &mut contract_versions, &mut wasm_codes);
            }
            observe_changes(
                &meta.tx_changes_after,
                &mut contract_versions,
                &mut wasm_codes,
            );
            if let Some(soroban) = &meta.soroban_meta {
                for event in soroban.events.iter() {
                    event_contract(event, "event", &mut touches);
                }
                for event in soroban.diagnostic_events.iter() {
                    event_contract(&event.event, "diagnostic", &mut touches);
                }
            }
        }
        stellar_xdr::TransactionMeta::V4(meta) => {
            observe_changes(
                &meta.tx_changes_before,
                &mut contract_versions,
                &mut wasm_codes,
            );
            for operation in meta.operations.iter() {
                observe_changes(&operation.changes, &mut contract_versions, &mut wasm_codes);
                for event in operation.events.iter() {
                    event_contract(event, "event", &mut touches);
                }
            }
            observe_changes(
                &meta.tx_changes_after,
                &mut contract_versions,
                &mut wasm_codes,
            );
            for event in meta.events.iter() {
                event_contract(&event.event, "event", &mut touches);
            }
            for event in meta.diagnostic_events.iter() {
                event_contract(&event.event, "diagnostic", &mut touches);
            }
        }
        _ => {}
    }
    let contract_touches = touches
        .into_iter()
        .map(|(contract_id, touch_kind)| ContractTouch {
            contract_id,
            touch_kind,
        })
        .collect();
    TransactionMetaFacts {
        contract_touches,
        contract_versions,
        wasm_codes,
    }
}

fn results(meta: &LedgerCloseMeta) -> Vec<ResultFacts> {
    let mut results = Vec::new();
    match meta {
        LedgerCloseMeta::V0(value) => {
            for item in value.tx_processing.iter() {
                let meta = transaction_meta_facts(&item.tx_apply_processing);
                results.push(ResultFacts {
                    hash: item.result.transaction_hash.0,
                    successful: transaction_success(&item.result.result),
                    fee_charged: item.result.result.fee_charged,
                    contract_touches: meta.contract_touches,
                    contract_versions: meta.contract_versions,
                    wasm_codes: meta.wasm_codes,
                });
            }
        }
        LedgerCloseMeta::V1(value) => {
            for item in value.tx_processing.iter() {
                let meta = transaction_meta_facts(&item.tx_apply_processing);
                results.push(ResultFacts {
                    hash: item.result.transaction_hash.0,
                    successful: transaction_success(&item.result.result),
                    fee_charged: item.result.result.fee_charged,
                    contract_touches: meta.contract_touches,
                    contract_versions: meta.contract_versions,
                    wasm_codes: meta.wasm_codes,
                });
            }
        }
        LedgerCloseMeta::V2(value) => {
            for item in value.tx_processing.iter() {
                let meta = transaction_meta_facts(&item.tx_apply_processing);
                results.push(ResultFacts {
                    hash: item.result.transaction_hash.0,
                    successful: transaction_success(&item.result.result),
                    fee_charged: item.result.result.fee_charged,
                    contract_touches: meta.contract_touches,
                    contract_versions: meta.contract_versions,
                    wasm_codes: meta.wasm_codes,
                });
            }
        }
    }
    results
}

pub fn decode_ledger(
    file: &ArchiveFile,
    network_passphrase: &str,
    max_xdr_bytes: usize,
) -> Result<DecodedLedger> {
    let compressed_bytes = std::fs::metadata(&file.path)
        .with_context(|| format!("statting {}", file.path.display()))?
        .len();
    if compressed_bytes > DEFAULT_MAX_COMPRESSED_BYTES {
        bail!(
            "compressed ledger is {} bytes, above the {} byte safety limit: {}",
            compressed_bytes,
            DEFAULT_MAX_COMPRESSED_BYTES,
            file.path.display()
        );
    }
    let input = BufReader::new(
        File::open(&file.path).with_context(|| format!("opening {}", file.path.display()))?,
    );
    let mut decoder = zstd::Decoder::new(input)
        .with_context(|| format!("initializing zstd for {}", file.path.display()))?;
    decoder.window_log_max(DEFAULT_ZSTD_WINDOW_LOG_MAX)?;
    let counting = CountingReader::new(decoder);
    let mut limited = Limited::new(
        counting,
        Limits {
            depth: 500,
            len: max_xdr_bytes,
        },
    );
    let batch = LedgerCloseMetaBatch::read_xdr_to_end(&mut limited)
        .with_context(|| format!("decoding {}", file.path.display()))?;
    let xdr_bytes = limited.inner.bytes_read;

    if batch.ledger_close_metas.len() != 1
        || batch.start_sequence != file.sequence
        || batch.end_sequence != file.sequence
    {
        bail!("invalid single-ledger batch in {}", file.path.display());
    }
    let meta = batch.ledger_close_metas.first().expect("checked above");
    let (header, meta_version, soroban_state_bytes, evicted_keys) = meta_parts(meta);
    if header.header.ledger_seq != file.sequence {
        bail!(
            "ledger header sequence {} disagrees with filename {}",
            header.header.ledger_seq,
            file.sequence
        );
    }

    let ordered_results = results(meta);
    let tx_envelopes = envelopes(meta);
    if tx_envelopes.len() != ordered_results.len() {
        bail!(
            "transaction set/result mismatch in ledger {}: {} envelopes, {} results",
            file.sequence,
            tx_envelopes.len(),
            ordered_results.len()
        );
    }

    let network_id = network_id(network_passphrase);
    let transaction_count = tx_envelopes.len();
    let mut envelope_by_hash = HashMap::with_capacity(transaction_count);
    for envelope in tx_envelopes {
        let hash = envelope
            .hash(network_id)
            .with_context(|| format!("hashing transaction in ledger {}", file.sequence))?;
        if envelope_by_hash.insert(hash, envelope).is_some() {
            bail!(
                "duplicate transaction hash {} in ledger {}",
                hex::encode(hash),
                file.sequence
            );
        }
    }
    let mut transactions = Vec::with_capacity(transaction_count);
    let mut operation_count = 0;
    let mut direct_contract_call_count = 0;
    let mut contract_touch_count = 0;
    let mut successful_transaction_count = 0;
    for (apply_order, result) in ordered_results.into_iter().enumerate() {
        let ResultFacts {
            hash,
            successful,
            fee_charged,
            contract_touches,
            contract_versions,
            wasm_codes,
        } = result;
        let envelope = envelope_by_hash.remove(&hash).with_context(|| {
            format!(
                "transaction {} has no matching result in ledger {}",
                hex::encode(hash),
                file.sequence
            )
        })?;
        let view = envelope_view(envelope);
        let (mut accounts, authorized_accounts, wallet_contract_links, contract_calls) =
            operation_details(view.operations, &view.source, view.source_muxed_id);
        if let Some(fee_source) = &view.fee_source {
            if fee_source != &view.source || view.fee_source_muxed_id != view.source_muxed_id {
                accounts.push(AccountInteraction {
                    account_id: fee_source.clone(),
                    muxed_id: view.fee_source_muxed_id,
                    role: "fee_source",
                });
            }
        }
        operation_count += view.operations.len();
        direct_contract_call_count += contract_calls
            .iter()
            .filter(|call| call.call_kind == "direct")
            .count();
        contract_touch_count += contract_touches.len();
        successful_transaction_count += usize::from(successful);
        transactions.push(IndexedTransaction {
            hash,
            apply_order,
            source_account: view.source,
            source_muxed_id: view.source_muxed_id,
            fee_source_account: view.fee_source,
            fee_source_muxed_id: view.fee_source_muxed_id,
            successful,
            fee_charged,
            operation_count: view.operations.len(),
            accounts,
            authorized_accounts,
            wallet_contract_links,
            contract_calls,
            contract_touches,
            contract_versions,
            wasm_codes,
        });
    }

    Ok(DecodedLedger {
        metrics: LedgerMetrics {
            sequence: file.sequence,
            path: file.path.display().to_string(),
            ledger_hash: header.hash.0,
            previous_ledger_hash: header.header.previous_ledger_hash.0,
            close_time: header.header.scp_value.close_time.0,
            meta_version,
            transaction_count: transactions.len(),
            successful_transaction_count,
            operation_count,
            direct_contract_call_count,
            contract_touch_count,
            soroban_state_bytes,
            evicted_keys,
            compressed_bytes,
            xdr_bytes,
        },
        transactions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{
        AccountId, ContractId, Hash, InvokeContractArgs, InvokeHostFunctionOp, PublicKey,
        ScAddress, ScVal, SorobanAddressCredentials, SorobanAuthorizationEntry, SorobanCredentials,
        Uint256,
    };

    fn account_id(byte: u8) -> AccountId {
        AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([byte; 32])))
    }

    fn muxed_account(byte: u8) -> MuxedAccount {
        MuxedAccount::Ed25519(Uint256([byte; 32]))
    }

    fn invocation(
        contract_byte: u8,
        children: Vec<SorobanAuthorizedInvocation>,
    ) -> SorobanAuthorizedInvocation {
        SorobanAuthorizedInvocation {
            function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                contract_address: ScAddress::Contract(ContractId(Hash([contract_byte; 32]))),
                function_name: "call".try_into().unwrap(),
                args: Vec::new().try_into().unwrap(),
            }),
            sub_invocations: children.try_into().unwrap(),
        }
    }

    fn authorization(
        credentials: SorobanCredentials,
        root: SorobanAuthorizedInvocation,
    ) -> SorobanAuthorizationEntry {
        SorobanAuthorizationEntry {
            credentials,
            root_invocation: root,
        }
    }

    fn invoke_operation(
        source_account: Option<MuxedAccount>,
        direct_contract: u8,
        auth: Vec<SorobanAuthorizationEntry>,
    ) -> Operation {
        Operation {
            source_account,
            body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                host_function: HostFunction::InvokeContract(InvokeContractArgs {
                    contract_address: ScAddress::Contract(ContractId(Hash([direct_contract; 32]))),
                    function_name: "direct".try_into().unwrap(),
                    args: Vec::new().try_into().unwrap(),
                }),
                auth: auth.try_into().unwrap(),
            }),
        }
    }

    #[test]
    fn wallet_contract_links_preserve_effective_sources_and_auth_subtrees() {
        let transaction_source = muxed_account(1).to_string();
        let operation_source = muxed_account(2);
        let operation_source_id = operation_source.to_string();
        let authorized_id = account_id(3);
        let authorized_strkey = authorized_id.to_string();
        let address_credentials = SorobanCredentials::Address(SorobanAddressCredentials {
            address: ScAddress::Account(authorized_id),
            nonce: 0,
            signature_expiration_ledger: 0,
            signature: ScVal::Void,
        });
        let address_auth = authorization(
            address_credentials,
            invocation(11, vec![invocation(12, Vec::new())]),
        );
        let operations = vec![
            invoke_operation(
                None,
                10,
                vec![
                    authorization(
                        SorobanCredentials::SourceAccount,
                        invocation(10, vec![invocation(11, Vec::new())]),
                    ),
                    address_auth.clone(),
                    address_auth,
                ],
            ),
            invoke_operation(
                Some(operation_source),
                13,
                vec![authorization(
                    SorobanCredentials::SourceAccount,
                    invocation(13, vec![invocation(14, Vec::new())]),
                )],
            ),
        ];

        let (accounts, authorized_accounts, links, calls) =
            operation_details(&operations, &transaction_source, None);

        assert_eq!(accounts.len(), 2);
        assert_eq!(authorized_accounts, vec![authorized_strkey.clone()]);
        assert_eq!(
            calls
                .iter()
                .filter(|call| call.call_kind == "direct")
                .count(),
            2
        );
        let actual = links
            .into_iter()
            .map(|link| (link.account_id, link.contract_id, link.role))
            .collect::<BTreeSet<_>>();
        let expected = BTreeSet::from([
            (transaction_source.clone(), [10; 32], "transaction_source"),
            (transaction_source, [11; 32], "transaction_source"),
            (authorized_strkey.clone(), [11; 32], "authorized_account"),
            (authorized_strkey, [12; 32], "authorized_account"),
            (operation_source_id.clone(), [13; 32], "operation_source"),
            (operation_source_id, [14; 32], "operation_source"),
        ]);
        assert_eq!(actual, expected);
    }
}
